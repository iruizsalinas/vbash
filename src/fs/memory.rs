//! In-memory filesystem. No disk I/O.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::SystemTime;

use crate::error::FsError;
use crate::fs::VirtualFs;
use crate::fs::path;
use crate::fs::types::{
    DirData, DirEntry, FsEntry, FileData, Metadata, SymlinkData, DEFAULT_DIR_MODE,
    DEFAULT_FILE_MODE, MAX_SYMLINK_DEPTH, SYMLINK_MODE,
};

/// A pure in-memory filesystem. No disk I/O whatsoever.
///
/// ```
/// use vbash::InMemoryFs;
/// use vbash::VirtualFs;
///
/// let fs = InMemoryFs::new();
/// fs.mkdir("/tmp", false).unwrap();
/// fs.write_file("/tmp/hello.txt", b"hello world").unwrap();
///
/// let content = fs.read_file_string("/tmp/hello.txt").unwrap();
/// assert_eq!(content, "hello world");
/// ```
pub struct InMemoryFs {
    entries: RefCell<BTreeMap<String, FsEntry>>,
}

impl InMemoryFs {
    pub fn new() -> Self {
        let mut entries = BTreeMap::new();
        entries.insert(
            "/".to_string(),
            FsEntry::Directory(DirData {
                mode: DEFAULT_DIR_MODE,
                mtime: SystemTime::now(),
            }),
        );
        Self {
            entries: RefCell::new(entries),
        }
    }

    /// Resolve a path, following all symlinks in every component.
    fn resolve_path(entries: &BTreeMap<String, FsEntry>, path: &str) -> Result<String, FsError> {
        Self::resolve_path_inner(entries, path, 0, true)
    }

    /// Resolve all symlinks in directory components, but not the final component.
    /// Used by `lstat` and `readlink`.
    fn resolve_parent_symlinks(
        entries: &BTreeMap<String, FsEntry>,
        abs_path: &str,
    ) -> Result<String, FsError> {
        if abs_path == "/" {
            return Ok("/".to_string());
        }
        let parent = path::parent(abs_path);
        let name = path::basename(abs_path);
        let resolved_parent = Self::resolve_path_inner(entries, parent, 0, true)?;
        Ok(path::join(&resolved_parent, name))
    }

    fn resolve_path_inner(
        entries: &BTreeMap<String, FsEntry>,
        abs_path: &str,
        depth: u32,
        follow_final: bool,
    ) -> Result<String, FsError> {
        if depth > MAX_SYMLINK_DEPTH {
            return Err(FsError::SymlinkLoop(abs_path.to_string()));
        }

        let normalized = path::normalize(abs_path);
        if normalized == "/" {
            return Ok("/".to_string());
        }

        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
        let mut resolved = String::from("/");

        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;
            let candidate = path::join(&resolved, part);

            match entries.get(&candidate) {
                Some(FsEntry::Symlink(sym)) => {
                    if is_last && !follow_final {
                        resolved = candidate;
                    } else {
                        let target = if sym.target.starts_with('/') {
                            sym.target.clone()
                        } else {
                            path::join(&resolved, &sym.target)
                        };
                        resolved =
                            Self::resolve_path_inner(entries, &target, depth + 1, true)?;
                    }
                }
                Some(FsEntry::Directory(_)) => {
                    resolved = candidate;
                }
                Some(FsEntry::File(_)) => {
                    if is_last {
                        resolved = candidate;
                    } else {
                        return Err(FsError::NotADirectory(candidate));
                    }
                }
                None => {
                    if is_last {
                        resolved = candidate;
                    } else {
                        return Err(FsError::NotFound(candidate));
                    }
                }
            }
        }

        Ok(resolved)
    }

    /// Collect all direct children of a directory path.
    fn children_of<'a>(
        entries: &'a BTreeMap<String, FsEntry>,
        dir_path: &str,
    ) -> Vec<(&'a str, &'a FsEntry)> {
        let prefix = if dir_path == "/" {
            "/".to_string()
        } else {
            format!("{dir_path}/")
        };

        entries
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .filter(|(k, _)| {
                let rest = &k[prefix.len()..];
                !rest.contains('/')
            })
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }

    fn now() -> SystemTime {
        SystemTime::now()
    }
}

impl Default for InMemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for InMemoryFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries = self.entries.borrow();
        f.debug_struct("InMemoryFs")
            .field("entry_count", &entries.len())
            .finish()
    }
}

