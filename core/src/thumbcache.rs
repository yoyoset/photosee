//! On-disk thumbnail cache. Package F. Key = hash(path, size, mtime, edge),
//! so an edited file gets a new entry; stale entries are evicted LRU once the
//! cache exceeds its byte budget. LRU needs no index file: `get` bumps the
//! file's mtime and eviction sorts by it.
//!
//! A cache must never break a thumbnail, so every IO failure is swallowed
//! (counted in `io_errors`) and the caller just regenerates.

use crate::error::Result;
use crate::types::Entry;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

pub struct ThumbCache {
    pub dir: PathBuf,
    /// 0 disables the cache: `get` and `put` do nothing.
    pub budget_bytes: u64,
    /// Bytes on disk. `None` until the first `put` scans the directory once;
    /// guarded by the same lock that serializes eviction.
    total: Mutex<Option<u64>>,
    tmp_seq: AtomicU64,
    io_errors: AtomicU64,
}

struct Found {
    path: PathBuf,
    size: u64,
    mtime: SystemTime,
}

impl ThumbCache {
    pub fn new(dir: PathBuf, budget_bytes: u64) -> Self {
        Self {
            dir,
            budget_bytes,
            total: Mutex::new(None),
            tmp_seq: AtomicU64::new(0),
            io_errors: AtomicU64::new(0),
        }
    }

    /// 16 hex digits of FNV-1a over path, size, mtime and edge. std's
    /// `DefaultHasher` is documented as unstable across Rust releases, and a
    /// changed hash would orphan the whole cache on a toolchain bump.
    pub fn key(path: &Path, entry: &Entry, edge: u32) -> String {
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        h.write(path.to_string_lossy().as_bytes());
        // Fixed-width fields after a separator keep "ab"+1 distinct from "a"+b1.
        h.write(&[0xFF]);
        h.write(&entry.size.to_le_bytes());
        h.write(&entry.mtime_ms.to_le_bytes());
        h.write(&edge.to_le_bytes());
        format!("{:016x}", h.0)
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        if self.budget_bytes == 0 {
            return None;
        }
        let path = self.path_of(key)?;
        let bytes = fs::read(&path).ok()?;
        // Write access is needed to set times on Windows; failure only
        // weakens LRU order.
        let bumped = File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_modified(SystemTime::now()));
        if bumped.is_err() {
            self.fail();
        }
        Some(bytes)
    }

    /// Never fails: a cache that cannot write is just a cache miss later.
    pub fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        if self.budget_bytes == 0 {
            return Ok(());
        }
        let Some(path) = self.path_of(key) else {
            return Ok(());
        };
        let mut total = self.total.lock().unwrap_or_else(|p| p.into_inner());
        let before = match *total {
            Some(t) => t,
            None => self.scan().iter().map(|f| f.size).sum(),
        };
        let old = fs::metadata(&path).map_or(0, |m| m.len());
        if self.write_atomic(&path, bytes).is_err() {
            self.fail();
            *total = Some(before);
            return Ok(());
        }
        let mut now = before.saturating_sub(old) + bytes.len() as u64;
        if now > self.budget_bytes {
            now = self.evict();
        }
        *total = Some(now);
        Ok(())
    }

    /// Count of swallowed IO failures, for diagnostics and tests.
    pub fn io_errors(&self) -> u64 {
        self.io_errors.load(Ordering::Relaxed)
    }

    fn fail(&self) {
        self.io_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Keys are our own hex; anything else could escape `dir`.
    fn path_of(&self, key: &str) -> Option<PathBuf> {
        let ok = key.len() >= 2 && key.bytes().all(|b| b.is_ascii_hexdigit());
        ok.then(|| self.dir.join(&key[..2]).join(format!("{key}.jpg")))
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Same directory so the rename never crosses a volume.
        let seq = self.tmp_seq.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("tmp{}-{seq}", std::process::id()));
        let res = fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, path));
        if res.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        res
    }

    /// All cache files (`<dir>/<xx>/<key>.jpg`). Leftover `.tmp*` files from a
    /// crashed write are ignored.
    fn scan(&self) -> Vec<Found> {
        let mut out = Vec::new();
        let Ok(top) = fs::read_dir(&self.dir) else {
            return out;
        };
        for sub in top.flatten().filter(|d| d.path().is_dir()) {
            let Ok(files) = fs::read_dir(sub.path()) else {
                continue;
            };
            for f in files.flatten() {
                let path = f.path();
                if path.extension().is_none_or(|e| e != "jpg") {
                    continue;
                }
                if let Ok(m) = f.metadata() {
                    let mtime = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    out.push(Found {
                        path,
                        size: m.len(),
                        mtime,
                    });
                }
            }
        }
        out
    }

    /// Deletes oldest-first down to 90% of the budget (hysteresis, so the
    /// next put does not rescan). Returns the new byte total, recomputed from
    /// the scan so drift from outside writers is corrected.
    fn evict(&self) -> u64 {
        let mut files = self.scan();
        files.sort_by_key(|f| f.mtime);
        let mut total: u64 = files.iter().map(|f| f.size).sum();
        let target = self.budget_bytes / 10 * 9;
        for f in files {
            if total <= target {
                break;
            }
            if fs::remove_file(&f.path).is_ok() {
                total -= f.size;
            } else {
                self.fail();
            }
        }
        total
    }
}

/// 64-bit FNV-1a.
struct Fnv(u64);

impl Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EntryKind;

    fn entry(size: u64, mtime_ms: i64) -> Entry {
        Entry {
            name: "a.jpg".into(),
            kind: EntryKind::Image(crate::types::Format::Jpeg),
            size,
            mtime_ms,
        }
    }

    /// Pinned values: if these change, every user's cache is orphaned.
    #[test]
    fn key_is_stable_across_releases() {
        let k = ThumbCache::key(Path::new("/p/a.jpg"), &entry(10, 20), 256);
        assert_eq!(k.len(), 16);
        assert_eq!(
            k,
            ThumbCache::key(Path::new("/p/a.jpg"), &entry(10, 20), 256)
        );
        assert_eq!(k, "a5a8c998991ee455");
    }

    #[test]
    fn key_changes_with_every_input() {
        let p = Path::new("/p/a.jpg");
        let base = ThumbCache::key(p, &entry(10, 20), 256);
        assert_ne!(
            base,
            ThumbCache::key(Path::new("/p/b.jpg"), &entry(10, 20), 256)
        );
        assert_ne!(base, ThumbCache::key(p, &entry(11, 20), 256));
        assert_ne!(base, ThumbCache::key(p, &entry(10, 21), 256));
        assert_ne!(base, ThumbCache::key(p, &entry(10, 20), 128));
    }
}
