//! JPEG through Windows Imaging Component with DCT-domain scaling. Package C2.
//!
//! Why: `image`'s zune-jpeg decodes a 24 MP photo in ~350 ms and cannot
//! reduce while decoding. WIC's `IWICBitmapSourceTransform::CopyPixels` can
//! ask the codec for a 1/2, 1/4 or 1/8 size, which skips most of the IDCT and
//! colour conversion. The caller (`pipeline::render`) still resizes to the
//! exact target afterwards, so the scale is the largest reduction that stays
//! at or above the target in both axes.
//!
//! Orientation, measured: WIC's JPEG decoder does NOT apply EXIF orientation
//! (`GetSize` is the stored size, pixels are as stored), unlike its HEIF
//! decoder. So `orient::apply` runs once on the already reduced pixels.
//!
//! Any failure here is not final: `std_formats::decode` falls back to the
//! `image` crate, which handles what WIC refuses (CMYK/gray are refused on
//! purpose below; arithmetic coding and 12-bit are refused by WIC itself).

mod com;

use crate::color;
use crate::decode::std_formats::{MAX_PIXELS, check_pixels};
use crate::error::{Error, ErrorCode, Result};
use crate::io::{self, ReadAt};
use crate::orient;
use crate::resize;
use crate::types::{Format, Rgba8, Size};
use com::{Session, wic};
use windows::Win32::Graphics::Imaging::*;
use windows::core::Interface;

/// A JPEG this big is not a photo we can hold in memory twice.
const MAX_FILE: u64 = 256 * 1024 * 1024;
/// DCT scaling denominators WIC's JPEG codec offers, largest first.
const SCALES: [u32; 3] = [8, 4, 2];

fn ceil_div(n: u32, d: u32) -> u32 {
    n.div_ceil(d)
}

/// Largest DCT reduction (1, 2, 4 or 8) that keeps both sides >= `need`.
/// `ok` vets a candidate size (WIC must agree it is a size it can produce).
fn pick_scale(full: (u32, u32), need: (u32, u32), ok: impl Fn((u32, u32)) -> bool) -> u32 {
    for s in SCALES {
        let cand = (ceil_div(full.0, s), ceil_div(full.1, s));
        if cand.0 >= need.0 && cand.1 >= need.1 && ok(cand) {
            return s;
        }
    }
    1
}

/// Oriented, sRGB pixels, reduced toward `max` when WIC can do it cheaply.
pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let len = src.len()?;
    if len > MAX_FILE {
        let msg = format!("JPEG file is {len} bytes (limit {MAX_FILE})");
        return Err(Error::new(ErrorCode::TooLarge, msg));
    }
    let bytes = io::read_all(src)?;
    // From the in-memory copy: a slice is a `ReadAt`, so no second trip to
    // a slow backend for the EXIF head.
    let orientation = orient::read(&bytes, Format::Jpeg)?;
    let s = Session::open(bytes)?;
    let (w, h) = s.size()?;
    check_pixels(Size::new(w, h))?;
    let swap = orientation.swaps_axes();
    let shown = if swap {
        Size::new(h, w)
    } else {
        Size::new(w, h)
    };
    let need = max.map_or(shown, |m| resize::fit(shown, m, false));
    let need = if swap {
        (need.h, need.w)
    } else {
        (need.w, need.h)
    };
    let tr: IWICBitmapSourceTransform = wic(s.frame.cast(), "QueryInterface")?;
    let scale = pick_scale((w, h), need, |c| closest(&tr, c) == Some(c));
    let to = (ceil_div(w, scale), ceil_div(h, scale));
    let mut img = bgr_to_rgba(Size::new(to.0, to.1), &copy_bgr(&tr, to)?);
    if let Some(icc) = s.icc() {
        color::to_srgb(&mut img, &icc)?;
    }
    Ok(orient::apply(img, orientation))
}

/// The size WIC would really produce when asked for `want`.
fn closest(tr: &IWICBitmapSourceTransform, want: (u32, u32)) -> Option<(u32, u32)> {
    let (mut w, mut h) = want;
    // SAFETY: both out-pointers are valid, live u32 locals.
    unsafe { tr.GetClosestSize(&mut w, &mut h) }.ok()?;
    Some((w, h))
}

/// Pixels at `to` in 24bpp BGR, the JPEG codec's native colour format. Gray
/// and CMYK files report another closest format and are refused here so the
/// `image` path (which converts them) takes over.
fn copy_bgr(tr: &IWICBitmapSourceTransform, to: (u32, u32)) -> Result<Vec<u8>> {
    let mut fmt = GUID_WICPixelFormat24bppBGR;
    // SAFETY: the out-pointer is a valid, live GUID local.
    wic(
        unsafe { tr.GetClosestPixelFormat(&mut fmt) },
        "GetClosestPixelFormat",
    )?;
    if fmt != GUID_WICPixelFormat24bppBGR {
        return Err(Error::unsupported("JPEG is not 24bpp colour (gray/CMYK)"));
    }
    let too_big = || Error::new(ErrorCode::TooLarge, "JPEG pixel buffer overflows");
    let stride = to.0.checked_mul(3).ok_or_else(too_big)?;
    let len = usize::try_from(u64::from(stride) * u64::from(to.1)).map_err(|_| too_big())?;
    let mut buf = vec![0u8; len];
    // SAFETY: a null rect means the whole source at the requested size; `buf`
    // holds exactly stride * height bytes and the wrapper passes that length.
    let r = unsafe {
        let none = WICBitmapTransformRotate0;
        tr.CopyPixels(std::ptr::null(), to.0, to.1, &fmt, none, stride, &mut buf)
    };
    wic(r, "CopyPixels")?;
    Ok(buf)
}

fn bgr_to_rgba(size: Size, bgr: &[u8]) -> Rgba8 {
    debug_assert!(size.pixels() <= MAX_PIXELS);
    let mut data = vec![255u8; bgr.len() / 3 * 4];
    for (d, s) in data.chunks_exact_mut(4).zip(bgr.chunks_exact(3)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
    }
    Rgba8 { size, data }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_is_largest_that_keeps_the_target() {
        let any = |_| true;
        // 6000x4000 -> 2160x1440: 1/2 gives 3000x2000, 1/4 gives 1500x1000.
        assert_eq!(pick_scale((6000, 4000), (2160, 1440), any), 2);
        assert_eq!(pick_scale((6000, 4000), (700, 500), any), 8);
        assert_eq!(pick_scale((6000, 4000), (1500, 1000), any), 4);
        assert_eq!(pick_scale((6000, 4000), (1501, 1000), any), 2);
        // Already at or below the target: no reduction.
        assert_eq!(pick_scale((100, 80), (100, 80), any), 1);
        // Odd sizes round up, as the codec does.
        assert_eq!(pick_scale((1001, 1001), (126, 126), any), 8);
        assert_eq!(pick_scale((1001, 1001), (127, 127), any), 4);
        // A size WIC would not produce is skipped.
        assert_eq!(pick_scale((6000, 4000), (700, 500), |c| c.0 != 750), 4);
    }
}
