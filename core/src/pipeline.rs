//! End-to-end image rendering used by the server's /img and /meta routes.
//! Package C. Synchronous: the server runs it on a blocking thread.

use crate::error::{Error, Result};
use crate::io::FsBackend;
use crate::types::{Delivery, Encoded, Format, ImageInfo, Size};
use std::path::Path;

pub enum Rendered {
    /// Serve the file (or a byte range of it) unchanged.
    Passthrough {
        delivery: Delivery,
        mime: &'static str,
        info: ImageInfo,
    },
    /// Serve these encoded bytes.
    Transcoded { encoded: Encoded, info: ImageInfo },
}

/// Sniff, read info, choose delivery and — when transcoding — decode, orient,
/// color-convert, resize and encode. `native` = formats the client decodes.
pub fn render(
    fs: &dyn FsBackend,
    path: &Path,
    request: Size,
    native: &[Format],
) -> Result<Rendered> {
    let _ = (fs, path, request, native);
    Err(Error::unsupported("pipeline: not implemented")) // package C
}

/// Image facts without decoding pixels.
pub fn info(fs: &dyn FsBackend, path: &Path) -> Result<ImageInfo> {
    let _ = (fs, path);
    Err(Error::unsupported("pipeline: not implemented")) // package C
}
