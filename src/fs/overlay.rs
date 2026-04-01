use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::error::FsError;
use crate::fs::memory::InMemoryFs;
use crate::fs::path;
use crate::fs::types::{
    DirEntry, FileType, Metadata, DEFAULT_DIR_MODE, DEFAULT_FILE_MODE,
};
use crate::fs::VirtualFs;

const DEFAULT_MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Copy-on-write overlay filesystem.
///
/// Reads fall through to a real directory on the host filesystem.
/// Writes are captured in an in-memory layer and never touch disk.
/// Deleted real files are tracked via tombstones.
pub struct OverlayFs {
    root: PathBuf,
    memory: InMemoryFs,
    deleted: RefCell<HashSet<String>>,
    max_file_size: usize,
}

impl OverlayFs {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, FsError> {
        let root = root.into();
        if !root.is_dir() {
            return Err(FsError::NotADirectory(root.display().to_string()));
        }
        let root = root.canonicalize().map_err(|_| {
            FsError::NotFound(root.display().to_string())
        })?;
        Ok(Self {
            root,
            memory: InMemoryFs::new(),
            deleted: RefCell::new(HashSet::new()),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        })
    }

    #[must_use]
    pub fn with_max_file_size(mut self, size: usize) -> Self {
        self.max_file_size = size;
        self
    }

    /// Map a virtual path to a real host path, checking it stays within the sandbox root.
    ///
    /// # TOCTOU note
    ///
    /// There is a theoretical time-of-check-to-time-of-use gap between
    /// `canonicalize()` and the `starts_with()` sandbox check: an external
    /// process could manipulate symlinks on the host filesystem in between.
    /// In practice this is not exploitable because:
    ///
    /// 1. `OverlayFs` never writes to the real filesystem -- all writes go
    ///    to the in-memory layer. The real FS is read-only from our perspective.
    /// 2. The only entity that could create a race is an external process with
    ///    write access to the host directory, which is outside our threat model
    ///    (the sandbox protects the *script* from escaping, not against a
    ///    malicious host).
    /// 3. Even if the race succeeded, the attacker could only cause us to
    ///    *read* an out-of-sandbox file; they cannot cause writes to disk.
    fn real_path(&self, virtual_path: &str) -> Result<PathBuf, FsError> {
        if !path::validate(virtual_path) {
            return Err(FsError::InvalidArgument(format!(
                "{virtual_path}: path contains invalid characters"
            )));
        }
        let normalized = path::normalize(virtual_path);
        let relative = normalized.strip_prefix('/').unwrap_or(&normalized);
        let joined = if relative.is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR))
        };
        let canonical_root = &self.root;
        let resolved = if joined.exists() {
            joined.canonicalize().map_err(|e| {
                FsError::NotFound(format!("{virtual_path}: {e}"))
            })?
        } else {
            let parent = joined.parent().ok_or_else(|| {
                FsError::NotFound(virtual_path.to_string())
            })?;
            if parent.exists() {
                let canonical_parent = parent.canonicalize().map_err(|e| {
                    FsError::NotFound(format!("{virtual_path}: {e}"))
                })?;
                if let Some(file_name) = joined.file_name() {
                    canonical_parent.join(file_name)
                } else {
                    canonical_parent
                }
            } else {
                joined.clone()
            }
        };
        if !resolved.starts_with(canonical_root) {
            return Err(FsError::PermissionDenied(format!(
                "{virtual_path}: path escapes sandbox"
            )));
        }
        Ok(resolved)
    }

    fn is_deleted(&self, virtual_path: &str) -> bool {
        let normalized = path::normalize(virtual_path);
        let deleted = self.deleted.borrow();
        if deleted.contains(&normalized) {
            return true;
        }
        for d in deleted.iter() {
            if normalized.starts_with(d) && normalized.as_bytes().get(d.len()) == Some(&b'/') {
                return true;
            }
        }
        false
    }

    fn mark_deleted(&self, virtual_path: &str) {
        let normalized = path::normalize(virtual_path);
        self.deleted.borrow_mut().insert(normalized);
    }

    fn unmark_deleted(&self, virtual_path: &str) {
        let normalized = path::normalize(virtual_path);
        self.deleted.borrow_mut().remove(&normalized);
    }

    fn real_exists(&self, virtual_path: &str) -> bool {
        if self.is_deleted(virtual_path) {
            return false;
        }
        match self.real_path(virtual_path) {
            Ok(p) => p.exists(),
            Err(_) => false,
        }
    }

    fn read_real_metadata(&self, virtual_path: &str) -> Result<Metadata, FsError> {
        let real = self.real_path(virtual_path)?;
        let meta = std::fs::metadata(&real).map_err(|_| {
            FsError::NotFound(virtual_path.to_string())
        })?;
        Ok(host_metadata_to_virtual(&meta))
    }

    fn read_real_symlink_metadata(&self, virtual_path: &str) -> Result<Metadata, FsError> {
        let real = self.real_path(virtual_path)?;
        let meta = std::fs::symlink_metadata(&real).map_err(|_| {
            FsError::NotFound(virtual_path.to_string())
        })?;
        Ok(host_metadata_to_virtual(&meta))
    }

    fn read_real_file(&self, virtual_path: &str) -> Result<Vec<u8>, FsError> {
        let real = self.real_path(virtual_path)?;
        let meta = std::fs::metadata(&real).map_err(|_| {
            FsError::NotFound(virtual_path.to_string())
        })?;
        if meta.is_dir() {
            return Err(FsError::IsADirectory(virtual_path.to_string()));
        }
        if meta.len() > self.max_file_size as u64 {
            return Err(FsError::TooLarge(virtual_path.to_string()));
        }
        std::fs::read(&real).map_err(|_| {
            FsError::NotFound(virtual_path.to_string())
        })
    }

    fn promote_to_memory(&self, virtual_path: &str) -> Result<(), FsError> {
        if self.memory.exists(virtual_path) {
            return Ok(());
        }
        let meta = self.read_real_metadata(virtual_path)?;
        if meta.is_dir() {
            self.memory.mkdir(virtual_path, true)?;
        } else if meta.is_file() {
            let content = self.read_real_file(virtual_path)?;
            let parent = path::parent(virtual_path);
            if parent != "/" && !self.memory.exists(parent) {
                self.memory.mkdir(parent, true)?;
            }
            self.memory.write_file(virtual_path, &content)?;
        }
        Ok(())
    }
}

