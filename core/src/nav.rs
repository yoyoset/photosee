//! Folder-to-folder navigation for "next folder" and slideshow end-of-folder.
//! Package A. Rules (docs/ux-spec.md §folders):
//! - pre-order depth-first over `root`: a folder comes before its children,
//!   children in natural order; when siblings run out, climb and continue;
//! - the root itself is the first folder when it has images;
//! - skip folders that contain no images;
//! - never leave `root`; return `None` past either end.
//! Golden cases: contracts/nav.json.

use crate::error::Result;
use crate::io::FsBackend;
use std::path::{Path, PathBuf};

pub fn next_folder(fs: &dyn FsBackend, root: &Path, current: &Path) -> Result<Option<PathBuf>> {
    let _ = (fs, root, current);
    Ok(None) // package A
}

pub fn prev_folder(fs: &dyn FsBackend, root: &Path, current: &Path) -> Result<Option<PathBuf>> {
    let _ = (fs, root, current);
    Ok(None) // package A
}
