//! In-memory directory listing cache keyed by path, revalidated by the
//! directory's mtime so SMB folders are scanned once. Package G.
//!
//! The directory mtime changes when entries are added, removed or renamed,
//! which is when a listing goes stale. It does not change when a file's
//! contents are rewritten in place, so a cached listing can show an old
//! `size`/`mtime_ms` for such a file. Image ETags stat the file themselves,
//! so only the listing's metadata lags, never the pixels.

use photosee_core::Listing;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Directories remembered. A viewing session touches a handful; the cap only
/// stops a long crawl from growing memory without bound.
pub const MAX_ENTRIES: usize = 512;

struct Slot {
    mtime_ms: i64,
    listing: Listing,
    /// Insertion order; the smallest is evicted first.
    seq: u64,
}

#[derive(Default)]
struct Inner {
    map: HashMap<PathBuf, Slot>,
    next_seq: u64,
}

#[derive(Default)]
pub struct ListingCache {
    inner: Mutex<Inner>,
}

impl ListingCache {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        // Plain map; a panic cannot leave it inconsistent.
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The cached listing, if it was stored for this exact directory mtime.
    pub fn get(&self, dir: &Path, dir_mtime_ms: i64) -> Option<Listing> {
        let inner = self.lock();
        let slot = inner.map.get(dir)?;
        (slot.mtime_ms == dir_mtime_ms).then(|| slot.listing.clone())
    }

    pub fn put(&self, dir: &Path, dir_mtime_ms: i64, listing: Listing) {
        let mut inner = self.lock();
        let seq = inner.next_seq;
        inner.next_seq += 1;
        if !inner.map.contains_key(dir) && inner.map.len() >= MAX_ENTRIES {
            // O(n) over at most 512 entries, and only when full.
            let oldest = inner
                .map
                .iter()
                .min_by_key(|(_, s)| s.seq)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                inner.map.remove(&k);
            }
        }
        inner.map.insert(
            dir.to_path_buf(),
            Slot {
                mtime_ms: dir_mtime_ms,
                listing,
                seq,
            },
        );
    }

    pub fn len(&self) -> usize {
        self.lock().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photosee_core::{Entry, EntryKind};

    fn listing(tag: &str) -> Listing {
        Listing {
            images: vec![],
            dirs: vec![Entry {
                name: tag.into(),
                kind: EntryKind::Dir,
                size: 0,
                mtime_ms: 0,
            }],
        }
    }

    #[test]
    fn hit_only_while_mtime_matches() {
        let c = ListingCache::default();
        let d = Path::new("/a");
        assert!(c.get(d, 1).is_none());
        c.put(d, 1, listing("x"));
        assert_eq!(c.get(d, 1), Some(listing("x")));
        assert!(c.get(d, 2).is_none(), "changed mtime must miss");
        c.put(d, 2, listing("y"));
        assert!(c.get(d, 1).is_none());
        assert_eq!(c.get(d, 2), Some(listing("y")));
    }

    #[test]
    fn evicts_the_oldest_inserted_entry() {
        let c = ListingCache::default();
        for i in 0..MAX_ENTRIES {
            c.put(Path::new(&format!("/d{i}")), 0, listing("x"));
        }
        // Reading does not refresh age: eviction is by insertion order.
        assert!(c.get(Path::new("/d0"), 0).is_some());
        c.put(Path::new("/new"), 0, listing("x"));
        assert_eq!(c.len(), MAX_ENTRIES);
        assert!(c.get(Path::new("/d0"), 0).is_none(), "oldest is gone");
        assert!(c.get(Path::new("/d1"), 0).is_some());
        assert!(c.get(Path::new("/new"), 0).is_some());
    }

    #[test]
    fn replacing_a_key_does_not_evict_others() {
        let c = ListingCache::default();
        for i in 0..MAX_ENTRIES {
            c.put(Path::new(&format!("/d{i}")), 0, listing("x"));
        }
        c.put(Path::new("/d5"), 7, listing("z"));
        assert_eq!(c.len(), MAX_ENTRIES);
        assert!(c.get(Path::new("/d0"), 0).is_some());
        assert_eq!(c.get(Path::new("/d5"), 7), Some(listing("z")));
    }
}
