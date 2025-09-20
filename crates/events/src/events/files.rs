use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Fs {
    Folder {
        name: String,
        path: PathBuf,
        modified: Option<SystemTime>,
        children: Vec<Fs>,
    },
    File {
        name: String,
        path: PathBuf,
        modified: Option<SystemTime>,
        size: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileEvent {
    GetFile {
        path: PathBuf,
    },
    FileContent {
        path: PathBuf,
        content: String,
    },
    GetDirectoryListing,
    DirectoryListing {
        path: PathBuf,
        listing: Option<Fs>,
    },
    ReadFile {
        path: PathBuf,
    },
    NewFile {
        path: PathBuf,
        content: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    RenameFile {
        old_path: PathBuf,
        new_path: PathBuf,
    },
}

pub struct FileSystem {
    pub root: PathBuf,
    fs: Fs,
}

impl FileSystem {
    pub fn new(root: PathBuf) -> Self {
        let fs = Self::scan_directory(&root);
        FileSystem { root, fs }
    }

    fn scan_directory(path: &Path) -> Fs {
        let metadata = std::fs::metadata(path).ok();
        let modified = metadata.as_ref().and_then(|m| m.modified().ok());
        let size = metadata.as_ref().and_then(|m| m.len().into());

        if path.is_dir() {
            let mut children = Vec::new();
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let child_path = entry.path();
                    children.push(Self::scan_directory(&child_path));
                }
            }
            Fs::Folder {
                name: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                path: path.to_path_buf(),
                modified,
                children,
            }
        } else {
            Fs::File {
                name: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                path: path.to_path_buf(),
                modified,
                size: size,
            }
        }
    }
}
impl FileEvent {
    pub fn execute(&self, root: &PathBuf) -> Option<FileEvent> {
        match self {
            FileEvent::GetFile { path } => {
                let full_path = root.join(path);
                if full_path.exists() && full_path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        Some(FileEvent::FileContent {
                            path: full_path,
                            content,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            FileEvent::ReadFile { path } => {
                let full_path = root.join(path);
                if full_path.exists() && full_path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        Some(FileEvent::FileContent {
                            path: full_path,
                            content,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            FileEvent::GetDirectoryListing => {
                let listing = FileSystem::scan_directory(root);
                Some(FileEvent::DirectoryListing {
                    path: root.clone(),
                    listing: Some(listing),
                })
            }
            FileEvent::DirectoryListing { path, listing: _ } => {
                let full_path = root.join(path);
                if full_path.exists() && full_path.is_dir() {
                    let listing = FileSystem::scan_directory(&full_path);
                    Some(FileEvent::DirectoryListing {
                        path: full_path,
                        listing: Some(listing),
                    })
                } else {
                    None
                }
            }
            FileEvent::NewFile { path, content } => {
                let full_path = root.join(path);
                if let Some(parent) = full_path.parent() {
                    if !parent.exists() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            log::error!("Failed to create directories: {}", e);
                            return None;
                        }
                    }
                }
                match std::fs::write(&full_path, content) {
                    Ok(_) => Some(FileEvent::FileContent {
                        path: full_path,
                        content: content.clone(),
                    }),
                    Err(e) => {
                        log::error!("Failed to write file: {}", e);
                        None
                    }
                }
            }
            FileEvent::DeleteFile { path } => {
                let full_path = root.join(path);
                if full_path.exists() && full_path.is_file() {
                    match std::fs::remove_file(&full_path) {
                        Ok(_) => Some(FileEvent::DeleteFile { path: full_path }),
                        Err(e) => {
                            log::error!("Failed to delete file: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            }
            FileEvent::RenameFile { old_path, new_path } => {
                let full_old_path = root.join(old_path);
                let full_new_path = root.join(new_path);
                if full_old_path.exists() && full_old_path.is_file() {
                    if let Some(parent) = full_new_path.parent() {
                        if !parent.exists() {
                            if let Err(e) = std::fs::create_dir_all(parent) {
                                log::error!("Failed to create directories: {}", e);
                                return None;
                            }
                        }
                    }
                    match std::fs::rename(&full_old_path, &full_new_path) {
                        Ok(_) => Some(FileEvent::RenameFile {
                            old_path: full_old_path,
                            new_path: full_new_path,
                        }),
                        Err(e) => {
                            log::error!("Failed to rename file: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
