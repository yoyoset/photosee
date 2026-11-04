//! The WIC JPEG path (Windows): DCT scaling, orientation, colour fallback and
//! corrupt input. Everything is generated in memory; no private samples.
#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "pipeline_util.rs"]
mod util;

use photosee_core::decode::jpeg_wic;
use photosee_core::decode::std_formats::{decode, decode_image};
use photosee_core::{ErrorCode, Format, Rgba8, Size};
use util::*;

/// Colour at the centre of quadrant `(qx, qy)` (each 0 or 1).
fn quad(img: &Rgba8, qx: u32, qy: u32) -> [u8; 4] {
    let (w, h) = (img.size.w, img.size.h);
    let (x, y) = (w / 4 + qx * w / 2, h / 4 + qy * h / 2);
    let o = ((y * w + x) * 4) as usize;
    img.data[o..o + 4].try_into().unwrap()
}

#[test]
fn all_eight_orientations_match_the_image_path() {
    let base = jpeg(&quadrants(64, 32), 95);
    for v in 1..=8u16 {
        let f = with_orientation(&base, v);
        let wic = decode(&f, Format::Jpeg, None).unwrap();
        let want = decode_image(&f, Format::Jpeg).unwrap();
        assert_eq!(wic.size, want.size, "orientation {v}: size");
        for (qx, qy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let (g, w) = (quad(&wic, qx, qy), quad(&want, qx, qy));
            assert!(
                near(&g, w, 16),
                "orientation {v} quadrant {qx}{qy}: {g:?} vs {w:?}"
            );
        }
    }
}

#[test]
fn reduces_in_the_dct_domain_when_the_target_is_small() {
    let f = jpeg(&quadrants(800, 600), 90);
    let cases = [
        (Size::new(400, 300), Size::new(400, 300)),   // 1/2
        (Size::new(200, 150), Size::new(200, 150)),   // 1/4
        (Size::new(100, 75), Size::new(100, 75)),     // 1/8
        (Size::new(300, 200), Size::new(400, 300)),   // 1/2: 1/4 would be too small
        (Size::new(4000, 3000), Size::new(800, 600)), // never upscales
    ];
    for (max, want) in cases {
        let img = decode(&f, Format::Jpeg, Some(max)).unwrap();
        assert_eq!(img.size, want, "max {max:?}");
        assert!(near(&quad(&img, 0, 0), RED, 12), "max {max:?}");
        assert!(near(&quad(&img, 1, 1), WHITE, 12), "max {max:?}");
    }
}

#[test]
fn a_rotated_target_reduces_along_the_stored_axes() {
    // Stored 800x600, displayed 600x800 (orientation 6). A 150x200 display
    // box needs stored >= 200x150 -> 1/4, giving display 150x200.
    let f = with_orientation(&jpeg(&quadrants(800, 600), 90), 6);
    let img = decode(&f, Format::Jpeg, Some(Size::new(150, 200))).unwrap();
    assert_eq!(img.size, Size::new(150, 200));
    // Orientation 6 turns the stored top-left (red) to the top-right.
    assert!(near(&quad(&img, 1, 0), RED, 12));
}

/// 8x8 four-component JPEG (Adobe marker, transform 0 = CMYK) whose every
/// block is DC 0 / EOB: tiny and valid, and a colour model WIC's 24bpp
/// path must hand over to `image`.
fn cmyk_jpeg() -> Vec<u8> {
    let mut j = vec![0xFF, 0xD8];
    j.extend([0xFF, 0xEE, 0, 14]);
    j.extend(b"Adobe");
    j.extend([0, 100, 0, 0, 0, 0, 0]);
    j.extend([0xFF, 0xDB, 0, 67, 0]);
    j.extend([1u8; 64]);
    j.extend([0xFF, 0xC0, 0, 20, 8, 0, 8, 0, 8, 4]);
    for id in 1..=4 {
        j.extend([id, 0x11, 0]);
    }
    for class_id in [0x00u8, 0x10] {
        j.extend([0xFF, 0xC4, 0, 20, class_id, 1]);
        j.extend([0u8; 15]);
        j.push(0); // one code of length 1 -> symbol 0 (DC size 0 / AC EOB)
    }
    j.extend([0xFF, 0xDA, 0, 14, 4]);
    for id in 1..=4 {
        j.extend([id, 0]);
    }
    j.extend([0, 63, 0]);
    j.push(0); // four blocks x two zero bits
    j.extend([0xFF, 0xD9]);
    j
}

#[test]
fn cmyk_is_refused_by_wic_and_decoded_by_the_fallback() {
    let f = cmyk_jpeg();
    let err = jpeg_wic::decode(&f, None).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported, "{err}");
    assert!(
        err.message.contains("CMYK"),
        "refused for the right reason: {err}"
    );
    let img = decode(&f, Format::Jpeg, None).expect("image decodes CMYK");
    assert_eq!(img.size, Size::new(8, 8));
}

#[test]
fn truncated_files_are_corrupt_not_a_wic_error() {
    let f = jpeg(&quadrants(256, 192), 90);
    for cut in [f.len() / 2, f.len() - 2, 100] {
        let err = decode(&f[..cut].to_vec(), Format::Jpeg, None).unwrap_err();
        assert_eq!(err.code, ErrorCode::Corrupt, "cut at {cut}: {err}");
    }
}

#[test]
fn garbage_after_the_header_falls_back_or_fails_cleanly() {
    let mut f = jpeg(&quadrants(256, 192), 90);
    let n = f.len();
    for b in &mut f[n / 2..n - 2] {
        *b = 0x5A; // keeps SOI and EOI, ruins the entropy-coded data
    }
    // Either decoder may cope; what matters is no panic and no WIC-only error.
    if let Err(e) = decode(&f, Format::Jpeg, None) {
        assert_eq!(e.code, ErrorCode::Corrupt, "{e}");
    }
}
