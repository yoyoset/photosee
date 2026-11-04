//! The WIC JPEG encoder (Windows): output validity, sizes, quality. Decoded
//! with the `image` crate on purpose: an independent reader of WIC's bytes.
#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "pipeline_util.rs"]
mod util;

use photosee_core::encode::{encode, jpeg_image};
use photosee_core::encode_wic;
use util::*;

fn quad(img: &image::RgbaImage, qx: u32, qy: u32) -> [u8; 4] {
    let (w, h) = img.dimensions();
    img.get_pixel(w / 4 + qx * w / 2, h / 4 + qy * h / 2).0
}

#[test]
fn output_is_a_valid_jpeg_with_the_right_pixels_and_no_exif() {
    let src = quadrants(200, 120);
    let bytes = encode_wic::jpeg(&src, 85).unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    assert_eq!(&bytes[bytes.len() - 2..], &[0xFF, 0xD9]);
    assert!(!has_exif(&bytes), "orientation must not be written");
    let img = load(&bytes);
    assert_eq!(img.dimensions(), (200, 120));
    for (q, want) in [
        ((0, 0), RED),
        ((1, 0), GREEN),
        ((0, 1), BLUE),
        ((1, 1), WHITE),
    ] {
        assert!(
            near(&quad(&img, q.0, q.1), want, 20),
            "{q:?}: {:?}",
            quad(&img, q.0, q.1)
        );
    }
}

#[test]
fn odd_and_tiny_sizes() {
    for (w, h) in [(1, 1), (2, 3), (17, 9), (33, 1), (1, 40), (257, 129)] {
        let bytes = encode_wic::jpeg(&quadrants(w, h), 85).unwrap();
        assert_eq!(load(&bytes).dimensions(), (w, h), "{w}x{h}");
    }
}

#[test]
fn quality_changes_the_size_the_way_the_other_encoder_does() {
    let src = rgba(320, 240, |x, y| {
        let n = ((x * 7 + y * 13) % 29) as u8;
        [
            ((x * 200 / 320) as u8) + n,
            ((y * 200 / 240) as u8) + n,
            n * 8,
            255,
        ]
    });
    let sizes: Vec<usize> = [30, 60, 85, 95]
        .iter()
        .map(|&q| encode_wic::jpeg(&src, q).unwrap().len())
        .collect();
    assert!(sizes.windows(2).all(|p| p[0] < p[1]), "{sizes:?}");
    // Same ballpark as `image` at the same setting (not 3x either way).
    let ours = encode_wic::jpeg(&src, 85).unwrap().len() as f64;
    let theirs = jpeg_image(&src, 85).unwrap().len() as f64;
    assert!((0.33..3.0).contains(&(ours / theirs)), "{ours} vs {theirs}");
}

#[test]
fn encode_picks_jpeg_for_opaque_and_png_for_alpha() {
    let opaque = encode(&quadrants(64, 48), 85).unwrap();
    assert_eq!(opaque.mime, "image/jpeg");
    assert_eq!(load(&opaque.bytes).dimensions(), (64, 48));
    let mut see_through = quadrants(64, 48);
    see_through.data[3] = 100;
    assert_eq!(encode(&see_through, 85).unwrap().mime, "image/png");
}
