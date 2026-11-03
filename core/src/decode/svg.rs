//! SVG rasterization via resvg, for thumbnails and transcodes only (display
//! is passthrough). Package D.

use crate::error::{Error, Result};
use crate::io::ReadAt;
use crate::types::{ImageInfo, Rgba8, Size};

pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    let _ = src;
    Err(Error::unsupported("svg: not implemented")) // package D
}

pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let _ = (src, max);
    Err(Error::unsupported("svg: not implemented")) // package D
}
