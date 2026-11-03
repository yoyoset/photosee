//! On-disk thumbnail cache. Package F. Key = hash(path, size, mtime, edge),
//! so an edited file gets a new entry; stale entries are evicted LRU once the
//! cache exceeds its byte budget.

use crate::error::Result;
use crate::types::Entry;
use std::path::{Path, PathBuf};

pub struct ThumbCache {
    pub dir: PathBuf,
    pub budget_bytes: u64,
}

impl ThumbCache {
    pub fn new(dir: PathBuf, budget_bytes: u64) -> Self {
        Self { dir, budget_bytes }
    }

    pub fn key(path: &Path, entry: &Entry, edge: u32) -> String {
        let _ = (path, entry, edge);
        String::new() // package F
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let _ = key;
        None // package F
    }

    pub fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let _ = (key, bytes);
        Ok(()) // package F
    }
}
