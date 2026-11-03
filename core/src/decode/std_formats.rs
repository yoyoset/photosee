//! JPEG PNG GIF WebP BMP ICO TIFF via the `image` crate. Package C.
//! Set `image::Limits` (MAX_PIXELS) so a hostile or giant TIFF cannot exhaust
//! memory. AVIF decoding is not compiled in (dav1d needs a C build we don't
//! ship): AVIF returns Unsupported here and is only passed through.

use crate::error::{Error, Result};
use crate::io::ReadAt;
use crate::types::{Format, ImageInfo, Rgba8, Size};

/// 200 MP: above any current camera, below what exhausts a 4GB phone.
pub const MAX_PIXELS: u64 = 200_000_000;

pub fn info(src: &dyn ReadAt, fmt: Format) -> Result<ImageInfo> {
    let _ = (src, fmt);
    Err(Error::unsupported("std_formats: not implemented")) // package C
}

pub fn decode(src: &dyn ReadAt, fmt: Format, max: Option<Size>) -> Result<Rgba8> {
    let _ = (src, fmt, max);
    Err(Error::unsupported("std_formats: not implemented")) // package C
}
