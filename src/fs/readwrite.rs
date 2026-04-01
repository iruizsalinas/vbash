use std::path::PathBuf;
use std::time::SystemTime;

use crate::error::FsError;
use crate::fs::path;
use crate::fs::types::{
    DirEntry, FileType, Metadata, DEFAULT_DIR_MODE, DEFAULT_FILE_MODE,
};
use crate::fs::VirtualFs;

const DEFAULT_MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Direct read-write access to a real directory on the host filesystem.
///
/// All operations go to disk. Paths are resolved relative to the configured
/// root and validated to prevent sandbox escape.
pub struct ReadWriteFs {
    root: PathBuf,
    max_file_size: usize,
}

impl ReadWriteFs {
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
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        })
    }

    /// Set the maximum file size that can be read (default: 10 MB).
    #[must_use]
    pub fn with_max_file_size(mut self, size: usize) -> Self {
        self.max_file_size = size;
        self
    }

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
        if !resolved.starts_with(&self.root) {
            return Err(FsError::PermissionDenied(format!(
                "{virtual_path}: path escapes sandbox"
            )));
        }
        Ok(resolved)
    }
}

fn host_metadata(meta: &std::fs::Metadata) -> Metadata {
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

impl std::fmt::Debug for ReadWriteFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadWriteFs")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl VirtualFs for ReadWriteFs {
    fn read_file(&self, file_path: &str) -> Result<Vec<u8>, FsError> {
        let real = self.real_path(file_path)?;
        if real.is_dir() {
            return Err(FsError::IsADirectory(file_path.to_string()));
        }
        let meta = std::fs::metadata(&real).map_err(|_| {
            FsError::NotFound(file_path.to_string())
        })?;
        if meta.len() > self.max_file_size as u64 {
            return Err(FsError::TooLarge(file_path.to_string()));
        }
        std::fs::read(&real).map_err(|_| FsError::NotFound(file_path.to_string()))
    }

    fn read_file_string(&self, file_path: &str) -> Result<String, FsError> {
        let bytes = self.read_file(file_path)?;
        String::from_utf8(bytes).map_err(|_| {
            FsError::InvalidArgument(format!("{file_path}: not valid UTF-8"))
        })
    }

    fn write_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        let real = self.real_path(file_path)?;
        if real.is_dir() {
            return Err(FsError::IsADirectory(file_path.to_string()));
        }
        if let Some(parent) = real.parent() {
            if !parent.exists() {
                return Err(FsError::NotFound(
                    path::parent(&path::normalize(file_path)).to_string(),
                ));
            }
        }
        std::fs::write(&real, content).map_err(|_| {
            FsError::PermissionDenied(file_path.to_string())
        })
    }

    fn append_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        use std::io::Write;
        let real = self.real_path(file_path)?;
        if real.is_dir() {
            return Err(FsError::IsADirectory(file_path.to_string()));
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&real)
            .map_err(|_| FsError::PermissionDenied(file_path.to_string()))?;
        file.write_all(content).map_err(|_| {
            FsError::PermissionDenied(file_path.to_string())
        })
    }

    fn exists(&self, file_path: &str) -> bool {
        self.real_path(file_path)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    fn stat(&self, file_path: &str) -> Result<Metadata, FsError> {
        let real = self.real_path(file_path)?;
        let meta = std::fs::metadata(&real).map_err(|_| {
            FsError::NotFound(file_path.to_string())
        })?;
        Ok(host_metadata(&meta))
    }

    fn lstat(&self, file_path: &str) -> Result<Metadata, FsError> {
        let real = self.real_path(file_path)?;
        let meta = std::fs::symlink_metadata(&real).map_err(|_| {
            FsError::NotFound(file_path.to_string())
        })?;
        Ok(host_metadata(&meta))
    }

    fn mkdir(&self, dir_path: &str, recursive: bool) -> Result<(), FsError> {
        let real = self.real_path(dir_path)?;
        if real.exists() {
            return Err(FsError::AlreadyExists(dir_path.to_string()));
        }
        let result = if recursive {
            std::fs::create_dir_all(&real)
        } else {
            std::fs::create_dir(&real)
        };
        result.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FsError::NotFound(path::parent(&path::normalize(dir_path)).to_string())
            } else {
                FsError::PermissionDenied(dir_path.to_string())
            }
        })
    }

    fn readdir(&self, dir_path: &str) -> Result<Vec<DirEntry>, FsError> {
        let real = self.real_path(dir_path)?;
        if !real.is_dir() {
            if real.exists() {
                return Err(FsError::NotADirectory(dir_path.to_string()));
            }
            return Err(FsError::NotFound(dir_path.to_string()));
        }
        let rd = std::fs::read_dir(&real).map_err(|_| {
            FsError::PermissionDenied(dir_path.to_string())
        })?;
        let mut entries = Vec::new();
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let ft = if entry.path().is_dir() {
                FileType::Directory
            } else if entry.path().is_symlink() {
                FileType::Symlink
            } else {
                FileType::File
            };
            entries.push(DirEntry { name, file_type: ft });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn rm(&self, file_path: &str, recursive: bool, force: bool) -> Result<(), FsError> {
        let real = match self.real_path(file_path) {
            Ok(r) => r,
            Err(_) if force => return Ok(()),
            Err(e) => return Err(e),
        };
        if !real.exists() {
            if force {
                return Ok(());
            }
            return Err(FsError::NotFound(file_path.to_string()));
        }
        if real.is_dir() {
            if recursive {
                std::fs::remove_dir_all(&real).map_err(|_| {
                    FsError::PermissionDenied(file_path.to_string())
                })
            } else {
                std::fs::remove_dir(&real).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::DirectoryNotEmpty {
                        FsError::IsADirectory(file_path.to_string())
                    } else {
                        FsError::PermissionDenied(file_path.to_string())
                    }
                })
            }
        } else {
            std::fs::remove_file(&real).map_err(|_| {
                FsError::PermissionDenied(file_path.to_string())
            })
        }
    }

    fn cp(&self, src: &str, dest: &str, recursive: bool) -> Result<(), FsError> {
        let real_src = self.real_path(src)?;
        if !real_src.exists() {
            return Err(FsError::NotFound(src.to_string()));
        }
        if real_src.is_dir() {
            if !recursive {
                return Err(FsError::IsADirectory(src.to_string()));
            }
            return self.cp_dir_recursive(src, dest);
        }
        let content = std::fs::read(&real_src).map_err(|_| {
            FsError::NotFound(src.to_string())
        })?;
        let real_dest = self.real_path(dest)?;
        let final_dest = if real_dest.is_dir() {
            let name = real_src
                .file_name()
                .ok_or_else(|| FsError::InvalidArgument(src.to_string()))?;
            real_dest.join(name)
        } else {
            real_dest
        };
        std::fs::write(&final_dest, content).map_err(|_| {
            FsError::PermissionDenied(dest.to_string())
        })
    }

    fn mv(&self, src: &str, dest: &str) -> Result<(), FsError> {
        let real_src = self.real_path(src)?;
        if !real_src.exists() {
            return Err(FsError::NotFound(src.to_string()));
        }
        let real_dest = self.real_path(dest)?;
        let final_dest = if real_dest.is_dir() {
            let name = real_src
                .file_name()
                .ok_or_else(|| FsError::InvalidArgument(src.to_string()))?;
            real_dest.join(name)
        } else {
            real_dest
        };
        std::fs::rename(&real_src, &final_dest).map_err(|_| {
            FsError::PermissionDenied(src.to_string())
        })
    }

    fn chmod(&self, file_path: &str, _mode: u32) -> Result<(), FsError> {
        let real = self.real_path(file_path)?;
        if !real.exists() {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(_mode);
            std::fs::set_permissions(&real, perms).map_err(|_| {
                FsError::PermissionDenied(file_path.to_string())
            })?;
        }
        Ok(())
    }

    fn symlink(&self, target: &str, link_path: &str) -> Result<(), FsError> {
        // Validate the symlink target stays within the sandbox.
        // Even though reads through symlinks are caught by canonicalize() in
        // real_path(), we must not create real symlink artifacts on disk that
        // point outside the sandbox root.
        let resolved_target = if target.starts_with('/') {
            // Absolute virtual target: resolve against sandbox root
            self.real_path(target)?
        } else {
            // Relative target: resolve against link's parent directory
            let normalized = path::normalize(link_path);
            let link_parent = path::parent(&normalized);
            let absolute_target = path::join(link_parent, target);
            self.real_path(&absolute_target)?
        };
        // Verify the resolved target is within the sandbox
        if !resolved_target.starts_with(&self.root) {
            return Err(FsError::PermissionDenied(format!(
                "{target}: symlink target escapes sandbox"
            )));
        }

        let real_link = self.real_path(link_path)?;
        if real_link.exists() {
            return Err(FsError::AlreadyExists(link_path.to_string()));
        }
        // Use the relative target as-is so the symlink works within the sandbox
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, &real_link).map_err(|_| {
                FsError::PermissionDenied(link_path.to_string())
            })?;
        }
        #[cfg(windows)]
        {
            let target_real = self.real_path(target).unwrap_or_else(|_| PathBuf::from(target));
            if target_real.is_dir() {
                std::os::windows::fs::symlink_dir(target, &real_link)
            } else {
                std::os::windows::fs::symlink_file(target, &real_link)
            }
            .map_err(|_| FsError::PermissionDenied(link_path.to_string()))?;
        }
        Ok(())
    }

    fn hard_link(&self, existing: &str, new_path: &str) -> Result<(), FsError> {
        let real_existing = self.real_path(existing)?;
        if !real_existing.exists() {
            return Err(FsError::NotFound(existing.to_string()));
        }
        if real_existing.is_dir() {
            return Err(FsError::InvalidArgument(format!(
                "{existing}: hard link not allowed for directory"
            )));
        }
        let real_new = self.real_path(new_path)?;
        if real_new.exists() {
            return Err(FsError::AlreadyExists(new_path.to_string()));
        }
        std::fs::hard_link(&real_existing, &real_new).map_err(|_| {
            FsError::PermissionDenied(new_path.to_string())
        })
    }

    fn readlink(&self, link_path: &str) -> Result<String, FsError> {
        let real = self.real_path(link_path)?;
        let target = std::fs::read_link(&real).map_err(|_| {
            FsError::InvalidArgument(format!("{link_path}: not a symbolic link"))
        })?;
        Ok(target.to_string_lossy().to_string())
    }

    fn realpath(&self, file_path: &str) -> Result<String, FsError> {
        let real = self.real_path(file_path)?;
        if !real.exists() {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        Ok(path::normalize(file_path))
    }

    fn touch(&self, file_path: &str) -> Result<(), FsError> {
        let real = self.real_path(file_path)?;
        if real.exists() {
            let now = filetime::FileTime::now();
            filetime::set_file_mtime(&real, now).map_err(|_| {
                FsError::PermissionDenied(file_path.to_string())
            })?;
            return Ok(());
        }
        std::fs::write(&real, []).map_err(|_| {
            FsError::PermissionDenied(file_path.to_string())
        })
    }

    fn set_times(&self, file_path: &str, mtime: Option<SystemTime>) -> Result<(), FsError> {
        let real = self.real_path(file_path)?;
        if !real.exists() {
            return Err(FsError::NotFound(file_path.to_string()));
        }
        let time = mtime.unwrap_or_else(SystemTime::now);
        let ft = filetime::FileTime::from_system_time(time);
        filetime::set_file_mtime(&real, ft).map_err(|_| {
            FsError::PermissionDenied(file_path.to_string())
        })
    }
}

