//! Core filesystem types: entries, metadata, and directory listings.

use std::time::SystemTime;

/// Default permission modes.
pub const DEFAULT_FILE_MODE: u32 = 0o644;
pub const DEFAULT_DIR_MODE: u32 = 0o755;
pub const SYMLINK_MODE: u32 = 0o777;

/// Maximum symlink resolution depth before reporting ELOOP.
pub const MAX_SYMLINK_DEPTH: u32 = 40;

/// An entry in the virtual filesystem.
#[derive(Debug, Clone)]
pub enum FsEntry {
    File(FileData),
    Directory(DirData),
    Symlink(SymlinkData),
}

impl FsEntry {
    pub fn file_type(&self) -> FileType {
        match self {
            Self::File(_) => FileType::File,
            Self::Directory(_) => FileType::Directory,
            Self::Symlink(_) => FileType::Symlink,
        }
    }

    pub fn metadata(&self) -> Metadata {
        match self {
            Self::File(f) => Metadata {
                file_type: FileType::File,
                size: f.content.len() as u64,
                mode: f.mode,
                mtime: f.mtime,
            },
            Self::Directory(d) => Metadata {
                file_type: FileType::Directory,
                size: 0,
                mode: d.mode,
                mtime: d.mtime,
            },
            Self::Symlink(s) => Metadata {
                file_type: FileType::Symlink,
                size: s.target.len() as u64,
                mode: s.mode,
                mtime: s.mtime,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileData {
    pub content: Vec<u8>,
    pub mode: u32,
    pub mtime: SystemTime,
}

#[derive(Debug, Clone)]
pub struct DirData {
    pub mode: u32,
    pub mtime: SystemTime,
}

#[derive(Debug, Clone)]
pub struct SymlinkData {
    pub target: String,
    pub mode: u32,
    pub mtime: SystemTime,
}

/// File metadata returned by `stat` / `lstat`.
#[derive(Debug, Clone)]
pub struct Metadata {
    pub file_type: FileType,
    pub size: u64,
    pub mode: u32,
    pub mtime: SystemTime,
}

impl Metadata {
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File
    }

    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }

    pub fn is_symlink(&self) -> bool {
        self.file_type == FileType::Symlink
    }
}

/// The kind of filesystem entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

/// An entry returned by `readdir`.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub file_type: FileType,
}
