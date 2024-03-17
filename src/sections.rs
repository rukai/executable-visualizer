use goblin::elf64::{header::Header, section_header::SectionHeader};
use std::{env::current_exe, path::Path};

pub struct ExecutableFile {
    pub root: FileNode,
    pub inspector_collapsed: bool,
    pub name: String,
}

impl ExecutableFile {
    pub fn load_self() -> Self {
        Self::load(&current_exe().unwrap())
    }

    pub fn load(path: &Path) -> Self {
        let file_bytes = std::fs::read(path).unwrap();
        let header = Header::parse(&file_bytes).unwrap();

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

        // These headers are usually at the very end of the file
        let section_headers_start = header.e_shoff as i64;
        for i in 0..header.e_shnum {
            children.push(FileNode {
                name: "ELF Section Header".into(),
                bytes_start: section_headers_start + i as i64 * header.e_shentsize as i64,
                bytes_end: section_headers_start + (i as i64 + 1) * header.e_shentsize as i64,
                children: vec![],
                ty: SectionType::ElfSectionHeader,
            });
        }

        for section_header in SectionHeader::from_bytes(
            &file_bytes[header.e_shoff as usize..],
            header.e_shnum as usize,
        ) {
            children.push(FileNode {
                name: "ELF Section".into(),
                bytes_start: section_header.sh_offset as i64,
                bytes_end: section_header.sh_offset as i64 + section_header.sh_size as i64,
                children: vec![],
                ty: SectionType::ElfSectionHeader,
            });
        }

        let root = FileNode {
            name: "ELF file".into(),
            bytes_start: 0,
            bytes_end: file_bytes.len() as i64,
            children,
            ty: SectionType::Root,
        };

        ExecutableFile {
            name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            root,
            inspector_collapsed: false,
        }
    }

    pub fn load_dummy() -> Self {
        let root = FileNode {
            name: "foo".into(),
            bytes_start: 0,
            bytes_end: 1_000_000,
            children: vec![
                FileNode {
                    name: "child1".into(),
                    bytes_start: 0,
                    bytes_end: 10,
                    children: vec![],
                    ty: SectionType::ElfHeader,
                },
                FileNode {
                    name: "child2".into(),
                    bytes_start: 10,
                    bytes_end: 1_000_000,
                    children: vec![
                        FileNode {
                            name: "child21".into(),
                            bytes_start: 10,
                            bytes_end: 100_000,
                            children: vec![],
                            ty: SectionType::ElfHeader,
                        },
                        FileNode {
                            name: "child22".into(),
                            bytes_start: 100_000,
                            bytes_end: 1_000_000,
                            children: vec![],
                            ty: SectionType::ElfHeader,
                        },
                    ],
                    ty: SectionType::Text,
                },
            ],
            ty: SectionType::ElfHeader,
        };

        ExecutableFile {
            name: "dummy file".to_owned(),
            root,
            inspector_collapsed: false,
        }
    }
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
