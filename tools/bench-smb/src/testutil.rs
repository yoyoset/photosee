//! Test-only helper: a self-deleting temp directory without the `tempfile` crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("bench-smb-{tag}-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).expect("temp dir is creatable in tests");
        TempDir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Writes `len` bytes of a cheap deterministic pattern.
    pub fn write(&self, name: &str, len: usize) -> PathBuf {
        let p = self.0.join(name);
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        fs::write(&p, data).expect("temp file is writable in tests");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
