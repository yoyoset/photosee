//! Fit computation and SIMD resizing (fast_image_resize). Package C.

use crate::error::{Error, Result};
use crate::types::{Rgba8, Size};

/// Largest size with `src`'s aspect ratio that fits in `bound`. Never larger
/// than `src` unless `upscale`. Never returns a zero dimension.
pub fn fit(src: Size, bound: Size, upscale: bool) -> Size {
    let _ = (bound, upscale);
    src // package C
}

pub fn resize(img: &Rgba8, to: Size) -> Result<Rgba8> {
    let _ = (img, to);
    Err(Error::unsupported("resize: not implemented")) // package C
}
