//! HEIC decode through WIC on a file made at test time: a 64x48 image with
//! a red top-left quadrant, so size, marker position and RGBA order are all
//! checked. Skips with a note when the HEIF encoder (HEVC extension) is
//! missing. Also decodes private samples when PHOTOSEE_PRIVATE_FIXTURES is set.
#![cfg(windows)]

#[path = "heic_enc.rs"]
mod heic_enc;

use photosee_core::decode::heic_wic::{decode, info};
use photosee_core::resize;
use photosee_core::types::{Format, Orientation, Rgba8, Size};
use std::time::Instant;

fn sample() -> Option<Vec<u8>> {
    match heic_enc::encode_heic(64, 48, &heic_enc::quadrant_bgra(64, 48)) {
        Ok(b) => Some(b),
        Err(e) => {
            println!("skipped: no WIC HEIF encoder ({e})");
            None
        }
    }
}

fn px(img: &Rgba8, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * img.size.w + x) * 4) as usize;
    [
        img.data[o],
        img.data[o + 1],
        img.data[o + 2],
        img.data[o + 3],
    ]
}

/// HEVC is lossy and 4:2:0, so "red" is "clearly red", not 255,0,0.
fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60 && p[3] == 255
}

fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 60 && p[1] < 60 && p[3] == 255
}

#[test]
fn info_reports_size_and_normal_orientation() {
    let Some(heic) = sample() else { return };
    let inf = info(&heic).expect("info");
    assert_eq!(inf.format, Format::Heic);
    assert_eq!(inf.size, Size::new(64, 48));
    assert_eq!(inf.orientation, Orientation::Normal);
    assert!(!inf.animated);
}

#[test]
fn decode_returns_rgba_with_marker_top_left() {
    let Some(heic) = sample() else { return };
    let img = decode(&heic, None).expect("decode");
    assert_eq!(img.size, Size::new(64, 48));
    assert_eq!(img.data.len(), 64 * 48 * 4);
    // Sample away from the quadrant edges, where chroma bleeds.
    assert!(is_red(px(&img, 8, 8)), "top-left {:?}", px(&img, 8, 8));
    assert!(is_red(px(&img, 26, 18)));
    for (x, y) in [(56, 8), (8, 40), (56, 40), (38, 30)] {
        assert!(is_blue(px(&img, x, y)), "({x},{y}) {:?}", px(&img, x, y));
    }
}

#[test]
fn decode_with_max_never_exceeds_the_fit() {
    let Some(heic) = sample() else { return };
    let max = Size::new(32, 32);
    let img = decode(&heic, Some(max)).expect("decode");
    // The Fant reduction follows resize::fit; the caller resizes the rest.
    assert_eq!(img.size, resize::fit(Size::new(64, 48), max, false));
    assert_eq!(img.data.len(), (img.size.w * img.size.h * 4) as usize);
    assert!(is_red(px(&img, img.size.w / 8, img.size.h / 8)));
    assert!(is_blue(px(&img, img.size.w * 7 / 8, img.size.h * 7 / 8)));
    // A bound larger than the image never upscales.
    let big = decode(&heic, Some(Size::new(1000, 1000))).expect("decode");
    assert_eq!(big.size, Size::new(64, 48));
}

fn is_heic(p: &std::path::Path) -> bool {
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    matches!(ext.as_deref(), Some("heic" | "heif" | "hif"))
}

#[test]
fn private_fixtures_decode() {
    let Ok(root) = std::env::var("PHOTOSEE_PRIVATE_FIXTURES") else {
        return println!("skipped: PHOTOSEE_PRIVATE_FIXTURES not set");
    };
    let dir = std::path::Path::new(&root).join("heic");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return println!("skipped: no {}", dir.display());
    };
    for path in entries.flatten().map(|e| e.path()).filter(|p| is_heic(p)) {
        let file = std::fs::File::open(&path).expect("open sample");
        let inf = info(&file).unwrap_or_else(|e| panic!("{}: info: {e}", path.display()));
        let t = Instant::now();
        let img = decode(&file, None).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        assert_eq!(img.size, inf.size);
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        println!(
            "{name}: {:?} {:?} decode {ms:.1} ms",
            inf.size, inf.orientation
        );
    }
}
