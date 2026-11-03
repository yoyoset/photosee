//! Output encoding for transcodes. Package C. JPEG for opaque images, PNG
//! when any pixel has alpha < 255.

use crate::error::{Error, Result};
use crate::types::{Encoded, Rgba8};

pub fn encode(img: &Rgba8, jpeg_quality: u8) -> Result<Encoded> {
    let _ = (img, jpeg_quality);
    Err(Error::unsupported("encode: not implemented")) // package C
}
