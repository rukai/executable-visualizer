use anyhow::{anyhow, Result};
use goblin::{
    elf::section_header::{shf_to_str, sht_to_str, SHF_FLAGS, SHT_DYNAMIC, SHT_REL, SHT_RELA},
    elf64::{header::Header, section_header::SectionHeader},
};
use std::{env::current_exe, path::Path};

pub struct ExecutableFile {
    pub root: FileNode,
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

        let mut children = vec![];
        children.push(FileNode {
            name: "ELF Header".into(),
            bytes_start: 0,
            bytes_end: header.e_ehsize as i64,
            children: vec![],
            notes: vec![],
            ty: SectionType::ElfHeader,
        });
        for i in 0..header.e_phnum {
            children.push(FileNode {
                name: format!("Program Header Segment #{i}"),
                bytes_start: header.e_phoff as i64 + i as i64 * header.e_phentsize as i64,
                bytes_end: header.e_phoff as i64 + (i as i64 + 1) * header.e_phentsize as i64,
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
        let section_headers_start = header.e_shoff as i64;
        for (i, section_header) in section_headers.iter().enumerate() {
            let name = parse_str_table(section_name_table, section_header.sh_name);
            children.push(FileNode {
                name: format!("ELF Section Header for {name}"),
                bytes_start: section_headers_start + i as i64 * header.e_shentsize as i64,
                bytes_end: section_headers_start + (i as i64 + 1) * header.e_shentsize as i64,
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

            let address = format!("0x{:x}", section_header.sh_addr);
            let address_alignment = format!("0x{:x}", section_header.sh_addralign);
            let mut notes = vec![
                ("type".into(), ty),
                ("flags".into(), flags),
                ("address".into(), address),
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

            let bytes_start = section_header.sh_offset as i64;
            let mut bytes_end = section_header.sh_offset as i64 + section_header.sh_size as i64;
            if bytes_start == bytes_end {
                bytes_end += 1;
                notes.push((
                    "".into(),
                    "This section is actually 0 bytes, but is drawn 1 byte wide so that it will show up"
                        .into(),
                ));
            }

            children.push(FileNode {
                name: parse_str_table(section_name_table, section_header.sh_name),
                bytes_start,
                bytes_end,
                children: vec![],
                notes,
                ty: SectionType::ElfSectionHeader,
            });
        }

        for (i, child) in children.iter_mut().enumerate() {
            child.notes.push(("i".into(), format!("{i}")));
        }

        // Some sections will overlap each other.
        // To ensure they are completely drawn render the smallest section as a child of the bigger section.
        // TODO: or something like that...
        // The problem is currently ill-defined, so I need to better define and improve our handling here.
        // For example .bss section still overlaps
        // Each child is immediately added as a child of another node when found to overlap.
        // If its new parent already has children:
        // * If overlap with any children:
        //   + if smaller become its child
        //   + if larger take its spot
        // * If no overlap with children join as sibling.
        let mut i = 0;
        loop {
            let mut found = None;
            for (other_i, other_child) in children.iter().enumerate() {
                if i != other_i && children[i].overlaps(other_child) {
                    found = Some((i, other_i));
                }
            }
            if let Some((base_i, other_i)) = found {
                // Need to remove the later index first to maintain index validity
                let (mut child1, mut child2) = if base_i < other_i {
                    (children.remove(other_i), children.remove(base_i))
                } else {
                    (children.remove(base_i), children.remove(other_i))
                };
                let child = if child1.len() > child2.len() {
                    child1.children.push(child2);
                    child1
                } else {
                    child2.children.push(child1);
                    child2
                };
                children.push(child);

                // restart from beginning
                i = 0;
            } else {
                i += 1;
                if i + 1 > children.len() {
                    break;
                }
            }
        }

        let mut root = FileNode {
            name: "ELF file".into(),
            bytes_start: 0,
            bytes_end: data.len() as i64,
            notes: vec![],
            children,
            ty: SectionType::Root,
        };
        root.sort();

        Ok(ExecutableFile {
            name,
            root,
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

#[derive(Debug)]
pub struct FileNode {
    pub name: String,
    pub bytes_start: i64,
    pub bytes_end: i64,
    pub ty: SectionType,
    pub notes: Vec<(String, String)>,
    pub children: Vec<FileNode>,
}

impl FileNode {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        (self.bytes_end - self.bytes_start) as u64
    }

    fn sort(&mut self) {
        self.children.sort_by_key(|x| x.bytes_start);
        for child in &mut self.children {
            child.sort();
        }
    }

    #[rustfmt::skip]
    fn overlaps(&self, other: &Self) -> bool {
        self.bytes_start > other.bytes_start && self.bytes_start < other.bytes_start + other.len() as i64 ||
        self.bytes_end   > other.bytes_start && self.bytes_end   < other.bytes_start + other.len() as i64
    }
}

#[derive(Debug)]
pub enum SectionType {
    ElfHeader,
    ElfSectionHeader,
    ElfProgramHeader,
    Text,
    Root,
}
