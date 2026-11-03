//! Phase 4: concurrency sweep. Whole-file reads of 40 JPEGs with K threads.

use crate::phases::read_full;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

pub const KS: [usize; 5] = [1, 2, 4, 8, 16];
pub const FILES_PER_K: usize = 40;

pub struct KResult {
    pub k: usize,
    pub secs: f64,
    pub bytes: u64,
    pub errors: Vec<String>,
}

impl KResult {
    /// Decimal megabytes (10^6 bytes) per second.
    pub fn mb_per_s(&self) -> f64 {
        if self.secs > 0.0 {
            self.bytes as f64 / 1e6 / self.secs
        } else {
            0.0
        }
    }
}

/// True when the folder is too small for every K to get its own files, so
/// later Ks re-read files an earlier K already pulled into the OS cache.
pub fn reuses_files(jpeg_count: usize) -> bool {
    jpeg_count < KS.len() * FILES_PER_K
}

/// The file set for the `slot`-th K. Distinct slots get disjoint windows while
/// there are enough files; otherwise the window wraps (see `reuses_files`).
pub fn file_set(files: &[PathBuf], slot: usize) -> Vec<PathBuf> {
    if files.is_empty() {
        return Vec::new();
    }
    (0..FILES_PER_K)
        .map(|i| files[(slot * FILES_PER_K + i) % files.len()].clone())
        .collect()
}

/// Worker loop: pull the next index from the shared counter until exhausted.
fn work(set: &[PathBuf], next: &AtomicUsize) -> (u64, Vec<String>) {
    let (mut bytes, mut errors) = (0u64, Vec::new());
    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        let Some(p) = set.get(i) else {
            return (bytes, errors);
        };
        match read_full(p) {
            Ok(n) => bytes += n,
            Err(e) => errors.push(format!("sweep {}: {e}", p.display())),
        }
    }
}

pub fn run_k(set: &[PathBuf], k: usize) -> KResult {
    let next = AtomicUsize::new(0);
    let mut res = KResult {
        k,
        secs: 0.0,
        bytes: 0,
        errors: Vec::new(),
    };
    let start = Instant::now();
    thread::scope(|s| {
        let handles: Vec<_> = (0..k)
            .map(|_| thread::Builder::new().spawn_scoped(s, || work(set, &next)))
            .collect();
        for h in handles {
            // A failed spawn or a panicked worker is data for the report, never a crash.
            let joined = h
                .map_err(|e| e.to_string())
                .and_then(|h| h.join().map_err(|_| "worker panicked".to_string()));
            match joined {
                Ok((b, errs)) => {
                    res.bytes += b;
                    res.errors.extend(errs);
                }
                Err(e) => res.errors.push(format!("sweep K={k}: thread failed: {e}")),
            }
        }
    });
    res.secs = start.elapsed().as_secs_f64();
    res
}

pub fn sweep(jpegs: &[PathBuf]) -> Vec<KResult> {
    KS.iter()
        .enumerate()
        .map(|(slot, &k)| run_k(&file_set(jpegs, slot), k))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn names(n: usize) -> Vec<PathBuf> {
        (0..n).map(|i| PathBuf::from(format!("f{i}.jpg"))).collect()
    }

    #[test]
    fn sets_are_disjoint_when_enough_files() {
        let f = names(200);
        assert!(!reuses_files(200));
        let a = file_set(&f, 0);
        let b = file_set(&f, 1);
        assert_eq!(a.len(), FILES_PER_K);
        assert!(a.iter().all(|p| !b.contains(p)));
        assert!(reuses_files(199));
    }

    #[test]
    fn small_folder_wraps_and_empty_is_empty() {
        assert_eq!(file_set(&names(3), 4).len(), FILES_PER_K);
        assert!(file_set(&[], 0).is_empty());
    }

    #[test]
    fn every_k_reads_every_byte_once() {
        let d = TempDir::new("sweep");
        let files: Vec<PathBuf> = (0..FILES_PER_K)
            .map(|i| d.write(&format!("{i}.jpg"), 1000))
            .collect();
        for k in KS {
            let r = run_k(&files, k);
            assert_eq!(r.bytes, 40_000, "K={k}");
            assert!(r.errors.is_empty());
        }
    }

    #[test]
    fn unreadable_files_are_counted_not_fatal() {
        let r = run_k(&names(5), 4);
        assert_eq!(r.bytes, 0);
        assert_eq!(r.errors.len(), 5);
        assert_eq!(r.mb_per_s(), 0.0);
    }
}
