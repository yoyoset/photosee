//! Directory listing: filter, classify, sort. Package A.
//!
//! One `read_dir` and no per-file IO: on SMB every extra `stat` is a round
//! trip, and a 2000-file folder must list in <= 2 s.

use crate::error::Result;
use crate::io::FsBackend;
use crate::sort::natural_cmp;
use crate::types::{EntryKind, Listing};
use std::path::Path;

/// Names that are never photos, compared case-insensitively. `.DS_Store` is
/// also caught by the leading-dot rule; it is listed so the intent is visible.
const LITTER: [&str; 8] = [
    "thumbs.db",
    "desktop.ini",
    "@eadir",
    "#recycle",
    "#snapshot",
    "$recycle.bin",
    "system volume information",
    ".ds_store",
];

fn is_ignored(name: &str) -> bool {
    name.starts_with('.') || LITTER.iter().any(|l| name.eq_ignore_ascii_case(l))
}

/// Images and subdirectories of `dir`, each sorted with `sort::natural_cmp`.
/// Skips hidden entries (leading '.'), and OS litter such as `Thumbs.db`,
/// `@eaDir` (Synology) and `#recycle`.
pub fn list(fs: &dyn FsBackend, dir: &Path) -> Result<Listing> {
    let mut out = Listing::default();
    for e in fs.read_dir(dir)? {
        if is_ignored(&e.name) {
            continue;
        }
        // Exhaustive on purpose: a new EntryKind must be classified here.
        match e.kind {
            EntryKind::Image(_) => out.images.push(e),
            EntryKind::Dir => out.dirs.push(e),
        }
    }
    out.images.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    out.dirs.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testfs::MemFs;
    use crate::types::Entry;

    fn names(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn drops_hidden_and_litter_case_insensitively() {
        let fs = MemFs::new();
        fs.file("/r/a.jpg", b"x")
            .file("/r/.hidden.jpg", b"x")
            .file("/r/.DS_Store", b"x")
            .file("/r/THUMBS.DB", b"x")
            .file("/r/Desktop.ini", b"x")
            .dir("/r/@eaDir")
            .dir("/r/#recycle")
            .dir("/r/#snapshot")
            .dir("/r/$RECYCLE.BIN")
            .dir("/r/system volume information")
            .dir("/r/.git")
            .dir("/r/keep");
        let l = list(&fs, Path::new("/r")).unwrap();
        assert_eq!(names(&l.images), ["a.jpg"]);
        assert_eq!(names(&l.dirs), ["keep"]);
    }

    #[test]
    fn non_image_files_never_appear() {
        let fs = MemFs::new();
        fs.file("/r/notes.txt", b"x").file("/r/a.png", b"x");
        let l = list(&fs, Path::new("/r")).unwrap();
        assert_eq!(names(&l.images), ["a.png"]);
        assert!(l.dirs.is_empty());
    }

    #[test]
    fn images_and_dirs_are_each_naturally_sorted() {
        let fs = MemFs::new();
        fs.file("/r/img10.jpg", b"x")
            .file("/r/img2.jpg", b"x")
            .file("/r/B.jpg", b"x")
            .file("/r/b.jpg", b"x")
            .dir("/r/d10")
            .dir("/r/d2")
            .dir("/r/D2");
        let l = list(&fs, Path::new("/r")).unwrap();
        assert_eq!(
            names(&l.images),
            ["B.jpg", "b.jpg", "img2.jpg", "img10.jpg"]
        );
        assert_eq!(names(&l.dirs), ["D2", "d2", "d10"]);
    }

    #[test]
    fn missing_directory_is_not_found() {
        let fs = MemFs::new();
        let e = list(&fs, Path::new("/nope")).unwrap_err();
        assert_eq!(e.code, crate::error::ErrorCode::NotFound);
    }
}
