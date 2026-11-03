//! HEIC through Windows Imaging Component. Package E. WIC needs both the
//! "HEIF Image Extensions" and the "HEVC Video Extensions"; when decoding
//! fails, probe which one is missing and return MissingCodec(HeifExt|HevcExt)
//! so the UI can tell the user exactly what to install.

use crate::error::{Error, Result};
use crate::io::ReadAt;
use crate::types::{ImageInfo, Rgba8, Size};

pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    let _ = src;
    Err(Error::unsupported("heic_wic: not implemented")) // package E
}

pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let _ = (src, max);
    Err(Error::unsupported("heic_wic: not implemented")) // package E
}
