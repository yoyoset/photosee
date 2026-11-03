//! Thumbnails for the filmstrip. Package F. Prefer the EXIF IFD1 embedded
//! thumbnail (a few KB, no decode); otherwise decode and downscale. Animated
//! images use their first frame.

use crate::error::{Error, Result};
use crate::io::FsBackend;
use crate::types::Encoded;
use std::path::Path;

/// Longest edge of a generated thumbnail, in pixels.
pub const DEFAULT_EDGE: u32 = 256;

pub fn make(fs: &dyn FsBackend, path: &Path, edge: u32) -> Result<Encoded> {
    let _ = (fs, path, edge);
    Err(Error::unsupported("thumb: not implemented")) // package F
}
