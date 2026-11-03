//! In-memory directory listing cache keyed by path, revalidated by the
//! directory's mtime so SMB folders are scanned once. Package G.

use photosee_core::Listing;
use std::path::Path;

#[derive(Default)]
pub struct ListingCache {}

impl ListingCache {
    pub fn get(&self, dir: &Path, dir_mtime_ms: i64) -> Option<Listing> {
        let _ = (dir, dir_mtime_ms);
        None // package G
    }

    pub fn put(&self, dir: &Path, dir_mtime_ms: i64, listing: Listing) {
        let _ = (dir, dir_mtime_ms, listing);
    }
}
