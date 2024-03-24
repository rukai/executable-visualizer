use anyhow::{anyhow, Result};
use goblin::elf64::{header::Header, section_header::SectionHeader};
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
            ty: SectionType::ElfHeader,
        });
        for i in 0..header.e_phnum {
            children.push(FileNode {
                name: format!("Program Header Segment #{i}"),
                bytes_start: header.e_phoff as i64 + i as i64 * header.e_phentsize as i64,
                bytes_end: header.e_phoff as i64 + (i as i64 + 1) * header.e_phentsize as i64,
                children: vec![],
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
        let section_name_table = parse_str_table(&data[str_table_start..str_table_end]);

        // These headers are usually at the very end of the file
        let section_headers_start = header.e_shoff as i64;
        for i in 0..header.e_shnum {
            let name = section_name_table
                .get(i as usize)
                .cloned()
                .unwrap_or_else(|| "Unnamed section".to_owned());
            children.push(FileNode {
                name: format!("ELF Section Header for {name}"),
                bytes_start: section_headers_start + i as i64 * header.e_shentsize as i64,
                bytes_end: section_headers_start + (i as i64 + 1) * header.e_shentsize as i64,
                children: vec![],
                ty: SectionType::ElfSectionHeader,
            });
        }

        for (i, section_header) in section_headers.iter().enumerate() {
            children.push(FileNode {
                name: section_name_table
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| "Unnamed section".to_owned()),
                bytes_start: section_header.sh_offset as i64,
                bytes_end: section_header.sh_offset as i64 + section_header.sh_size as i64,
                children: vec![],
                ty: SectionType::ElfSectionHeader,
            });
        }

        let root = FileNode {
            name: "ELF file".into(),
            bytes_start: 0,
            bytes_end: data.len() as i64,
            children,
            ty: SectionType::Root,
        };

        Ok(ExecutableFile {
            name,
            root,
            inspector_collapsed: false,
        })
    }
}

fn parse_str_table(data: &[u8]) -> Vec<String> {
    data.split(|c| *c == 0)
        .map(|x| String::from_utf8(x.to_vec()).unwrap())
        .collect()
}

pub struct FileNode {
    pub name: String,
    pub bytes_start: i64,
    pub bytes_end: i64,
    pub ty: SectionType,
    pub children: Vec<FileNode>,
}

pub enum SectionType {
    ElfHeader,
    ElfSectionHeader,
    ElfProgramHeader,
    Text,
    Root,
}