impl ReadWriteFs {
    fn cp_dir_recursive(&self, src: &str, dest: &str) -> Result<(), FsError> {
        self.mkdir(dest, true)?;
        let entries = self.readdir(src)?;
        let src_norm = path::normalize(src);
        let dest_norm = path::normalize(dest);
        for entry in entries {
            let child_src = path::join(&src_norm, &entry.name);
            let child_dest = path::join(&dest_norm, &entry.name);
            self.cp(&child_src, &child_dest, true)?;
        }
        Ok(())
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
        fs::write(
            dir.path().join("subdir").join("nested.txt"),
            "nested content",
        )
        .expect("write");
        dir
    }

    #[test]
    fn read_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        assert_eq!(rw.read_file_string("/hello.txt").unwrap(), "hello world");
    }

    #[test]
    fn write_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.write_file("/new.txt", b"new content").unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "new content"
        );
    }

    #[test]
    fn exists_check() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        assert!(rw.exists("/hello.txt"));
        assert!(!rw.exists("/no_such_file.txt"));
    }

    #[test]
    fn stat_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        let meta = rw.stat("/hello.txt").unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.size, 11);
    }

    #[test]
    fn stat_dir() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        let meta = rw.stat("/subdir").unwrap();
        assert!(meta.is_dir());
    }

    #[test]
    fn mkdir_and_readdir() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.mkdir("/newdir", false).unwrap();
        assert!(dir.path().join("newdir").is_dir());
        let entries = rw.readdir("/").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"newdir"));
    }

    #[test]
    fn rm_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.rm("/hello.txt", false, false).unwrap();
        assert!(!dir.path().join("hello.txt").exists());
    }

    #[test]
    fn rm_dir_recursive() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.rm("/subdir", true, false).unwrap();
        assert!(!dir.path().join("subdir").exists());
    }

    #[test]
    fn cp_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.cp("/hello.txt", "/copy.txt", false).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("copy.txt")).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn mv_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.mv("/hello.txt", "/moved.txt").unwrap();
        assert!(!dir.path().join("hello.txt").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("moved.txt")).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn append_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.append_file("/hello.txt", b" appended").unwrap();
        assert_eq!(
            rw.read_file_string("/hello.txt").unwrap(),
            "hello world appended"
        );
    }

    #[test]
    fn nested_read() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        assert_eq!(
            rw.read_file_string("/subdir/nested.txt").unwrap(),
            "nested content"
        );
    }

    #[test]
    fn path_escape_blocked() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        assert!(rw.read_file("/../../etc/passwd").is_err());
    }

    #[test]
    fn invalid_root_fails() {
        let result = ReadWriteFs::new("/nonexistent_dir_12345");
        assert!(result.is_err());
    }

    #[test]
    fn touch_creates_file() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        rw.touch("/brand_new.txt").unwrap();
        assert!(dir.path().join("brand_new.txt").exists());
    }

    #[test]
    fn realpath_existing() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        assert_eq!(rw.realpath("/hello.txt").unwrap(), "/hello.txt");
    }

    #[test]
    fn realpath_nonexistent_fails() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        assert!(rw.realpath("/nope.txt").is_err());
    }

    #[test]
    fn null_byte_path_rejected() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        assert!(rw.read_file("/hello\0.txt").is_err());
        assert!(rw.write_file("/evil\0.txt", b"data").is_err());
        assert!(rw.stat("/null\0byte").is_err());
    }

    #[test]
    fn read_file_too_large_rejected() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap().with_max_file_size(5);
        // "hello world" is 11 bytes, limit is 5
        let result = rw.read_file("/hello.txt");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, FsError::TooLarge(_)));
    }

    #[test]
    fn read_file_within_limit_ok() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap().with_max_file_size(100);
        assert_eq!(rw.read_file_string("/hello.txt").unwrap(), "hello world");
    }

    #[test]
    fn symlink_escape_blocked() {
        let dir = temp_dir_with_files();
        let rw = ReadWriteFs::new(dir.path()).unwrap();
        // Attempting to create a symlink pointing outside the sandbox must fail
        let result = rw.symlink("/etc/passwd", "/evil_link");
        assert!(result.is_err());
    }
}
