//! Directory listing: filter, classify, sort. Package A.

use crate::error::Result;
use crate::io::FsBackend;
use crate::types::Listing;
use std::path::Path;

/// Images and subdirectories of `dir`, each sorted with `sort::natural_cmp`.
/// Skips hidden entries (leading '.'), and OS litter such as `Thumbs.db`,
/// `@eaDir` (Synology) and `#recycle`.
pub fn list(fs: &dyn FsBackend, dir: &Path) -> Result<Listing> {
    let _ = (fs, dir);
    Ok(Listing::default()) // package A
}
