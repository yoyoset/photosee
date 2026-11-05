//! In-memory `FsBackend` for tests. Paths use '/' separators on every OS.

use crate::error::{Error, ErrorCode, Result};
use crate::io::{FsBackend, ReadAt};
use crate::types::{Entry, EntryKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
pub struct MemFs {
    files: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
    dirs: Mutex<Vec<PathBuf>>,
}

impl MemFs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file, creating parent directories implicitly.
    pub fn file(&self, path: &str, bytes: &[u8]) -> &Self {
        let p = PathBuf::from(path);
        self.add_parents(&p);
        self.files
            .lock()
            .expect("memfs lock")
            .insert(p, bytes.to_vec());
        self
    }

    /// Adds an (possibly empty) directory.
    pub fn dir(&self, path: &str) -> &Self {
        let p = PathBuf::from(path);
        self.add_parents(&p);
        self.push_dir(p);
        self
    }

    fn add_parents(&self, p: &Path) {
        for a in p.ancestors().skip(1) {
            if !a.as_os_str().is_empty() {
                self.push_dir(a.to_path_buf());
            }
        }
    }

    fn push_dir(&self, p: PathBuf) {
        let mut dirs = self.dirs.lock().expect("memfs lock");
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
}

fn not_found(p: &Path) -> Error {
    Error::new(ErrorCode::NotFound, format!("memfs: {}", p.display()))
}

impl FsBackend for MemFs {
    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>> {
        if !self
            .dirs
            .lock()
            .expect("memfs lock")
            .iter()
            .any(|d| d == dir)
        {
            return Err(not_found(dir));
        }
        // Collect child paths first: `stat` takes the same locks again.
        let mut children: Vec<PathBuf> = self.dirs.lock().expect("memfs lock").clone();
        children.extend(self.files.lock().expect("memfs lock").keys().cloned());
        Ok(children
            .iter()
            .filter(|p| p.parent() == Some(dir))
            .filter_map(|p| self.stat(p).ok())
            .collect())
    }

    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>> {
        let files = self.files.lock().expect("memfs lock");
        let bytes = files.get(path).ok_or_else(|| not_found(path))?;
        Ok(Box::new(bytes.clone()))
    }

    fn stat(&self, path: &Path) -> Result<Entry> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if self
            .dirs
            .lock()
            .expect("memfs lock")
            .iter()
            .any(|d| d == path)
        {
            return Ok(Entry {
                name,
                kind: EntryKind::Dir,
                size: 0,
                mtime_ms: 0,
            });
        }
        let files = self.files.lock().expect("memfs lock");
        let bytes = files.get(path).ok_or_else(|| not_found(path))?;
        // Same rule as LocalFs: archives list as folders, size = file length.
        let kind = if crate::archive::is_archive_name(&name) {
            EntryKind::Dir
        } else {
            EntryKind::Image(
                crate::format::from_extension(&name)
                    .ok_or_else(|| Error::unsupported(format!("not an image: {name}")))?,
            )
        };
        Ok(Entry {
            name,
            kind,
            size: bytes.len() as u64,
            mtime_ms: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_dir_lists_direct_children_only() {
        let fs = MemFs::new();
        fs.file("/r/a.jpg", b"x")
            .file("/r/sub/b.png", b"y")
            .dir("/r/empty");
        let mut names: Vec<_> = fs
            .read_dir(Path::new("/r"))
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        names.sort();
        assert_eq!(names, ["a.jpg", "empty", "sub"]);
    }
}
