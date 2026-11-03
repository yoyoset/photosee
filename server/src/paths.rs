//! Logical path <-> filesystem path mapping and access policy. Package H
//! (orchestrator; security-sensitive). Logical paths are what the API speaks:
//! - `Policy::AnyLocal` (desktop): the native absolute path, e.g. `C:\a\b.jpg`
//!   or `\\nas\photos\x.jpg`, UTF-8.
//! - `Policy::Roots` (Docker): `/<root name>/<relative path>` with '/'
//!   separators; resolution must stay inside the root after normalization
//!   (no `..`, no symlink escape).
//!
//! Every check is on whole path segments; nothing here concatenates strings
//! into paths. The desktop policy is not a sandbox: the embedded server only
//! answers its own webview (token + Host, see guard.rs), and the user may
//! open any file they can read anyway.

use photosee_core::{Error, ErrorCode, Result};
use std::path::{Component, Path, PathBuf};

pub struct Root {
    pub name: String,
    pub dir: PathBuf,
}

pub enum Policy {
    AnyLocal,
    Roots(Vec<Root>),
}

fn forbidden(why: &str) -> Error {
    Error::new(ErrorCode::Forbidden, why)
}

impl Policy {
    pub fn resolve(&self, logical: &str) -> Result<PathBuf> {
        if logical.contains('\0') {
            return Err(forbidden("path contains NUL"));
        }
        match self {
            Self::AnyLocal => resolve_local(logical),
            Self::Roots(roots) => resolve_rooted(roots, logical),
        }
    }

    pub fn to_logical(&self, physical: &Path) -> Result<String> {
        match self {
            Self::AnyLocal => physical
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| forbidden("path is not valid UTF-8")),
            Self::Roots(roots) => rooted_logical(roots, physical),
        }
    }
}

fn resolve_local(logical: &str) -> Result<PathBuf> {
    let p = PathBuf::from(logical);
    if !p.is_absolute() {
        return Err(forbidden("path must be absolute"));
    }
    // `..` is harmless under AnyLocal but would make two spellings of one
    // file, which breaks listing caches and ETags; refuse it outright.
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(forbidden("path must not contain '..'"));
    }
    Ok(p)
}

/// A segment is safe when joining it can only ever add one level below the
/// current directory on every OS the server runs on.
fn safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        // '\' is a separator on Windows; ':' opens drive letters and NTFS
        // alternate data streams (`a.jpg:secret`).
        && !s.contains(['/', '\\', ':'])
        && !(cfg!(windows) && is_dos_device(s))
}

/// `CON`, `nul.jpg`, `COM1.png`... open devices on Windows, not files.
fn is_dos_device(s: &str) -> bool {
    let stem = s.split('.').next().unwrap_or(s).trim_end();
    let stem = stem.to_ascii_uppercase();
    let numbered = stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.as_bytes()[3].is_ascii_digit();
    numbered || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
}

fn resolve_rooted(roots: &[Root], logical: &str) -> Result<PathBuf> {
    let rest = logical
        .strip_prefix('/')
        .ok_or_else(|| forbidden("path must start with '/'"))?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let mut segs = rest.split('/');
    let name = segs.next().unwrap_or_default();
    let root = roots
        .iter()
        .find(|r| r.name == name)
        .ok_or_else(|| Error::new(ErrorCode::NotFound, "no such root"))?;
    let mut path = root.dir.clone();
    for seg in segs {
        if !safe_segment(seg) {
            return Err(forbidden("invalid path segment"));
        }
        path.push(seg);
    }
    check_no_escape(&root.dir, &path)?;
    Ok(path)
}

/// Symlinks inside a mounted root may point anywhere (`photos/x -> /etc`).
/// Canonicalize the deepest existing ancestor and require it to stay under
/// the canonical root. Skipped when the root is not on the real filesystem
/// (in-memory test backends), since there is nothing to follow there.
fn check_no_escape(root: &Path, path: &Path) -> Result<()> {
    let Ok(root_real) = std::fs::canonicalize(root) else {
        return Ok(());
    };
    let Some(existing) = path.ancestors().find(|a| a.exists()) else {
        return Ok(());
    };
    let real = std::fs::canonicalize(existing).map_err(Error::from)?;
    if real.starts_with(&root_real) {
        Ok(())
    } else {
        Err(forbidden("path leaves its root"))
    }
}

fn rooted_logical(roots: &[Root], physical: &Path) -> Result<String> {
    // Longest matching root wins, so nested roots map to the inner one.
    let (root, rel) = roots
        .iter()
        .filter_map(|r| physical.strip_prefix(&r.dir).ok().map(|rel| (r, rel)))
        .max_by_key(|(r, _)| r.dir.as_os_str().len())
        .ok_or_else(|| forbidden("path is outside every root"))?;
    let mut out = format!("/{}", root.name);
    for c in rel.components() {
        let Component::Normal(seg) = c else {
            return Err(forbidden("path is not normalized"));
        };
        let seg = seg
            .to_str()
            .ok_or_else(|| forbidden("path is not valid UTF-8"))?;
        out.push('/');
        out.push_str(seg);
    }
    Ok(out)
}
