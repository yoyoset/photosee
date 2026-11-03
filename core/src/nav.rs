//! Folder-to-folder navigation for "next folder" and slideshow end-of-folder.
//! Package A. Rules (docs/ux-spec.md §folders):
//! - pre-order depth-first over `root`: a folder comes before its children,
//!   children in natural order; when siblings run out, climb and continue;
//! - the root itself is the first folder when it has images;
//! - skip folders that contain no images;
//! - never leave `root`; return `None` past either end.
//!
//! Golden cases: contracts/nav.json.
//!
//! Cost model: each folder looked at costs exactly one `read_dir` (one
//! `scan::list` gives both "has images?" and the child folders). A NAS root
//! may hold 10k folders, so we stop at the first neighbour instead of
//! building the whole sequence.

use crate::error::{Error, ErrorCode, Result};
use crate::io::FsBackend;
use crate::scan;
use crate::sort::natural_cmp;
use crate::types::{Entry, Listing};
use std::cmp::Ordering;
use std::path::{Component, Path, PathBuf};

/// Deepest descent from the starting folder. Real trees are a handful of
/// levels; the cap only exists so a directory-symlink loop on a NAS ends in
/// "no neighbour here" instead of a stack overflow (rule: no panics on bad
/// input).
const MAX_DEPTH: usize = 64;

pub fn next_folder(fs: &dyn FsBackend, root: &Path, current: &Path) -> Result<Option<PathBuf>> {
    check_inside(root, current)?;
    // Descend first: in pre-order a folder is followed by its own children.
    // `current` is read strictly; an error here is the user's folder failing.
    let own = scan::list(fs, current)?;
    if let Some(found) = first_among(fs, current, &own.dirs, MAX_DEPTH)? {
        return Ok(Some(found));
    }
    let mut cur = current.to_path_buf();
    while cur != root {
        let Some((parent, name)) = split(&cur) else {
            return Ok(None);
        };
        let siblings = scan::list(fs, &parent)?;
        let later: Vec<&Entry> = siblings
            .dirs
            .iter()
            .filter(|e| natural_cmp(&e.name, &name) == Ordering::Greater)
            .collect();
        for sib in later {
            if let Some(found) = first_in_subtree(fs, &parent.join(&sib.name), MAX_DEPTH)? {
                return Ok(Some(found));
            }
        }
        cur = parent;
    }
    Ok(None)
}

pub fn prev_folder(fs: &dyn FsBackend, root: &Path, current: &Path) -> Result<Option<PathBuf>> {
    check_inside(root, current)?;
    let mut cur = current.to_path_buf();
    while cur != root {
        let Some((parent, name)) = split(&cur) else {
            return Ok(None);
        };
        let siblings = scan::list(fs, &parent)?;
        // Previous siblings, nearest first: each contributes its deepest last
        // descendant (or itself) before we fall back to the parent.
        for sib in siblings.dirs.iter().rev() {
            if natural_cmp(&sib.name, &name) != Ordering::Less {
                continue;
            }
            if let Some(found) = last_in_subtree(fs, &parent.join(&sib.name), MAX_DEPTH)? {
                return Ok(Some(found));
            }
        }
        // The parent precedes all of its children in pre-order. We already
        // listed it, so this check is free.
        if !siblings.images.is_empty() {
            return Ok(Some(if parent == root {
                root.to_path_buf()
            } else {
                parent
            }));
        }
        cur = parent;
    }
    Ok(None)
}

/// `current` must be `root` or below it, with no `..` tricks. Component-wise
/// `starts_with`, so `/photos2` is not inside `/photos`.
fn check_inside(root: &Path, current: &Path) -> Result<()> {
    let escapes = current.components().any(|c| c == Component::ParentDir);
    if escapes || !current.starts_with(root) {
        return Err(Error::new(
            ErrorCode::Forbidden,
            format!("{} is outside {}", current.display(), root.display()),
        ));
    }
    Ok(())
}

fn split(dir: &Path) -> Option<(PathBuf, String)> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    Some((dir.parent()?.to_path_buf(), name))
}

/// A neighbour that vanished or cannot be read cannot show images, so it is
/// skipped. Timeouts and other IO errors still propagate: the NAS being down
/// must reach the user, not read as "no more folders".
fn list_neighbour(fs: &dyn FsBackend, dir: &Path) -> Result<Option<Listing>> {
    match scan::list(fs, dir) {
        Ok(l) => Ok(Some(l)),
        Err(e) if matches!(e.code, ErrorCode::NotFound | ErrorCode::Forbidden) => Ok(None),
        Err(e) => Err(e),
    }
}

/// First folder with images in pre-order over `dir` and its descendants.
fn first_in_subtree(fs: &dyn FsBackend, dir: &Path, depth: usize) -> Result<Option<PathBuf>> {
    let Some(listing) = list_neighbour(fs, dir)? else {
        return Ok(None);
    };
    if !listing.images.is_empty() {
        return Ok(Some(dir.to_path_buf()));
    }
    first_among(fs, dir, &listing.dirs, depth)
}

/// First folder with images among `children` of `parent` (each child before
/// its own descendants, children in the given order).
fn first_among(
    fs: &dyn FsBackend,
    parent: &Path,
    children: &[Entry],
    depth: usize,
) -> Result<Option<PathBuf>> {
    let Some(depth) = depth.checked_sub(1) else {
        return Ok(None);
    };
    for child in children {
        if let Some(found) = first_in_subtree(fs, &parent.join(&child.name), depth)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// Last folder with images in pre-order over `dir` and its descendants: the
/// deepest last child wins, `dir` itself only when no descendant has images.
fn last_in_subtree(fs: &dyn FsBackend, dir: &Path, depth: usize) -> Result<Option<PathBuf>> {
    let Some(listing) = list_neighbour(fs, dir)? else {
        return Ok(None);
    };
    if let Some(depth) = depth.checked_sub(1) {
        for child in listing.dirs.iter().rev() {
            if let Some(found) = last_in_subtree(fs, &dir.join(&child.name), depth)? {
                return Ok(Some(found));
            }
        }
    }
    Ok((!listing.images.is_empty()).then(|| dir.to_path_buf()))
}