fn host_metadata_to_virtual(meta: &std::fs::Metadata) -> Metadata {
    let file_type = if meta.is_dir() {
        FileType::Directory
    } else if meta.file_type().is_symlink() {
        FileType::Symlink
    } else {
        FileType::File
    };
    let mode = if meta.is_dir() {
        DEFAULT_DIR_MODE
    } else {
        DEFAULT_FILE_MODE
    };
    let mtime = meta.modified().unwrap_or_else(|_| SystemTime::now());
    Metadata {
        file_type,
        size: meta.len(),
        mode,
        mtime,
    }
}

impl std::fmt::Debug for OverlayFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayFs")
            .field("root", &self.root)
            .field("deleted_count", &self.deleted.borrow().len())
            .finish_non_exhaustive()
    }
}

impl OverlayFs {
    fn validate_path(file_path: &str) -> Result<(), FsError> {
        if !path::validate(file_path) {
            return Err(FsError::InvalidArgument(format!(
                "{file_path}: path contains invalid characters"
            )));
        }
        Ok(())
    }
}

impl VirtualFs for OverlayFs {
    fn read_file(&self, file_path: &str) -> Result<Vec<u8>, FsError> {
        Self::validate_path(file_path)?;
        if self.is_deleted(file_path) {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        if self.memory.exists(file_path) {
            return self.memory.read_file(file_path);
        }
        self.read_real_file(file_path)
    }

    fn read_file_string(&self, file_path: &str) -> Result<String, FsError> {
        Self::validate_path(file_path)?;
        let bytes = self.read_file(file_path)?;
        String::from_utf8(bytes).map_err(|_| {
            FsError::InvalidArgument(format!("{file_path}: not valid UTF-8"))
        })
    }

    fn write_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        Self::validate_path(file_path)?;
        let normalized = path::normalize(file_path);
        let parent = path::parent(&normalized);
        if parent != "/" && !self.memory.exists(parent) {
            self.memory.mkdir(parent, true)?;
        }
        self.unmark_deleted(&normalized);
        self.memory.write_file(file_path, content)
    }