impl VirtualFs for InMemoryFs {
    fn read_file(&self, file_path: &str) -> Result<Vec<u8>, FsError> {
        let entries = self.entries.borrow();
        let resolved = Self::resolve_path(&entries, file_path)?;
        match entries.get(&resolved) {
            Some(FsEntry::File(f)) => Ok(f.content.clone()),
            Some(FsEntry::Directory(_)) => Err(FsError::IsADirectory(file_path.to_string())),
            Some(FsEntry::Symlink(_)) | None => Err(FsError::NotFound(file_path.to_string())),
        }
    }

    fn read_file_string(&self, file_path: &str) -> Result<String, FsError> {
        let bytes = self.read_file(file_path)?;
        String::from_utf8(bytes).map_err(|_| {
            FsError::InvalidArgument(format!("{file_path}: not valid UTF-8"))
        })
    }

    fn write_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        let normalized = path::normalize(file_path);
        let mut entries = self.entries.borrow_mut();
        let resolved = Self::resolve_path_inner(&entries, &normalized, 0, true)?;

        let parent = path::parent(&resolved);
        match entries.get(parent) {
            Some(FsEntry::Directory(_)) => {}
            Some(_) => return Err(FsError::NotADirectory(parent.to_string())),
            None => return Err(FsError::NotFound(parent.to_string())),
        }

        if let Some(FsEntry::Directory(_)) = entries.get(&resolved) {
            return Err(FsError::IsADirectory(file_path.to_string()));
        }

        // 100MB per-file write cap to prevent memory exhaustion
        if content.len() > 100 * 1024 * 1024 {
            return Err(FsError::TooLarge(file_path.to_string()));
        }

