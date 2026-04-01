use std::collections::BTreeMap;
use std::time::SystemTime;

use crate::error::FsError;
use crate::fs::path;
use crate::fs::types::{DirEntry, FileType, Metadata};
use crate::fs::VirtualFs;

/// A filesystem router that composes multiple backends at different mount points.
///
/// Operations are dispatched to the filesystem with the longest matching mount
/// point prefix. If no mount matches, the base filesystem handles the request.
pub struct MountableFs {
    base: Box<dyn VirtualFs>,
    mounts: BTreeMap<String, Box<dyn VirtualFs>>,
}

impl MountableFs {
    pub fn new(base: impl VirtualFs + 'static) -> Self {
        Self {
            base: Box::new(base),
            mounts: BTreeMap::new(),
        }
    }

    pub fn mount(&mut self, mount_path: &str, fs: impl VirtualFs + 'static) {
        let normalized = path::normalize(mount_path);
        self.mounts.insert(normalized, Box::new(fs));
    }

    pub fn unmount(&mut self, mount_path: &str) {
        let normalized = path::normalize(mount_path);
        self.mounts.remove(&normalized);
    }

    fn resolve(&self, virtual_path: &str) -> (&dyn VirtualFs, String) {
        let normalized = path::normalize(virtual_path);
        let mut best_match: Option<(&str, &dyn VirtualFs)> = None;

        for (mount_point, fs) in &self.mounts {
            if normalized == *mount_point
                || (normalized.starts_with(mount_point.as_str())
                    && normalized.as_bytes().get(mount_point.len()) == Some(&b'/'))
            {
                match best_match {
                    Some((prev, _)) if mount_point.len() <= prev.len() => {}
                    _ => best_match = Some((mount_point.as_str(), &**fs)),
                }
            }
        }

        match best_match {
            Some((mount_point, fs)) => {
                let relative = &normalized[mount_point.len()..];
                let adjusted = if relative.is_empty() {
                    "/".to_string()
                } else {
                    relative.to_string()
                };
                (fs, adjusted)
            }
            None => (&*self.base, normalized),
        }
    }

    fn mount_points_under(&self, dir_path: &str) -> Vec<String> {
        let normalized = path::normalize(dir_path);
        let prefix = if normalized == "/" {
            "/".to_string()
        } else {
            format!("{normalized}/")
        };

        let mut names = Vec::new();
        for mount_point in self.mounts.keys() {
            let candidate = if normalized == "/" {
                mount_point.as_str()
            } else if let Some(rest) = mount_point.strip_prefix(prefix.as_str()) {
                rest
            } else {
                continue;
            };

            if normalized == "/" {
                let trimmed = candidate.strip_prefix('/').unwrap_or(candidate);
                if let Some(first) = trimmed.split('/').next() {
                    if !first.is_empty() {
                        names.push(first.to_string());
                    }
                }
            } else if !candidate.is_empty() && !candidate.contains('/') {
                names.push(candidate.to_string());
            } else if let Some(first) = candidate.split('/').next() {
                if !first.is_empty() {
                    names.push(first.to_string());
                }
            }
        }

        names.sort();
        names.dedup();
        names
    }
}