    fn append_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        if self.is_deleted(file_path) {
            return self.write_file(file_path, content);
        }
        if !self.memory.exists(file_path) && self.real_exists(file_path) {
            self.promote_to_memory(file_path)?;
        }
        if self.memory.exists(file_path) {
            return self.memory.append_file(file_path, content);
        }
        self.write_file(file_path, content)
    }

    fn exists(&self, file_path: &str) -> bool {
        if self.is_deleted(file_path) {
            return false;
        }
        if self.memory.exists(file_path) {
            return true;
        }
        self.real_exists(file_path)
    }

    fn stat(&self, file_path: &str) -> Result<Metadata, FsError> {
        Self::validate_path(file_path)?;
        if self.is_deleted(file_path) {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        if self.memory.exists(file_path) {
            return self.memory.stat(file_path);
        }
        self.read_real_metadata(file_path)
    }

    fn lstat(&self, file_path: &str) -> Result<Metadata, FsError> {
        if self.is_deleted(file_path) {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        if self.memory.exists(file_path) {
            return self.memory.lstat(file_path);
        }
        self.read_real_symlink_metadata(file_path)
    }

    fn mkdir(&self, dir_path: &str, recursive: bool) -> Result<(), FsError> {
        self.unmark_deleted(dir_path);
        let normalized = path::normalize(dir_path);
        let parent = path::parent(&normalized);
        if recursive && parent != "/" && !self.memory.exists(parent) {
            self.memory.mkdir(parent, true)?;
        }
        self.memory.mkdir(dir_path, recursive)
    }

    fn readdir(&self, dir_path: &str) -> Result<Vec<DirEntry>, FsError> {
        if self.is_deleted(dir_path) {
            return Err(FsError::NotFound(dir_path.to_string()));
        }

        let mut entries_map = std::collections::BTreeMap::<String, DirEntry>::new();

        if let Ok(real) = self.real_path(dir_path) {
            if real.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&real) {
                    for entry in rd.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let child_vpath = path::join(&path::normalize(dir_path), &name);
                        if self.is_deleted(&child_vpath) {
                            continue;
                        }
                        let ft = if entry.path().is_dir() {
                            FileType::Directory
                        } else if entry.path().is_symlink() {
                            FileType::Symlink
                        } else {
                            FileType::File
                        };
                        entries_map.insert(name.clone(), DirEntry { name, file_type: ft });
                    }
                }
            }
        }

        if let Ok(mem_entries) = self.memory.readdir(dir_path) {
            for e in mem_entries {
                entries_map.insert(e.name.clone(), e);
            }
        } else if entries_map.is_empty() {
            if !self.exists(dir_path) {
                return Err(FsError::NotFound(dir_path.to_string()));
            }
            let meta = self.stat(dir_path)?;
            if !meta.is_dir() {
                return Err(FsError::NotADirectory(dir_path.to_string()));
            }
        }

        Ok(entries_map.into_values().collect())
    }

    fn rm(&self, file_path: &str, recursive: bool, force: bool) -> Result<(), FsError> {
        let normalized = path::normalize(file_path);
        let exists_in_memory = self.memory.exists(&normalized);
        let exists_on_real = !self.is_deleted(&normalized) && self.real_exists(&normalized);

        if !exists_in_memory && !exists_on_real {
            if force {
                return Ok(());
            }
            return Err(FsError::NotFound(file_path.to_string()));
        }

        if !recursive {
            let meta = self.stat(&normalized)?;
            if meta.is_dir() {
                let children = self.readdir(&normalized)?;
                if !children.is_empty() {
                    return Err(FsError::IsADirectory(file_path.to_string()));
                }
            }
        }

        if exists_in_memory {
            let _ = self.memory.rm(&normalized, recursive, true);
        }

        if exists_on_real {
            self.mark_deleted(&normalized);
        }

        Ok(())
    }

    fn cp(&self, src: &str, dest: &str, recursive: bool) -> Result<(), FsError> {
        if self.is_deleted(src) {
            return Err(FsError::NotFound(src.to_string()));
        }

        let meta = self.stat(src)?;

        if meta.is_dir() {
            if !recursive {
                return Err(FsError::IsADirectory(src.to_string()));
            }
            self.mkdir(dest, true)?;
            let children = self.readdir(src)?;
            let src_norm = path::normalize(src);
            let dest_norm = path::normalize(dest);
            for child in children {
                let child_src = path::join(&src_norm, &child.name);
                let child_dest = path::join(&dest_norm, &child.name);
                self.cp(&child_src, &child_dest, true)?;
            }
            return Ok(());
        }

        let content = self.read_file(src)?;
        let dest_norm = path::normalize(dest);

        let final_dest = if self.exists(&dest_norm) && self.stat(&dest_norm)?.is_dir() {
            let src_name = path::basename(src);
            path::join(&dest_norm, src_name)
        } else {
            dest_norm
        };

        self.write_file(&final_dest, &content)
    }

    fn mv(&self, src: &str, dest: &str) -> Result<(), FsError> {
        self.cp(src, dest, true)?;
        self.rm(src, true, false)
    }

    fn chmod(&self, file_path: &str, mode: u32) -> Result<(), FsError> {
        if self.is_deleted(file_path) {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        if !self.memory.exists(file_path) && self.real_exists(file_path) {
            self.promote_to_memory(file_path)?;
        }
        self.memory.chmod(file_path, mode)
    }

    fn symlink(&self, target: &str, link_path: &str) -> Result<(), FsError> {
        let normalized = path::normalize(link_path);
        let parent = path::parent(&normalized);
        if parent != "/" && !self.memory.exists(parent) {
            self.memory.mkdir(parent, true)?;
        }
        self.unmark_deleted(&normalized);
        self.memory.symlink(target, link_path)
    }

    fn hard_link(&self, existing: &str, new_path: &str) -> Result<(), FsError> {
        if self.is_deleted(existing) {
            return Err(FsError::NotFound(existing.to_string()));
        }
        if !self.memory.exists(existing) && self.real_exists(existing) {
            self.promote_to_memory(existing)?;
        }
        self.memory.hard_link(existing, new_path)
    }

    fn readlink(&self, link_path: &str) -> Result<String, FsError> {
        if self.is_deleted(link_path) {
            return Err(FsError::NotFound(link_path.to_string()));
        }
        if self.memory.exists(link_path) {
            return self.memory.readlink(link_path);
        }
        Err(FsError::NotFound(link_path.to_string()))
    }

    fn realpath(&self, file_path: &str) -> Result<String, FsError> {
        if self.is_deleted(file_path) {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        if self.memory.exists(file_path) {
            return self.memory.realpath(file_path);
        }
        if self.real_exists(file_path) {
            return Ok(path::normalize(file_path));
        }
        Err(FsError::NotFound(file_path.to_string()))
    }

    fn touch(&self, file_path: &str) -> Result<(), FsError> {
        if self.is_deleted(file_path) || (!self.memory.exists(file_path) && !self.real_exists(file_path)) {
            self.unmark_deleted(file_path);
            let normalized = path::normalize(file_path);
            let parent = path::parent(&normalized);
            if parent != "/" && !self.memory.exists(parent) {
                self.memory.mkdir(parent, true)?;
            }
            return self.memory.touch(file_path);
        }
        if !self.memory.exists(file_path) {
            self.promote_to_memory(file_path)?;
        }
        self.memory.touch(file_path)
    }

    fn set_times(&self, file_path: &str, mtime: Option<SystemTime>) -> Result<(), FsError> {
        if self.is_deleted(file_path) {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        if !self.memory.exists(file_path) && self.real_exists(file_path) {
            self.promote_to_memory(file_path)?;
        }
        self.memory.set_times(file_path, mtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir_with_files() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("hello.txt"), "hello world").expect("write");
        fs::create_dir(dir.path().join("subdir")).expect("mkdir");
        fs::write(dir.path().join("subdir").join("nested.txt"), "nested content").expect("write");
        dir
    }

    #[test]
    fn read_real_file() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        assert_eq!(ofs.read_file_string("/hello.txt").unwrap(), "hello world");
    }

    #[test]
    fn write_goes_to_memory() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        ofs.write_file("/newfile.txt", b"new content").unwrap();
        assert_eq!(ofs.read_file_string("/newfile.txt").unwrap(), "new content");
        assert!(!dir.path().join("newfile.txt").exists());
    }

    #[test]
    fn overwrite_real_file_in_memory() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        ofs.write_file("/hello.txt", b"overwritten").unwrap();
        assert_eq!(ofs.read_file_string("/hello.txt").unwrap(), "overwritten");
        assert_eq!(
            fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn delete_real_file() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        assert!(ofs.exists("/hello.txt"));
        ofs.rm("/hello.txt", false, false).unwrap();
        assert!(!ofs.exists("/hello.txt"));
        assert!(dir.path().join("hello.txt").exists());
    }

    #[test]
    fn readdir_merges_layers() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        ofs.write_file("/extra.txt", b"extra").unwrap();
        let entries = ofs.readdir("/").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello.txt"));
        assert!(names.contains(&"extra.txt"));
        assert!(names.contains(&"subdir"));
    }

    #[test]
    fn readdir_excludes_deleted() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        ofs.rm("/hello.txt", false, false).unwrap();
        let entries = ofs.readdir("/").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"hello.txt"));
    }

    #[test]
    fn stat_real_file() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        let meta = ofs.stat("/hello.txt").unwrap();
        assert!(meta.is_file());
    }

    #[test]
    fn stat_deleted_file_fails() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        ofs.rm("/hello.txt", false, false).unwrap();
        assert!(ofs.stat("/hello.txt").is_err());
    }

    #[test]
    fn nested_real_read() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        assert_eq!(
            ofs.read_file_string("/subdir/nested.txt").unwrap(),
            "nested content"
        );
    }

    #[test]
    fn chmod_promotes_to_memory() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        ofs.chmod("/hello.txt", 0o755).unwrap();
        assert_eq!(ofs.stat("/hello.txt").unwrap().mode, 0o755);
    }

    #[test]
    fn touch_creates_new_file() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        ofs.touch("/brand_new.txt").unwrap();
        assert!(ofs.exists("/brand_new.txt"));
        assert!(!dir.path().join("brand_new.txt").exists());
    }

    #[test]
    fn path_escape_prevented() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        assert!(ofs.read_file("/../../etc/passwd").is_err());
    }

    #[test]
    fn invalid_root_fails() {
        let result = OverlayFs::new("/nonexistent_dir_12345");
        assert!(result.is_err());
    }

    #[test]
    fn null_byte_path_rejected() {
        let dir = temp_dir_with_files();
        let ofs = OverlayFs::new(dir.path()).unwrap();
        assert!(ofs.read_file("/hello\0.txt").is_err());
        assert!(ofs.write_file("/evil\0.txt", b"data").is_err());
        assert!(ofs.stat("/null\0byte").is_err());
    }
}
