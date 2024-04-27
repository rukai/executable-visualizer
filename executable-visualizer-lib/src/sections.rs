use anyhow::{anyhow, Result};
use goblin::{
    elf::section_header::{
        shf_to_str, sht_to_str, SHF_ALLOC, SHF_FLAGS, SHT_DYNAMIC, SHT_NOBITS, SHT_NULL, SHT_REL,
        SHT_RELA,
    },
    elf64::{header::Header, section_header::SectionHeader},
};
use std::{env::current_exe, path::Path};

pub struct ExecutableFile {
    pub file_root: FileNode,
    pub ram_root: FileNode,
    pub inspector_collapsed: bool,
    pub name: String,
}

impl ExecutableFile {
    pub fn load_self() -> Self {
        Self::load(&current_exe().unwrap()).unwrap()
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file_bytes = std::fs::read(path).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        Self::load_from_bytes(name, &file_bytes)
    }

    pub fn load_from_bytes(name: String, data: &[u8]) -> Result<Self> {
        if data.len() < 4 || data[0..4] != [0x7f, b'E', b'L', b'F'] {
            return Err(anyhow!("Magic ELF bytes were wrong."));
        }
        let header = Header::parse(data).unwrap();

        let mut file_children = vec![];
        let mut ram_children = vec![];
        file_children.push(FileNode {
            name: "ELF Header".into(),
            bytes_start: 0,
            bytes_end: header.e_ehsize as u64,
            ram_bytes_start: 0,
            ram_bytes_end: 0,
            file_bytes_start: 0,
            file_bytes_end: header.e_ehsize as u64,
            children: vec![],
            notes: vec![],
            ty: SectionType::ElfHeader,
        });
        for i in 0..header.e_phnum {
            let bytes_start = header.e_phoff + i as u64 * header.e_phentsize as u64;
            let bytes_end = header.e_phoff + (i as u64 + 1) * header.e_phentsize as u64;
            file_children.push(FileNode {
                name: format!("Program Header Segment #{i}"),
                bytes_start,
                bytes_end,
                ram_bytes_start: 0,
                ram_bytes_end: 0,
                file_bytes_start: bytes_start,
                file_bytes_end: bytes_end,
                children: vec![],
                notes: vec![],
                ty: SectionType::ElfProgramHeader,
            });
        }

        // The program headers will point at parts of the file, telling the os which parts to load into specific locations in memory.
        // We dont parse or take that into account at all since that is just a subset of the data defined by the elf sections.

        let section_headers =
            SectionHeader::from_bytes(&data[header.e_shoff as usize..], header.e_shnum as usize);
        let str_table_header = section_headers[header.e_shstrndx as usize];
        let str_table_start = str_table_header.sh_offset as usize;
        let str_table_end = str_table_start + str_table_header.sh_size as usize;
        let section_name_table = &data[str_table_start..str_table_end];

        // These headers are usually at the very end of the file
        let section_headers_start = header.e_shoff;

        for (i, section_header) in section_headers.iter().enumerate() {
            let name = parse_str_table(section_name_table, section_header.sh_name);
            let bytes_start = section_headers_start + i as u64 * header.e_shentsize as u64;
            let bytes_end = section_headers_start + (i as u64 + 1) * header.e_shentsize as u64;
            file_children.push(FileNode {
                name: format!("ELF Section Header for {name}"),
                bytes_start,
                bytes_end,
                ram_bytes_start: 0,
                ram_bytes_end: 0,
                file_bytes_start: bytes_start,
                file_bytes_end: bytes_end,
                children: vec![],
                notes: vec![],
                ty: SectionType::ElfSectionHeader,
            });
        }

        for section_header in &section_headers {
            // https://docs.oracle.com/cd/E19683-01/816-1386/chapter6-94076/index.html

            let ty = sht_to_str(section_header.sh_type).to_owned();
            let mut flags = String::new();
            for flag in SHF_FLAGS {
                if section_header.sh_flags & flag as u64 != 0 {
                    if !flags.is_empty() {
                        flags.push('|')
                    }
                    flags.push_str(shf_to_str(flag));
                }
            }
            if flags.is_empty() {
                flags = "NONE".to_owned();
            }

            let ram_bytes_start = section_header.sh_addr;
            let ram_bytes_end = section_header.sh_addr + section_header.sh_size;
            let address_alignment = format!("0x{:x}", section_header.sh_addralign);
            let mut notes = vec![
                ("type".into(), ty),
                ("flags".into(), flags),
                ("address alignment".into(), address_alignment),
            ];

            // TODO: and many other sh_link handling https://docs.oracle.com/cd/E19683-01/816-1386/6m7qcoblj/index.html#chapter6-47976
            let link_name = section_headers
                .get(section_header.sh_link as usize)
                .map(|section_header| parse_str_table(section_name_table, section_header.sh_name))
                .unwrap_or_else(|| "bad link section".to_owned());
            if section_header.sh_type == SHT_DYNAMIC {
                notes.push(("string table in section".into(), link_name));
            } else if section_header.sh_type == SHT_REL || section_header.sh_type == SHT_RELA {
                notes.push(("symbol table in section".into(), link_name));
            }

            let file_bytes_start = section_header.sh_offset;
            let file_bytes_end = section_header.sh_offset + section_header.sh_size;
            let name = parse_str_table(section_name_table, section_header.sh_name);

            if section_header.sh_flags & SHF_ALLOC as u64 != 0 {
                ram_children.push(FileNode {
                    name: name.clone(),
                    bytes_start: ram_bytes_start,
                    bytes_end: ram_bytes_end,
                    ram_bytes_start,
                    ram_bytes_end,
                    file_bytes_start,
                    file_bytes_end,
                    children: vec![],
                    notes: notes.clone(),
                    ty: SectionType::ElfSectionHeader,
                });
            }
            if section_header.sh_type != SHT_NOBITS && section_header.sh_type != SHT_NULL {
                file_children.push(FileNode {
                    name,
                    bytes_start: file_bytes_start,
                    bytes_end: file_bytes_end,
                    ram_bytes_start,
                    ram_bytes_end,
                    file_bytes_start,
                    file_bytes_end,
                    children: vec![],
                    notes,
                    ty: SectionType::ElfSectionHeader,
                });
            }
        }
        for child in &file_children {
            for other_child in &file_children {
                if child.overlaps(other_child) {
                    println!("WARN: Overlapping children found: {child:?} {other_child:?}");
                }
            }
        }

        let mut file_root = FileNode {
            name: "ELF file".into(),
            bytes_start: 0,
            bytes_end: data.len() as u64,
            ram_bytes_start: 0,
            ram_bytes_end: 0, // TODO
            file_bytes_start: 0,
            file_bytes_end: data.len() as u64,
            notes: vec![],
            children: file_children,
            ty: SectionType::Root,
        };
        file_root.sort();

        let ram_bytes_end = ram_children.iter().map(|x| x.bytes_end).max().unwrap();
        let mut ram_root = FileNode {
            name: "RAM".into(),
            bytes_start: 0,
            bytes_end: ram_bytes_end,
            ram_bytes_start: 0,
            ram_bytes_end,
            file_bytes_start: 0,
            file_bytes_end: data.len() as u64,
            notes: vec![],
            children: ram_children,
            ty: SectionType::Root,
        };
        ram_root.sort();

        Ok(ExecutableFile {
            name,
            file_root,
            ram_root,
            inspector_collapsed: false,
        })
    }
}

