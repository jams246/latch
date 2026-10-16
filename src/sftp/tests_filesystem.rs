use std::collections::{HashMap, HashSet};

use russh_sftp::protocol::{File, FileAttributes, StatusCode};

const FOLDERS: &[&str] = &["/", "/folder", "/other", "/other/deep"];
const LINKS: &[(&str, &str)] = &[
    ("/folder-link", "/folder"),
    ("/file-link", "/hello.txt"),
    ("/relative-link", "other/deep"),
    ("/chain-link", "/relative-link"),
    ("/broken-link", "/missing"),
    ("/loop-link", "/loop-link"),
    ("/other/parent-link", ".."),
];

pub(super) struct Filesystem {
    pub files: HashMap<String, Vec<u8>>,
    pub directories: HashSet<String>,
    pub stall_close: bool,
    pub deny_remove: bool,
}

pub(super) fn normalized(path: &str) -> String {
    let mut components = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                components.pop();
            }
            name => components.push(name),
        }
    }
    format!("/{}", components.join("/"))
}

fn attributes(size: u64, permissions: u32, modified: u32) -> FileAttributes {
    FileAttributes {
        size: Some(size),
        permissions: Some(permissions),
        atime: Some(modified),
        mtime: Some(modified),
        ..Default::default()
    }
}

impl Filesystem {
    pub fn new(stall_close: bool, deny_remove: bool) -> Self {
        Self {
            files: HashMap::from([
                ("/hello.txt".into(), b"Synthetic SFTP content\n".to_vec()),
                (
                    "/folder/nested.txt".into(),
                    b"Nested synthetic content\n".to_vec(),
                ),
                (
                    "/other/deep/item.csv".into(),
                    b"name,value\nSFTP,42\n".to_vec(),
                ),
                ("/large.bin".into(), vec![0x5a; 16 * 1024 * 1024]),
            ]),
            directories: HashSet::new(),
            stall_close,
            deny_remove,
        }
    }

    pub fn attrs(&self, path: &str) -> Result<FileAttributes, StatusCode> {
        let path = normalized(path);
        if FOLDERS.contains(&path.as_str()) {
            Ok(attributes(4096, 0o040755, 1_690_000_000))
        } else if let Some((_, target)) = LINKS.iter().find(|(link, _)| *link == path) {
            Ok(attributes(target.len() as u64, 0o120777, 1_710_000_000))
        } else {
            self.files
                .get(&path)
                .map(|file| {
                    attributes(
                        file.len() as u64,
                        0o100644,
                        1_700_000_000 + file.len() as u32,
                    )
                })
                .ok_or(StatusCode::NoSuchFile)
        }
    }

    pub fn resolved(&self, path: &str) -> Result<String, StatusCode> {
        let mut path = normalized(path);
        for _ in 0..32 {
            let Some((link, target)) = LINKS.iter().find(|(link, _)| {
                path == *link
                    || path
                        .strip_prefix(link)
                        .is_some_and(|rest| rest.starts_with('/'))
            }) else {
                self.attrs(&path)?;
                return Ok(path);
            };
            let target = if target.starts_with('/') {
                target.to_string()
            } else {
                let parent = link.rsplit_once('/').unwrap().0;
                format!("{parent}/{target}")
            };
            path = normalized(&format!("{target}{}", &path[link.len()..]));
        }
        Err(StatusCode::Failure)
    }

    pub fn entries(&self, directory: &str) -> Result<Vec<File>, StatusCode> {
        let prefix = if directory == "/" {
            "/".into()
        } else {
            format!("{directory}/")
        };
        FOLDERS
            .iter()
            .copied()
            .chain(LINKS.iter().map(|(path, _)| *path))
            .chain(self.files.keys().map(String::as_str))
            .filter_map(|path| {
                let name = path.strip_prefix(&prefix)?;
                (!name.is_empty() && !name.contains('/')).then_some((path, name))
            })
            .map(|(path, name)| self.attrs(path).map(|attrs| File::new(name, attrs)))
            .collect()
    }
}
