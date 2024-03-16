use std::{env::current_exe, path::Path};

pub struct ExecutableFile {
    pub root: Section,
    pub inspector_collapsed: bool,
    pub name: String,
}

impl ExecutableFile {
    pub fn load_self() -> Self {
        Self::load(&current_exe().unwrap())
    }

    pub fn load(path: &Path) -> Self {
        let file_bytes = std::fs::read(path).unwrap();
        let root = Section {
            name: "ELF file".into(),
            bytes_start: 0,
            bytes_end: file_bytes.len() as i64,
            children: vec![],
            ty: SectionType::Header,
        };

        ExecutableFile {
            name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            root,
            inspector_collapsed: false,
        }
    }

    pub fn load_dummy() -> Self {
        let root = Section {
            name: "foo".into(),
            bytes_start: 0,
            bytes_end: 1_000_000,
            children: vec![
                Section {
                    name: "child1".into(),
                    bytes_start: 0,
                    bytes_end: 10,
                    children: vec![],
                    ty: SectionType::Header,
                },
                Section {
                    name: "child2".into(),
                    bytes_start: 10,
                    bytes_end: 1_000_000,
                    children: vec![
                        Section {
                            name: "child21".into(),
                            bytes_start: 10,
                            bytes_end: 100_000,
                            children: vec![],
                            ty: SectionType::Header,
                        },
                        Section {
                            name: "child22".into(),
                            bytes_start: 100_000,
                            bytes_end: 1_000_000,
                            children: vec![],
                            ty: SectionType::Header,
                        },
                    ],
                    ty: SectionType::Text,
                },
            ],
            ty: SectionType::Header,
        };

        ExecutableFile {
            name: "dummy file".to_owned(),
            root,
            inspector_collapsed: false,
        }
    }
}

pub struct Section {
    pub name: String,
    pub bytes_start: i64,
    pub bytes_end: i64,
    pub ty: SectionType,
    pub children: Vec<Section>,
}

pub enum SectionType {
    Header,
    Text,
}
