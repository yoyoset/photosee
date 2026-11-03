//! Logical path <-> filesystem path mapping and access policy. Package H
//! (orchestrator; security-sensitive). Logical paths are what the API speaks:
//! - `Policy::AnyLocal` (desktop): the native absolute path, e.g. `C:\a\b.jpg`
//!   or `\nas\photos\x.jpg`, UTF-8.
//! - `Policy::Roots` (Docker): `/<root name>/<relative path>` with '/'
//!   separators; resolution must stay inside the root after normalization
//!   (no `..`, no symlink escape).

use photosee_core::{Error, ErrorCode, Result};
use std::path::PathBuf;

pub struct Root {
    pub name: String,
    pub dir: PathBuf,
}

pub enum Policy {
    AnyLocal,
    Roots(Vec<Root>),
}

impl Policy {
    pub fn resolve(&self, logical: &str) -> Result<PathBuf> {
        let _ = logical;
        Err(Error::new(ErrorCode::Forbidden, "paths: not implemented")) // package H
    }

    pub fn to_logical(&self, physical: &std::path::Path) -> Result<String> {
        let _ = physical;
        Err(Error::new(ErrorCode::Forbidden, "paths: not implemented")) // package H
    }
}
