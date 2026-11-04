//! Fit computation and SIMD resizing (fast_image_resize). Package C.

use crate::error::{Error, Result};
use crate::types::{Rgba8, Size};
use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

/// Filter for every transcode. Chosen by `core/tests/pipeline_bench.rs`
/// (`filters`), see the numbers there; filled in after measuring.
pub const FILTER: FilterType = FilterType::CatmullRom;

/// Largest size with `src`'s aspect ratio that fits in `bound`. Never larger
/// than `src` unless `upscale`. Never returns a zero dimension.
///
/// Symmetric in the axes (fit(src.T, bound.T) == fit(src, bound).T), so a
/// size computed before or after a 90° rotation agrees.
pub fn fit(src: Size, bound: Size, upscale: bool) -> Size {
    let src = Size::new(src.w.max(1), src.h.max(1));
    let bound = Size::new(bound.w.max(1), bound.h.max(1));
    if !upscale && src.w <= bound.w && src.h <= bound.h {
        return src;
    }
    let (sw, sh, bw, bh) = (
        u64::from(src.w),
        u64::from(src.h),
        u64::from(bound.w),
        u64::from(bound.h),
    );
    // Round half up; the result never exceeds the bound because the exact
    // value is <= the bound, which is an integer.
    let scale = |num: u64, den: u64| ((2 * num + den) / (2 * den)).max(1) as u32;
    if bw * sh <= bh * sw {
        Size::new(bound.w, scale(sh * bw, sw)) // width-limited
    } else {
        Size::new(scale(sw * bh, sh), bound.h)
    }
}

pub fn resize(img: &Rgba8, to: Size) -> Result<Rgba8> {
    if to.w == 0 || to.h == 0 {
        return Err(Error::corrupt(format!("resize to {}x{}", to.w, to.h)));
    }
    if to == img.size {
        return Ok(img.clone());
    }
    let src = ImageRef::new(img.size.w, img.size.h, &img.data, PixelType::U8x4)
        .map_err(|e| Error::corrupt(format!("resize source: {e}")))?;
    let mut dst = Image::new(to.w, to.h, PixelType::U8x4);
    // Premultiplying is needed for correct edges around transparency
    // (no colour bleeding from invisible pixels), and wasted on opaque images.
    let opts = ResizeOptions::new()
        .resize_alg(ResizeAlg::Convolution(FILTER))
        .use_alpha(crate::encode::has_alpha(img));
    Resizer::new()
        .resize(&src, &mut dst, &opts)
        .map_err(|e| Error::corrupt(format!("resize: {e}")))?;
    Ok(Rgba8 {
        size: to,
        data: dst.into_vec(),
    })
}