fn parse_str_table(data: &[u8], offset: u32) -> String {
    if offset as usize > data.len() {
        return "sh_name out of bounds of string table".to_owned();
    }
    std::ffi::CStr::from_bytes_until_nul(&data[offset as usize..])
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub bytes_start: u64,
    pub bytes_end: u64,
    pub ram_bytes_start: u64,
    pub ram_bytes_end: u64,
    pub file_bytes_start: u64,
    pub file_bytes_end: u64,
    pub ty: SectionType,
    pub notes: Vec<(String, String)>,
    pub children: Vec<FileNode>,
}

impl FileNode {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        self.bytes_end - self.bytes_start
    }

    fn sort(&mut self) {
        self.children.sort_by_key(|x| x.bytes_start);
        for child in &mut self.children {
            child.sort();
        }
    }

    #[rustfmt::skip]
    fn overlaps(&self, other: &Self) -> bool {
        self.bytes_start > other.bytes_start && self.bytes_start < other.bytes_start + other.len() ||
        self.bytes_end   > other.bytes_start && self.bytes_end   < other.bytes_start + other.len()
    }
}

#[derive(Debug, Clone)]
pub enum SectionType {
    ElfHeader,
    ElfSectionHeader,
    ElfProgramHeader,
    Text,
    Root,
}
