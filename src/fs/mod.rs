//! Virtual filesystem trait and implementations.

pub mod memory;
pub mod mountable;
pub mod overlay;
pub mod path;
pub mod readwrite;
pub mod types;

pub use types::{DirEntry, FileType, Metadata};

use crate::error::FsError;

/// A virtual filesystem that shell commands operate on.
///
/// All paths are absolute, Unix-style strings (e.g. `/home/user/file.txt`).
/// Implementations handle path normalization internally.
///
/// Methods take `&self` and use interior mutability so the filesystem can
/// be shared between the `Shell` instance and command contexts.
pub trait VirtualFs {
    fn read_file(&self, path: &str) -> Result<Vec<u8>, FsError>;
    fn read_file_string(&self, path: &str) -> Result<String, FsError>;
    fn write_file(&self, path: &str, content: &[u8]) -> Result<(), FsError>;
    fn append_file(&self, path: &str, content: &[u8]) -> Result<(), FsError>;
    fn exists(&self, path: &str) -> bool;
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;
    fn lstat(&self, path: &str) -> Result<Metadata, FsError>;
    fn mkdir(&self, path: &str, recursive: bool) -> Result<(), FsError>;
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
    fn rm(&self, path: &str, recursive: bool, force: bool) -> Result<(), FsError>;
    fn cp(&self, src: &str, dest: &str, recursive: bool) -> Result<(), FsError>;
    fn mv(&self, src: &str, dest: &str) -> Result<(), FsError>;
    fn chmod(&self, path: &str, mode: u32) -> Result<(), FsError>;
    fn symlink(&self, target: &str, link_path: &str) -> Result<(), FsError>;
    fn hard_link(&self, existing: &str, new_path: &str) -> Result<(), FsError>;
    fn readlink(&self, path: &str) -> Result<String, FsError>;
    fn realpath(&self, path: &str) -> Result<String, FsError>;
    fn touch(&self, path: &str) -> Result<(), FsError>;
    fn set_times(&self, path: &str, mtime: Option<std::time::SystemTime>) -> Result<(), FsError>;
}
