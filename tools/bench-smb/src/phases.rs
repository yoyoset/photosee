//! Phases 2 and 3: head reads and first-image read. Everything here only opens
//! files for reading.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const HEAD_FILES: usize = 50;
pub const HEAD_BYTES: u64 = 64 * 1024;
const BUF: usize = 256 * 1024;

pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Reads `limit` bytes (or to EOF) and returns how many arrived. Streams into
/// a fixed buffer instead of `fs::read`, so a stray multi-GB "jpg" cannot make
/// the benchmark itself allocate gigabytes.
pub fn read_counting(path: &Path, limit: u64) -> io::Result<u64> {
    let mut f = File::open(path)?.take(limit);
    let mut buf = vec![0u8; BUF];
    let mut total = 0u64;
    loop {
        match f.read(&mut buf)? {
            0 => return Ok(total),
            n => total += n as u64,
        }
    }
}

pub fn read_full(path: &Path) -> io::Result<u64> {
    read_counting(path, u64::MAX)
}

pub struct HeadRun {
    pub latencies_ms: Vec<f64>,
    pub errors: Vec<String>,
}

/// Per-file latency of reading the first 64 KB (open + read), the cost of
/// sniffing a header/EXIF thumbnail. Failed files are reported, not timed.
pub fn head_reads(jpegs: &[PathBuf]) -> HeadRun {
    let mut run = HeadRun {
        latencies_ms: Vec::new(),
        errors: Vec::new(),
    };
    for p in jpegs.iter().take(HEAD_FILES) {
        let t = Instant::now();
        match read_counting(p, HEAD_BYTES) {
            Ok(_) => run.latencies_ms.push(ms(t.elapsed())),
            Err(e) => run.errors.push(format!("head {}: {e}", p.display())),
        }
    }
    run
}

pub struct FirstRun {
    pub ms: f64,
    pub bytes: u64,
}

/// Full read of the first JPEG: the lower bound of "time to first image"
/// (decode time is deliberately not included; this tool measures the share).
pub fn first_image(jpegs: &[PathBuf]) -> Result<FirstRun, String> {
    let p = jpegs.first().ok_or("no JPEG files found")?;
    let t = Instant::now();
    let bytes = read_full(p).map_err(|e| format!("first image {}: {e}", p.display()))?;
    Ok(FirstRun {
        ms: ms(t.elapsed()),
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn head_is_capped_and_full_is_complete() {
        let d = TempDir::new("phases");
        let p = d.write("a.jpg", 100_000);
        assert_eq!(read_counting(&p, HEAD_BYTES).unwrap(), HEAD_BYTES);
        assert_eq!(read_full(&p).unwrap(), 100_000);
    }

    #[test]
    fn unreadable_files_become_errors() {
        let missing = vec![PathBuf::from("no/such/file.jpg")];
        let run = head_reads(&missing);
        assert!(run.latencies_ms.is_empty());
        assert_eq!(run.errors.len(), 1);
        assert!(first_image(&missing).is_err());
        assert!(first_image(&[]).is_err());
    }
}
