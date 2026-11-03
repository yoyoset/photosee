//! PSD merged (composite) image via zune-psd. PSB and ZIP-compressed PSD are
//! non-goals and return Unsupported. Package D.

use crate::error::{Error, Result};
use crate::io::ReadAt;
use crate::types::{ImageInfo, Rgba8, Size};

pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    let _ = src;
    Err(Error::unsupported("psd: not implemented")) // package D
}

pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let _ = (src, max);
    Err(Error::unsupported("psd: not implemented")) // package D
}