impl std::fmt::Debug for MountableFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountableFs")
            .field("mount_count", &self.mounts.len())
            .field("mount_points", &self.mounts.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl VirtualFs for MountableFs {
    fn read_file(&self, file_path: &str) -> Result<Vec<u8>, FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        fs.read_file(&adjusted)
    }

    fn read_file_string(&self, file_path: &str) -> Result<String, FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        fs.read_file_string(&adjusted)
    }

    fn write_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        fs.write_file(&adjusted, content)
    }

    fn append_file(&self, file_path: &str, content: &[u8]) -> Result<(), FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        fs.append_file(&adjusted, content)
    }

    fn exists(&self, file_path: &str) -> bool {
        let normalized = path::normalize(file_path);
        for mount_point in self.mounts.keys() {
            if *mount_point == normalized {
                return true;
            }
        }
        let (fs, adjusted) = self.resolve(file_path);
        fs.exists(&adjusted)
    }

    fn stat(&self, file_path: &str) -> Result<Metadata, FsError> {
        let normalized = path::normalize(file_path);
        for mount_point in self.mounts.keys() {
            if *mount_point == normalized {
                let fs = &self.mounts[mount_point];
                return fs.stat("/");
            }
        }
        let (fs, adjusted) = self.resolve(file_path);
        fs.stat(&adjusted)
    }

    fn lstat(&self, file_path: &str) -> Result<Metadata, FsError> {
        let normalized = path::normalize(file_path);
        for mount_point in self.mounts.keys() {
            if *mount_point == normalized {
                let fs = &self.mounts[mount_point];
                return fs.lstat("/");
            }
        }
        let (fs, adjusted) = self.resolve(file_path);
        fs.lstat(&adjusted)
    }

    fn mkdir(&self, dir_path: &str, recursive: bool) -> Result<(), FsError> {
        let (fs, adjusted) = self.resolve(dir_path);
        fs.mkdir(&adjusted, recursive)
    }

    fn readdir(&self, dir_path: &str) -> Result<Vec<DirEntry>, FsError> {
        let (fs, adjusted) = self.resolve(dir_path);
        let mut entries = fs.readdir(&adjusted)?;

        let mount_children = self.mount_points_under(dir_path);
        let existing_names: std::collections::HashSet<String> =
            entries.iter().map(|e| e.name.clone()).collect();

        for name in mount_children {
            if !existing_names.contains(&name) {
                entries.push(DirEntry {
                    name,
                    file_type: FileType::Directory,
                });
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn rm(&self, file_path: &str, recursive: bool, force: bool) -> Result<(), FsError> {
        let normalized = path::normalize(file_path);
        if self.mounts.contains_key(&normalized) {
            return Err(FsError::Busy(format!(
                "{file_path}: is a mount point"
            )));
        }
        let (fs, adjusted) = self.resolve(file_path);
        fs.rm(&adjusted, recursive, force)
    }

    fn cp(&self, src: &str, dest: &str, recursive: bool) -> Result<(), FsError> {
        let (src_fs, src_adjusted) = self.resolve(src);
        let (dest_fs, dest_adjusted) = self.resolve(dest);

        if std::ptr::eq(src_fs, dest_fs) {
            return src_fs.cp(&src_adjusted, &dest_adjusted, recursive);
        }

        let meta = src_fs.stat(&src_adjusted)?;
        if meta.is_dir() {
            if !recursive {
                return Err(FsError::IsADirectory(src.to_string()));
            }
            dest_fs.mkdir(&dest_adjusted, true)?;
            let children = src_fs.readdir(&src_adjusted)?;
            let src_norm = path::normalize(src);
            let dest_norm = path::normalize(dest);
            for child in children {
                let child_src = path::join(&src_norm, &child.name);
                let child_dest = path::join(&dest_norm, &child.name);
                self.cp(&child_src, &child_dest, true)?;
            }
            return Ok(());
        }

        let content = src_fs.read_file(&src_adjusted)?;
        dest_fs.write_file(&dest_adjusted, &content)
    }

    fn mv(&self, src: &str, dest: &str) -> Result<(), FsError> {
        let normalized_src = path::normalize(src);
        if self.mounts.contains_key(&normalized_src) {
            return Err(FsError::Busy(format!(
                "{src}: is a mount point"
            )));
        }
        self.cp(src, dest, true)?;
        self.rm(src, true, false)
    }

    fn chmod(&self, file_path: &str, mode: u32) -> Result<(), FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        fs.chmod(&adjusted, mode)
    }

    fn symlink(&self, target: &str, link_path: &str) -> Result<(), FsError> {
        let (fs, adjusted) = self.resolve(link_path);
        fs.symlink(target, &adjusted)
    }

    fn hard_link(&self, existing: &str, new_path: &str) -> Result<(), FsError> {
        let (src_fs, src_adjusted) = self.resolve(existing);
        let (dest_fs, dest_adjusted) = self.resolve(new_path);

        if std::ptr::eq(src_fs, dest_fs) {
            return src_fs.hard_link(&src_adjusted, &dest_adjusted);
        }

        Err(FsError::CrossDevice(format!(
            "{existing} -> {new_path}: cross-mount hard link"
        )))
    }

    fn readlink(&self, link_path: &str) -> Result<String, FsError> {
        let (fs, adjusted) = self.resolve(link_path);
        fs.readlink(&adjusted)
    }

    fn realpath(&self, file_path: &str) -> Result<String, FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        let resolved = fs.realpath(&adjusted)?;

        let normalized = path::normalize(file_path);
        let mut best_mount = None;
        for mount_point in self.mounts.keys() {
            if normalized == *mount_point
                || (normalized.starts_with(mount_point.as_str())
                    && normalized.as_bytes().get(mount_point.len()) == Some(&b'/'))
            {
                match best_mount {
                    Some((prev_len, _)) if mount_point.len() <= prev_len => {}
                    _ => best_mount = Some((mount_point.len(), mount_point.as_str())),
                }
            }
        }

        match best_mount {
            Some((_, mount_point)) => {
                if resolved == "/" {
                    Ok(mount_point.to_string())
                } else {
                    Ok(format!("{mount_point}{resolved}"))
                }
            }
            None => Ok(resolved),
        }
    }

    fn touch(&self, file_path: &str) -> Result<(), FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        fs.touch(&adjusted)
    }

    fn set_times(&self, file_path: &str, mtime: Option<SystemTime>) -> Result<(), FsError> {
        let (fs, adjusted) = self.resolve(file_path);
        fs.set_times(&adjusted, mtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::memory::InMemoryFs;

    fn setup_base() -> InMemoryFs {
        let fs = InMemoryFs::new();
        fs.mkdir("/home", false).unwrap();
        fs.mkdir("/home/user", false).unwrap();
        fs.write_file("/home/user/file.txt", b"base file").unwrap();
        fs.mkdir("/tmp", false).unwrap();
        fs
    }

    fn setup_mounted() -> InMemoryFs {
        let fs = InMemoryFs::new();
        fs.mkdir("/data", false).unwrap();
        fs.write_file("/data/info.txt", b"mounted data").unwrap();
        fs
    }

    #[test]
    fn read_from_base() {
        let mfs = MountableFs::new(setup_base());
        assert_eq!(
            mfs.read_file_string("/home/user/file.txt").unwrap(),
            "base file"
        );
    }

    #[test]
    fn read_from_mount() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        assert_eq!(
            mfs.read_file_string("/mnt/ext/data/info.txt").unwrap(),
            "mounted data"
        );
    }

    #[test]
    fn write_to_mount() {
        let mut mfs = MountableFs::new(setup_base());
        let mounted = setup_mounted();
        mfs.mount("/mnt/ext", mounted);
        mfs.write_file("/mnt/ext/data/new.txt", b"new content")
            .unwrap();
        assert_eq!(
            mfs.read_file_string("/mnt/ext/data/new.txt").unwrap(),
            "new content"
        );
    }

    #[test]
    fn readdir_shows_mount_points() {
        let base = InMemoryFs::new();
        base.mkdir("/mnt", false).unwrap();
        let mut mfs = MountableFs::new(base);
        mfs.mount("/mnt/disk1", InMemoryFs::new());
        mfs.mount("/mnt/disk2", InMemoryFs::new());
        let entries = mfs.readdir("/mnt").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"disk1"));
        assert!(names.contains(&"disk2"));
    }

    #[test]
    fn exists_on_mount_point() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        assert!(mfs.exists("/mnt/ext"));
    }

    #[test]
    fn stat_mount_point() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        let meta = mfs.stat("/mnt/ext").unwrap();
        assert!(meta.is_dir());
    }

    #[test]
    fn rm_mount_point_fails() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        let result = mfs.rm("/mnt/ext", true, false);
        assert!(result.is_err());
    }

    #[test]
    fn mv_mount_point_fails() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        let result = mfs.mv("/mnt/ext", "/mnt/other");
        assert!(result.is_err());
    }

    #[test]
    fn unmount_removes_mount() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        assert!(mfs.exists("/mnt/ext"));
        mfs.unmount("/mnt/ext");
        assert!(!mfs.exists("/mnt/ext"));
    }

    #[test]
    fn longest_prefix_wins() {
        let base = InMemoryFs::new();
        let outer = InMemoryFs::new();
        outer.mkdir("/file", false).unwrap();
        outer.write_file("/file/data.txt", b"outer").unwrap();
        let inner = InMemoryFs::new();
        inner.write_file("/data.txt", b"inner").unwrap();

        let mut mfs = MountableFs::new(base);
        mfs.mount("/mnt", outer);
        mfs.mount("/mnt/file", inner);

        assert_eq!(
            mfs.read_file_string("/mnt/file/data.txt").unwrap(),
            "inner"
        );
    }

    #[test]
    fn cross_mount_cp() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        mfs.cp("/mnt/ext/data/info.txt", "/tmp/copied.txt", false)
            .unwrap();
        assert_eq!(
            mfs.read_file_string("/tmp/copied.txt").unwrap(),
            "mounted data"
        );
    }

    #[test]
    fn cross_mount_hard_link_fails() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt/ext", setup_mounted());
        let result = mfs.hard_link("/mnt/ext/data/info.txt", "/tmp/link.txt");
        assert!(result.is_err());
    }

    #[test]
    fn touch_through_mount() {
        let mut mfs = MountableFs::new(setup_base());
        let mounted = InMemoryFs::new();
        mounted.mkdir("/dir", false).unwrap();
        mfs.mount("/mnt", mounted);
        mfs.touch("/mnt/dir/new.txt").unwrap();
        assert!(mfs.exists("/mnt/dir/new.txt"));
    }

    #[test]
    fn mkdir_through_mount() {
        let mut mfs = MountableFs::new(setup_base());
        mfs.mount("/mnt", InMemoryFs::new());
        mfs.mkdir("/mnt/newdir", false).unwrap();
        assert!(mfs.exists("/mnt/newdir"));
        assert!(mfs.stat("/mnt/newdir").unwrap().is_dir());
    }
}
