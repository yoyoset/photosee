//! Phase 1: directory listing, and the JPEG file set the other phases use.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct Listing {
    pub files: usize,
    pub bytes: u64,
    /// JPEGs in name order. Sorted after the clock stops: sorting is our cost,
    /// not the share's.
    pub jpegs: Vec<PathBuf>,
    pub elapsed: Duration,
    pub errors: Vec<String>,
}

pub fn is_jpeg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
}

/// One pass of `read_dir` plus `DirEntry::metadata()` per entry (top level
/// only). Pitfall: on Windows `DirEntry::metadata` is served from the
/// directory enumeration data, so it may look cheaper than a per-file `stat`
/// over SMB; that is exactly what the app's scanner will see, so we keep it.
/// `Err` only when the directory itself cannot be opened.
pub fn list_once(dir: &Path) -> Result<Listing, String> {
    let start = Instant::now();
    let rd = fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    let (mut files, mut bytes, mut jpegs, mut errors) = (0usize, 0u64, Vec::new(), Vec::new());
    for entry in rd {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                errors.push(format!("list entry: {e}"));
                continue;
            }
        };
        match entry.metadata() {
            Ok(m) if m.is_file() => {
                files += 1;
                bytes += m.len();
                if is_jpeg(&entry.path()) {
                    jpegs.push(entry.path());
                }
            }
            Ok(_) => {}
            Err(e) => errors.push(format!("metadata {}: {e}", entry.path().display())),
        }
    }
    let elapsed = start.elapsed();
    jpegs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    Ok(Listing {
        files,
        bytes,
        jpegs,
        elapsed,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_extension_matching() {
        assert!(is_jpeg(Path::new("a.JPG")));
        assert!(is_jpeg(Path::new("dir/b.jpeg")));
        assert!(!is_jpeg(Path::new("c.png")));
        assert!(!is_jpeg(Path::new("jpg")));
    }

    #[test]
    fn missing_dir_is_an_error_not_a_panic() {
        let r = list_once(Path::new("Z:/definitely/not/here/photosee-bench"));
        assert!(r.is_err());
    }
}