        entries.insert(
            resolved,
            FsEntry::File(FileData {
                content: content.to_vec(),
                mode: DEFAULT_FILE_MODE,
                mtime: Self::now(),
            }),
        );
        Ok(())
    }

    fn append_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        let mut entries = self.entries.borrow_mut();
        let resolved = Self::resolve_path(&entries, file_path)?;

        match entries.get_mut(&resolved) {
            Some(FsEntry::File(f)) => {
                f.content.extend_from_slice(content);
                f.mtime = Self::now();
                Ok(())
            }
            Some(FsEntry::Directory(_)) => Err(FsError::IsADirectory(file_path.to_string())),
            _ => {
                drop(entries);
                self.write_file(file_path, content)
            }
        }
    }

    fn exists(&self, file_path: &str) -> bool {
        let entries = self.entries.borrow();
        let Ok(resolved) = Self::resolve_path(&entries, file_path) else {
            return false;
        };
        entries.contains_key(&resolved)
    }

    fn stat(&self, file_path: &str) -> Result<Metadata, FsError> {
        let entries = self.entries.borrow();
        let resolved = Self::resolve_path(&entries, file_path)?;
        entries
            .get(&resolved)
            .map(FsEntry::metadata)
            .ok_or_else(|| FsError::NotFound(file_path.to_string()))
    }

    fn lstat(&self, file_path: &str) -> Result<Metadata, FsError> {
        let entries = self.entries.borrow();
        let normalized = path::normalize(file_path);
        let resolved = Self::resolve_parent_symlinks(&entries, &normalized)?;
        entries
            .get(&resolved)
            .map(FsEntry::metadata)
            .ok_or_else(|| FsError::NotFound(file_path.to_string()))
    }

    fn mkdir(&self, dir_path: &str, recursive: bool) -> Result<(), FsError> {
        let normalized = path::normalize(dir_path);
        let mut entries = self.entries.borrow_mut();

        if recursive {
            let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
            let mut current = String::new();
            for part in parts {
                current = format!("{current}/{part}");
                if !entries.contains_key(&current) {
                    entries.insert(
                        current.clone(),
                        FsEntry::Directory(DirData {
                            mode: DEFAULT_DIR_MODE,
                            mtime: Self::now(),
                        }),
                    );
                }
            }
            Ok(())
        } else {
            let parent = path::parent(&normalized);
            match entries.get(parent) {
                Some(FsEntry::Directory(_)) => {}
                Some(_) => return Err(FsError::NotADirectory(parent.to_string())),
                None => return Err(FsError::NotFound(parent.to_string())),
            }

            if entries.contains_key(&normalized) {
                return Err(FsError::AlreadyExists(dir_path.to_string()));
            }

            entries.insert(
                normalized,
                FsEntry::Directory(DirData {
                    mode: DEFAULT_DIR_MODE,
                    mtime: Self::now(),
                }),
            );
            Ok(())
        }
    }

    fn readdir(&self, dir_path: &str) -> Result<Vec<DirEntry>, FsError> {
        let entries = self.entries.borrow();
        let resolved = Self::resolve_path(&entries, dir_path)?;

        match entries.get(&resolved) {
            Some(FsEntry::Directory(_)) => {}
            Some(_) => return Err(FsError::NotADirectory(dir_path.to_string())),
            None => return Err(FsError::NotFound(dir_path.to_string())),
        }

        let children = Self::children_of(&entries, &resolved);
        Ok(children
            .into_iter()
            .map(|(full_path, entry)| DirEntry {
                name: path::basename(full_path).to_string(),
                file_type: entry.file_type(),
            })
            .collect())
    }

    fn rm(&self, file_path: &str, recursive: bool, force: bool) -> Result<(), FsError> {
        let mut entries = self.entries.borrow_mut();
        let normalized = path::normalize(file_path);
        let resolved = match Self::resolve_path_inner(&entries, &normalized, 0, true) {
            Ok(r) => r,
            Err(_) if force => return Ok(()),
            Err(e) => return Err(e),
        };

        match entries.get(&resolved) {
            None if force => return Ok(()),
            None => return Err(FsError::NotFound(file_path.to_string())),
            Some(FsEntry::Directory(_)) => {
                if !recursive {
                    let children = Self::children_of(&entries, &resolved);
                    if !children.is_empty() {
                        return Err(FsError::IsADirectory(file_path.to_string()));
                    }
                }
            }
            Some(_) => {}
        }

        if recursive {
            let prefix = if resolved == "/" {
                "/".to_string()
            } else {
                format!("{resolved}/")
            };
            let to_remove: Vec<String> = entries
                .range(prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&prefix))
                .map(|(k, _)| k.clone())
                .collect();
            for key in to_remove {
                entries.remove(&key);
            }
        }

        entries.remove(&resolved);
        Ok(())
    }

    fn cp(&self, src: &str, dest: &str, recursive: bool) -> Result<(), FsError> {
        let entries = self.entries.borrow();
        let resolved_src = Self::resolve_path(&entries, src)?;

        let src_entry = entries
            .get(&resolved_src)
            .ok_or_else(|| FsError::NotFound(src.to_string()))?;

        match src_entry {
            FsEntry::Directory(_) => {
                if !recursive {
                    return Err(FsError::IsADirectory(src.to_string()));
                }
                let prefix = format!("{resolved_src}/");
                let to_copy: Vec<(String, FsEntry)> = entries
                    .range(prefix.clone()..)
                    .take_while(|(k, _)| k.starts_with(&prefix))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let src_dir_entry = src_entry.clone();
                drop(entries);

                let dest_normalized = path::normalize(dest);
                let mut entries = self.entries.borrow_mut();

                entries.insert(dest_normalized.clone(), src_dir_entry);

                for (child_path, child_entry) in to_copy {
                    let relative = &child_path[resolved_src.len()..];
                    let new_path = format!("{dest_normalized}{relative}");
                    entries.insert(new_path, child_entry);
                }
                Ok(())
            }
            FsEntry::File(f) => {
                let file_data = f.clone();
                drop(entries);

                let dest_normalized = path::normalize(dest);
                let mut entries = self.entries.borrow_mut();

                let final_dest = if entries
                    .get(&dest_normalized)
                    .is_some_and(|e| matches!(e, FsEntry::Directory(_)))
                {
                    path::join(&dest_normalized, path::basename(&resolved_src))
                } else {
                    dest_normalized
                };

                entries.insert(
                    final_dest,
                    FsEntry::File(FileData {
                        content: file_data.content,
                        mode: file_data.mode,
                        mtime: Self::now(),
                    }),
                );
                Ok(())
            }
            FsEntry::Symlink(s) => {
                let sym_data = s.clone();
                drop(entries);

                let dest_normalized = path::normalize(dest);
                let mut entries = self.entries.borrow_mut();
                entries.insert(
                    dest_normalized,
                    FsEntry::Symlink(SymlinkData {
                        target: sym_data.target,
                        mode: sym_data.mode,
                        mtime: Self::now(),
                    }),
                );
                Ok(())
            }
        }
    }

    fn mv(&self, src: &str, dest: &str) -> Result<(), FsError> {
        self.cp(src, dest, true)?;
        self.rm(src, true, false)
    }

    fn chmod(&self, file_path: &str, mode: u32) -> Result<(), FsError> {
        let mut entries = self.entries.borrow_mut();
        let resolved = Self::resolve_path(&entries, file_path)?;
        match entries.get_mut(&resolved) {
            Some(FsEntry::File(f)) => {
                f.mode = mode;
                Ok(())
            }
            Some(FsEntry::Directory(d)) => {
                d.mode = mode;
                Ok(())
            }
            Some(FsEntry::Symlink(_)) => Ok(()),
            None => Err(FsError::NotFound(file_path.to_string())),
        }
    }

    fn symlink(&self, target: &str, link_path: &str) -> Result<(), FsError> {
        let normalized = path::normalize(link_path);
        let mut entries = self.entries.borrow_mut();

        let parent = path::parent(&normalized);
        match entries.get(parent) {
            Some(FsEntry::Directory(_)) => {}
            Some(_) => return Err(FsError::NotADirectory(parent.to_string())),
            None => return Err(FsError::NotFound(parent.to_string())),
        }

        if entries.contains_key(&normalized) {
            return Err(FsError::AlreadyExists(link_path.to_string()));
        }

        entries.insert(
            normalized,
            FsEntry::Symlink(SymlinkData {
                target: target.to_string(),
                mode: SYMLINK_MODE,
                mtime: Self::now(),
            }),
        );
        Ok(())
    }

    fn hard_link(&self, existing: &str, new_path: &str) -> Result<(), FsError> {
        let entries_ref = self.entries.borrow();
        let resolved = Self::resolve_path(&entries_ref, existing)?;

        let file_data = match entries_ref.get(&resolved) {
            Some(FsEntry::File(f)) => f.clone(),
            Some(FsEntry::Directory(_)) => {
                return Err(FsError::InvalidArgument(format!(
                    "{existing}: hard link not allowed for directory"
                )));
            }
            _ => return Err(FsError::NotFound(existing.to_string())),
        };
        drop(entries_ref);

        let new_normalized = path::normalize(new_path);
        let mut entries = self.entries.borrow_mut();

        if entries.contains_key(&new_normalized) {
            return Err(FsError::AlreadyExists(new_path.to_string()));
        }

        entries.insert(new_normalized, FsEntry::File(file_data));
        Ok(())
    }

    fn readlink(&self, link_path: &str) -> Result<String, FsError> {
        let entries = self.entries.borrow();
        let normalized = path::normalize(link_path);
        let resolved = Self::resolve_parent_symlinks(&entries, &normalized)?;

        match entries.get(&resolved) {
            Some(FsEntry::Symlink(s)) => Ok(s.target.clone()),
            Some(_) => Err(FsError::InvalidArgument(format!(
                "{link_path}: not a symbolic link"
            ))),
            None => Err(FsError::NotFound(link_path.to_string())),
        }
    }

    fn realpath(&self, file_path: &str) -> Result<String, FsError> {
        let entries = self.entries.borrow();
        let resolved = Self::resolve_path(&entries, file_path)?;
        if entries.contains_key(&resolved) {
            Ok(resolved)
        } else {
            Err(FsError::NotFound(file_path.to_string()))
        }
    }

    fn touch(&self, file_path: &str) -> Result<(), FsError> {
        let normalized = path::normalize(file_path);
        let mut entries = self.entries.borrow_mut();

        let resolved = Self::resolve_path_inner(&entries, &normalized, 0, true)?;
        if let Some(entry) = entries.get_mut(&resolved) {
            match entry {
                FsEntry::File(f) => f.mtime = Self::now(),
                FsEntry::Directory(d) => d.mtime = Self::now(),
                FsEntry::Symlink(_) => {}
            }
            return Ok(());
        }

        let parent = path::parent(&resolved);
        match entries.get(parent) {
            Some(FsEntry::Directory(_)) => {}
            Some(_) => return Err(FsError::NotADirectory(parent.to_string())),
            None => return Err(FsError::NotFound(parent.to_string())),
        }

        entries.insert(
            resolved,
            FsEntry::File(FileData {
                content: Vec::new(),
                mode: DEFAULT_FILE_MODE,
                mtime: Self::now(),
            }),
        );
        Ok(())
    }

    fn set_times(&self, file_path: &str, mtime: Option<SystemTime>) -> Result<(), FsError> {
        let mut entries = self.entries.borrow_mut();
        let resolved = Self::resolve_path(&entries, file_path)?;

        let time = mtime.unwrap_or_else(Self::now);
        match entries.get_mut(&resolved) {
            Some(FsEntry::File(f)) => {
                f.mtime = time;
                Ok(())
            }
            Some(FsEntry::Directory(d)) => {
                d.mtime = time;
                Ok(())
            }
            Some(FsEntry::Symlink(_)) => Ok(()),
            None => Err(FsError::NotFound(file_path.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_read_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/tmp", false).unwrap();
        fs.write_file("/tmp/test.txt", b"hello").unwrap();
        assert_eq!(fs.read_file_string("/tmp/test.txt").unwrap(), "hello");
    }

    #[test]
    fn read_nonexistent_file() {
        let fs = InMemoryFs::new();
        assert!(fs.read_file("/nope.txt").is_err());
    }

    #[test]
    fn write_creates_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/a.txt", b"content").unwrap();
        assert!(fs.exists("/dir/a.txt"));
    }

    #[test]
    fn overwrite_existing_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/a.txt", b"old").unwrap();
        fs.write_file("/dir/a.txt", b"new").unwrap();
        assert_eq!(fs.read_file_string("/dir/a.txt").unwrap(), "new");
    }

    #[test]
    fn append_to_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/a.txt", b"hello").unwrap();
        fs.append_file("/dir/a.txt", b" world").unwrap();
        assert_eq!(fs.read_file_string("/dir/a.txt").unwrap(), "hello world");
    }

    #[test]
    fn append_creates_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.append_file("/dir/new.txt", b"created").unwrap();
        assert_eq!(fs.read_file_string("/dir/new.txt").unwrap(), "created");
    }

    #[test]
    fn mkdir_simple() {
        let fs = InMemoryFs::new();
        fs.mkdir("/newdir", false).unwrap();
        assert!(fs.exists("/newdir"));
        assert!(fs.stat("/newdir").unwrap().is_dir());
    }

    #[test]
    fn mkdir_recursive() {
        let fs = InMemoryFs::new();
        fs.mkdir("/a/b/c/d", true).unwrap();
        assert!(fs.exists("/a"));
        assert!(fs.exists("/a/b"));
        assert!(fs.exists("/a/b/c"));
        assert!(fs.exists("/a/b/c/d"));
    }

    #[test]
    fn mkdir_no_parent_fails() {
        let fs = InMemoryFs::new();
        assert!(fs.mkdir("/no/parent", false).is_err());
    }

    #[test]
    fn mkdir_already_exists() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        assert!(fs.mkdir("/dir", false).is_err());
    }

    #[test]
    fn readdir_lists_children() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/a.txt", b"").unwrap();
        fs.write_file("/dir/b.txt", b"").unwrap();
        fs.mkdir("/dir/sub", false).unwrap();

        let mut names: Vec<String> = fs
            .readdir("/dir")
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["a.txt", "b.txt", "sub"]);
    }

    #[test]
    fn rm_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/file.txt", b"data").unwrap();
        fs.rm("/dir/file.txt", false, false).unwrap();
        assert!(!fs.exists("/dir/file.txt"));
    }

    #[test]
    fn rm_directory_recursive() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir/sub", true).unwrap();
        fs.write_file("/dir/sub/file.txt", b"data").unwrap();
        fs.rm("/dir", true, false).unwrap();
        assert!(!fs.exists("/dir"));
        assert!(!fs.exists("/dir/sub"));
        assert!(!fs.exists("/dir/sub/file.txt"));
    }

    #[test]
    fn rm_force_nonexistent() {
        let fs = InMemoryFs::new();
        assert!(fs.rm("/nope", false, true).is_ok());
    }

    #[test]
    fn cp_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/src", false).unwrap();
        fs.mkdir("/dst", false).unwrap();
        fs.write_file("/src/file.txt", b"content").unwrap();
        fs.cp("/src/file.txt", "/dst/copy.txt", false).unwrap();
        assert_eq!(fs.read_file_string("/dst/copy.txt").unwrap(), "content");
    }

    #[test]
    fn cp_file_into_directory() {
        let fs = InMemoryFs::new();
        fs.mkdir("/src", false).unwrap();
        fs.mkdir("/dst", false).unwrap();
        fs.write_file("/src/file.txt", b"content").unwrap();
        fs.cp("/src/file.txt", "/dst", false).unwrap();
        assert_eq!(fs.read_file_string("/dst/file.txt").unwrap(), "content");
    }

    #[test]
    fn cp_directory_recursive() {
        let fs = InMemoryFs::new();
        fs.mkdir("/src/sub", true).unwrap();
        fs.write_file("/src/a.txt", b"aaa").unwrap();
        fs.write_file("/src/sub/b.txt", b"bbb").unwrap();
        fs.cp("/src", "/dst", true).unwrap();
        assert!(fs.exists("/dst"));
        assert_eq!(fs.read_file_string("/dst/a.txt").unwrap(), "aaa");
        assert_eq!(fs.read_file_string("/dst/sub/b.txt").unwrap(), "bbb");
    }

    #[test]
    fn mv_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/old.txt", b"data").unwrap();
        fs.mv("/dir/old.txt", "/dir/new.txt").unwrap();
        assert!(!fs.exists("/dir/old.txt"));
        assert_eq!(fs.read_file_string("/dir/new.txt").unwrap(), "data");
    }

    #[test]
    fn chmod_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/file.txt", b"").unwrap();
        fs.chmod("/dir/file.txt", 0o755).unwrap();
        assert_eq!(fs.stat("/dir/file.txt").unwrap().mode, 0o755);
    }

    #[test]
    fn symlink_and_readlink() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/real.txt", b"content").unwrap();
        fs.symlink("/dir/real.txt", "/dir/link.txt").unwrap();

        assert_eq!(fs.readlink("/dir/link.txt").unwrap(), "/dir/real.txt");
        assert_eq!(fs.read_file_string("/dir/link.txt").unwrap(), "content");
    }

    #[test]
    fn lstat_returns_symlink_metadata() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/real.txt", b"content").unwrap();
        fs.symlink("/dir/real.txt", "/dir/link.txt").unwrap();

        let stat = fs.stat("/dir/link.txt").unwrap();
        assert!(stat.is_file()); // stat follows symlink

        let lstat = fs.lstat("/dir/link.txt").unwrap();
        assert!(lstat.is_symlink()); // lstat doesn't follow
    }

    #[test]
    fn hard_link() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/orig.txt", b"data").unwrap();
        fs.hard_link("/dir/orig.txt", "/dir/link.txt").unwrap();
        assert_eq!(fs.read_file_string("/dir/link.txt").unwrap(), "data");
    }

    #[test]
    fn realpath_resolves_symlinks() {
        let fs = InMemoryFs::new();
        fs.mkdir("/a", false).unwrap();
        fs.write_file("/a/file.txt", b"").unwrap();
        fs.symlink("/a", "/b").unwrap();
        assert_eq!(fs.realpath("/b/file.txt").unwrap(), "/a/file.txt");
    }

    #[test]
    fn symlink_loop_detected() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.symlink("/dir/b", "/dir/a").unwrap();
        fs.symlink("/dir/a", "/dir/b").unwrap();
        assert!(matches!(
            fs.read_file("/dir/a"),
            Err(FsError::SymlinkLoop(_))
        ));
    }

    #[test]
    fn touch_creates_empty_file() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.touch("/dir/new.txt").unwrap();
        assert!(fs.exists("/dir/new.txt"));
        assert_eq!(fs.read_file("/dir/new.txt").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn touch_updates_existing_mtime() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/file.txt", b"data").unwrap();
        let before = fs.stat("/dir/file.txt").unwrap().mtime;
        // Touch and verify mtime changed (or at least didn't error)
        fs.touch("/dir/file.txt").unwrap();
        let after = fs.stat("/dir/file.txt").unwrap().mtime;
        assert!(after >= before);
    }

    #[test]
    fn stat_file_metadata() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/file.txt", b"hello").unwrap();
        let meta = fs.stat("/dir/file.txt").unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.size, 5);
        assert_eq!(meta.mode, DEFAULT_FILE_MODE);
    }

    #[test]
    fn stat_directory_metadata() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        let meta = fs.stat("/dir").unwrap();
        assert!(meta.is_dir());
        assert_eq!(meta.mode, DEFAULT_DIR_MODE);
    }

    #[test]
    fn write_to_nonexistent_parent_fails() {
        let fs = InMemoryFs::new();
        assert!(fs.write_file("/no/parent/file.txt", b"data").is_err());
    }

    #[test]
    fn relative_symlink() {
        let fs = InMemoryFs::new();
        fs.mkdir("/dir", false).unwrap();
        fs.write_file("/dir/real.txt", b"content").unwrap();
        fs.symlink("real.txt", "/dir/link.txt").unwrap();
        assert_eq!(fs.read_file_string("/dir/link.txt").unwrap(), "content");
    }
}
