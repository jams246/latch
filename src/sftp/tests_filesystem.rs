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
    pub folders: HashSet<String>,
    pub links: HashMap<String, String>,
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
                ("/socket".into(), Vec::new()),
                ("/block-device".into(), Vec::new()),
            ]),
            folders: FOLDERS.iter().map(|path| (*path).into()).collect(),
            links: LINKS
                .iter()
                .map(|(path, target)| ((*path).into(), (*target).into()))
                .collect(),
            directories: HashSet::new(),
            stall_close,
            deny_remove,
        }
    }

    pub fn attrs(&self, path: &str) -> Result<FileAttributes, StatusCode> {
        let path = normalized(path);
        if self.folders.contains(&path) {
            Ok(attributes(4096, 0o040755, 1_690_000_000))
        } else if let Some(target) = self.links.get(&path) {
            Ok(attributes(target.len() as u64, 0o120777, 1_710_000_000))
        } else {
            self.files
                .get(&path)
                .map(|file| {
                    let permissions = match path.as_str() {
                        "/socket" => 0o140777,
                        "/block-device" => 0o060660,
                        _ => 0o100644,
                    };
                    attributes(
                        file.len() as u64,
                        permissions,
                        1_700_000_000 + file.len() as u32,
                    )
                })
                .ok_or(StatusCode::NoSuchFile)
        }
    }

    pub fn resolved(&self, path: &str) -> Result<String, StatusCode> {
        let mut path = normalized(path);
        for _ in 0..32 {
            let Some((link, target)) = self.links.iter().find(|(link, _)| {
                path == **link
                    || path
                        .strip_prefix(link.as_str())
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
        self.folders
            .iter()
            .map(String::as_str)
            .chain(self.links.keys().map(String::as_str))
            .chain(self.files.keys().map(String::as_str))
            .filter_map(|path| {
                let name = path.strip_prefix(&prefix)?;
                (!name.is_empty() && !name.contains('/')).then_some((path, name))
            })
            .map(|(path, name)| self.attrs(path).map(|attrs| File::new(name, attrs)))
            .collect()
    }

    pub fn rename_path(&mut self, old: &str, new: &str) -> Result<(), StatusCode> {
        self.attrs(old)?;
        if self.attrs(new).is_ok() {
            return Err(StatusCode::Failure);
        }
        let prefix = format!("{old}/");
        let paths: Vec<_> = self
            .folders
            .iter()
            .chain(self.files.keys())
            .chain(self.links.keys())
            .filter(|path| path.as_str() == old || path.starts_with(&prefix))
            .cloned()
            .collect();
        for path in paths {
            let destination = format!("{new}{}", &path[old.len()..]);
            if let Some(content) = self.files.remove(&path) {
                self.files.insert(destination, content);
            } else if let Some(target) = self.links.remove(&path) {
                self.links.insert(destination, target);
            } else {
                self.folders.remove(&path);
                self.folders.insert(destination);
            }
        }
        Ok(())
    }
}
