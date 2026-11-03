//! Deterministic pixel sources. No randomness except the explicit xorshift
//! stream passed in, so the same call always yields the same buffer.

use crate::rng::XorShift64;
use image::{Rgb, RgbImage, Rgba, RgbaImage};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

/// Colours of the three GIF frames, in order.
pub const GIF_COLORS: [[u8; 4]; 3] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];

/// Orientation marker: TL red, TR green, BL blue, BR white. Distinct on both
/// axes, so every one of the 8 EXIF orientations looks different.
pub fn quadrants(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        Rgb(match (x < w / 2, y < h / 2) {
            (true, true) => RED,
            (false, true) => GREEN,
            (true, false) => BLUE,
            (false, false) => WHITE,
        })
    })
}

fn lerp(i: u32, len: u32) -> u8 {
    (i as u64 * 255 / len.saturating_sub(1).max(1) as u64) as u8
}

/// Left-to-right gradient; green and blue vary too so JPEG chroma is exercised.
pub fn horizontal_gradient(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, _| {
        let t = lerp(x, w);
        Rgb([t, 128, 255 - t])
    })
}

pub fn vertical_gradient(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |_, y| {
        let t = lerp(y, h);
        Rgb([t, 128, 255 - t])
    })
}

fn jittered(base: u8, rng: &mut XorShift64, range: i32) -> u8 {
    (base as i32 + rng.jitter(range)).clamp(0, 255) as u8
}

/// Diagonal gradient plus per-channel noise. `from_fn` visits pixels in a
/// fixed row-major order, which is what makes the noise stream reproducible.
pub fn gradient_noise(w: u32, h: u32, rng: &mut XorShift64, range: i32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        let r = lerp(x, w);
        let g = lerp(y, h);
        let b = ((r as u16 + g as u16) / 2) as u8;
        Rgb([
            jittered(r, rng, range),
            jittered(g, rng, range),
            jittered(b, rng, range),
        ])
    })
}

/// `cell`-pixel black/white squares.
pub fn checkerboard(size: u32, cell: u32) -> RgbImage {
    RgbImage::from_fn(size, size, |x, y| {
        if (x / cell + y / cell).is_multiple_of(2) {
            Rgb([0, 0, 0])
        } else {
            Rgb(WHITE)
        }
    })
}

/// Left half fully transparent, right half opaque. The RGB under the
/// transparent half is non-zero on purpose: a decoder that drops alpha shows
/// a visible colour there instead of black.
pub fn alpha_split(size: u32) -> RgbaImage {
    RgbaImage::from_fn(size, size, |x, _| {
        Rgba([0, 128, 255, if x < size / 2 { 0 } else { 255 }])
    })
}

pub fn solid_rgba(size: u32, rgba: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(size, size, Rgba(rgba))
}

/// Small image whose colour depends on `tint`, so tree files differ.
pub fn tinted(size: u32, tint: u8) -> RgbImage {
    RgbImage::from_fn(size, size, |x, y| Rgb([tint, lerp(x, size), lerp(y, size)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SEED;

    #[test]
    fn quadrant_corners_have_the_documented_colours() {
        let img = quadrants(300, 200);
        assert_eq!(img.get_pixel(0, 0).0, RED);
        assert_eq!(img.get_pixel(299, 0).0, GREEN);
        assert_eq!(img.get_pixel(0, 199).0, BLUE);
        assert_eq!(img.get_pixel(299, 199).0, WHITE);
    }

    #[test]
    fn noise_is_reproducible_and_bounded() {
        let a = gradient_noise(64, 64, &mut XorShift64::new(SEED), 16);
        let b = gradient_noise(64, 64, &mut XorShift64::new(SEED), 16);
        assert_eq!(a, b);
        let plain = gradient_noise(64, 64, &mut XorShift64::new(SEED), 0);
        for (p, q) in a.pixels().zip(plain.pixels()) {
            for c in 0..3 {
                assert!((p.0[c] as i32 - q.0[c] as i32).abs() <= 16);
            }
        }
    }

    #[test]
    fn alpha_split_halves() {
        let img = alpha_split(128);
        assert_eq!(img.get_pixel(63, 5).0[3], 0);
        assert_eq!(img.get_pixel(64, 5).0[3], 255);
    }

    #[test]
    fn gradients_span_full_range() {
        let img = horizontal_gradient(6000, 1);
        assert_eq!(img.get_pixel(0, 0).0[0], 0);
        assert_eq!(img.get_pixel(5999, 0).0[0], 255);
    }
}
