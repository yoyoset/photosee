//! Comic archives (.cbz) shown as folders. `ArchiveFs` wraps any
//! `FsBackend`: a path that runs through `x.cbz` is answered from the
//! archive, everything else goes straight to the wrapped backend. The rest
//! of the system (scan, nav, thumbnails, the HTTP API) never knows.
//!
//! Why a folder and not a multi-page image: "next folder" (nav.rs) then
//! walks volume 1 → volume 2 for free, and the grid, prefetch and thumbnail
//! cache work unchanged.
//!
//! Limitation: a real *directory* whose name ends in `.cbz` is treated as
//! an archive and fails to open.

mod tree;
pub mod zip;

use crate::error::{Error, ErrorCode, Result};
use crate::io::{FsBackend, ReadAt};
use crate::types::{Entry, EntryKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tree::Tree;

/// Parsed directories kept per archive. Paging through a volume hits the
/// same archive hundreds of times; re-reading its directory each time would
/// cost two extra reads per page (one tail, one directory) on SMB.
const INDEX_CACHE: usize = 8;

/// `.cbz` only. Plain `.zip` stays a file: users keep all kinds of archives
/// next to photos and should not have them all turn into folders.
pub fn is_archive_name(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("cbz"))
}

struct Cached {
    path: PathBuf,
    size: u64,
    mtime_ms: i64,
    tree: Arc<Tree>,
}

pub struct ArchiveFs<B> {
    inner: B,
    cache: Mutex<Vec<Cached>>,
}

/// The archive file, the path inside it, and the archive's own entry.
struct Split<'a> {
    archive: PathBuf,
    inner: String,
    leaf: &'a Path,
}

impl<B: FsBackend> ArchiveFs<B> {
    pub fn new(inner: B) -> Self {
        Self {
            inner,
            cache: Mutex::new(Vec::new()),
        }
    }

    /// `None` when no component of `path` is an archive.
    fn split<'a>(&self, path: &'a Path) -> Option<Split<'a>> {
        let mut archive = PathBuf::new();
        let mut comps = path.components();
        for c in comps.by_ref() {
            archive.push(c);
            if is_archive_name(&c.as_os_str().to_string_lossy()) {
                let rest: Vec<String> = comps
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                let inner = rest.join("/");
                return Some(Split {
                    archive,
                    inner,
                    leaf: path,
                });
            }
        }
        None
    }

    fn tree(&self, archive: &Path) -> Result<(Arc<Tree>, Entry)> {
        let st = self.inner.stat(archive)?;
        let mut cache = self.cache.lock().map_err(|_| poisoned())?;
        if let Some(i) = cache
            .iter()
            .position(|c| c.path == archive && c.size == st.size && c.mtime_ms == st.mtime_ms)
        {
            let hit = cache.remove(i);
            let tree = Arc::clone(&hit.tree);
            cache.push(hit);
            return Ok((tree, st));
        }
        // Parse without the lock: on a NAS this can take a second, and
        // other archives must not wait for it.
        drop(cache);
        let src = self.inner.open(archive)?;
        let tree = Arc::new(tree::build(zip::members(src.as_ref())?));
        let mut cache = self.cache.lock().map_err(|_| poisoned())?;
        cache.retain(|c| c.path != archive);
        if cache.len() >= INDEX_CACHE {
            cache.remove(0);
        }
        cache.push(Cached {
            path: archive.to_path_buf(),
            size: st.size,
            mtime_ms: st.mtime_ms,
            tree: Arc::clone(&tree),
        });
        Ok((tree, st))
    }

    fn open_member(&self, s: &Split<'_>) -> Result<Box<dyn ReadAt>> {
        let (tree, _) = self.tree(&s.archive)?;
        let m = tree.file(&s.inner).ok_or_else(|| missing(s.leaf))?;
        let src = self.inner.open(&s.archive)?;
        if m.method == zip::STORED && !m.encrypted {
            // Stored pages are a byte range of the archive: no copy, and the
            // server can pass them through with Range support like a file.
            let start = zip::data_start(src.as_ref(), m)?;
            return Ok(Box::new(Window {
                src,
                start,
                len: m.size,
            }));
        }
        Ok(Box::new(zip::read_member(src.as_ref(), m)?))
    }
}

fn poisoned() -> Error {
    Error::new(ErrorCode::Io, "archive index cache lock poisoned")
}

fn missing(p: &Path) -> Error {
    Error::new(
        ErrorCode::NotFound,
        format!("not in archive: {}", p.display()),
    )
}

/// Entry for a member, inheriting the archive's mtime so ETags and the
/// thumbnail cache change when the archive is replaced.
fn member_entry(name: &str, m: Option<&zip::Member>, archive: &Entry) -> Option<Entry> {
    let kind = match m {
        None => EntryKind::Dir,
        Some(_) => EntryKind::Image(crate::format::from_extension(name)?),
    };
    Some(Entry {
        name: name.to_owned(),
        kind,
        size: m.map_or(0, |m| m.size),
        mtime_ms: archive.mtime_ms,
    })
}

impl<B: FsBackend> FsBackend for ArchiveFs<B> {
    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>> {
        let Some(s) = self.split(dir) else {
            return self.inner.read_dir(dir);
        };
        let (tree, st) = self.tree(&s.archive)?;
        if !tree.dirs.contains(&s.inner) {
            return Err(missing(dir));
        }
        Ok(tree
            .children(&s.inner)
            .into_iter()
            .filter_map(|(name, m)| member_entry(name, m, &st))
            .collect())
    }

    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>> {
        match self.split(path) {
            Some(s) if !s.inner.is_empty() => self.open_member(&s),
            _ => self.inner.open(path),
        }
    }

    fn stat(&self, path: &Path) -> Result<Entry> {
        let Some(s) = self.split(path) else {
            return self.inner.stat(path);
        };
        if s.inner.is_empty() {
            return Ok(Entry {
                kind: EntryKind::Dir,
                size: 0,
                ..self.inner.stat(path)?
            });
        }
        let (tree, st) = self.tree(&s.archive)?;
        let (_, name) = tree::split_last(&s.inner);
        let m = if tree.dirs.contains(&s.inner) {
            None
        } else {
            Some(tree.file(&s.inner).ok_or_else(|| missing(path))?)
        };
        member_entry(name, m, &st)
            .ok_or_else(|| Error::unsupported(format!("not an image: {name}")))
    }
}

/// A byte range of another source.
struct Window {
    src: Box<dyn ReadAt>,
    start: u64,
    len: u64,
}

impl ReadAt for Window {
    fn len(&self) -> Result<u64> {
        Ok(self.len)
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        if offset >= self.len {
            return Ok(0);
        }
        let room = usize::try_from(self.len - offset).unwrap_or(usize::MAX);
        let n = buf.len().min(room);
        self.src.read_at(self.start + offset, &mut buf[..n])
    }
}
