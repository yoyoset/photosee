//! Color management: convert embedded ICC profiles to sRGB with moxcms so P3
//! (iPhone) and AdobeRGB photos don't look washed out after transcoding.
//! Package C.

use crate::error::Result;
use crate::types::Rgba8;
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};

/// Rows of work per transform call: bounds the scratch buffer (moxcms does
/// not transform in place without its `in_place` feature).
const CHUNK_BYTES: usize = 256 * 1024;

/// Colorant tolerance for "already sRGB". Real sRGB profiles (HP's v2,
/// ICC's v4, camera vendors') differ from each other by up to ~1e-3 after
/// s15Fixed16 rounding and chromatic adaptation; P3/AdobeRGB differ by >0.05.
const COLORANT_EPS: f64 = 2e-3;
/// Max difference of the linearized 8-bit TRC tables (0..1 scale).
const TRC_EPS: f32 = 2e-3;

/// A profile that is already sRGB, fails to parse, or cannot build a
/// transform (CMYK, gray, LUT-only oddities) leaves the pixels untouched and
/// returns Ok: a bad profile must never fail a photo.
pub fn to_srgb(img: &mut Rgba8, icc: &[u8]) -> Result<()> {
    let Ok(src) = ColorProfile::new_from_slice(icc) else {
        return Ok(());
    };
    let srgb = ColorProfile::new_srgb();
    if is_srgb(&src, &srgb) {
        return Ok(());
    }
    let opts = TransformOptions::default();
    let Ok(t) = src.create_transform_8bit(Layout::Rgba, &srgb, Layout::Rgba, opts) else {
        return Ok(());
    };
    let mut scratch = vec![0u8; CHUNK_BYTES.min(img.data.len())];
    // CHUNK_BYTES is a multiple of 4, so every chunk holds whole pixels.
    for chunk in img.data.chunks_mut(CHUNK_BYTES) {
        let out = &mut scratch[..chunk.len()];
        if t.transform(chunk, out).is_err() {
            return Ok(()); // only fails on length mismatch, impossible here
        }
        chunk.copy_from_slice(out);
    }
    Ok(())
}

/// Compares the parsed profile, not bytes: the many sRGB profiles in the
/// wild are byte-different but colorimetrically the same.
fn is_srgb(p: &ColorProfile, srgb: &ColorProfile) -> bool {
    if p.color_space != DataColorSpace::Rgb || !p.is_matrix_shaper() {
        return false;
    }
    let near = |a: moxcms::Xyzd, b: moxcms::Xyzd| {
        (a.x - b.x).abs() < COLORANT_EPS
            && (a.y - b.y).abs() < COLORANT_EPS
            && (a.z - b.z).abs() < COLORANT_EPS
    };
    if !(near(p.red_colorant, srgb.red_colorant)
        && near(p.green_colorant, srgb.green_colorant)
        && near(p.blue_colorant, srgb.blue_colorant))
    {
        return false;
    }
    let Ok(want) = srgb.build_8bit_lin_table(&srgb.red_trc) else {
        return false;
    };
    [&p.red_trc, &p.green_trc, &p.blue_trc].iter().all(|trc| {
        p.build_8bit_lin_table(trc).is_ok_and(|t| {
            t.iter()
                .zip(want.iter())
                .all(|(a, b)| (a - b).abs() < TRC_EPS)
        })
    })
}
