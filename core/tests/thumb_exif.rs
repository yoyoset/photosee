//! `meta::exif_summary`.
#![allow(clippy::unwrap_used, reason = "test code")]

#[path = "thumb_util.rs"]
mod util;

use photosee_core::meta::exif_summary;
use photosee_core::{ErrorCode, ExifSummary};
use std::path::Path;
use util::*;

fn summary(spec: ExifSpec) -> ExifSummary {
    let jpg = jpeg(&quadrants(64, 48), 85);
    let fs = CountFs::with("/a.jpg", &jpeg_with_exif(&jpg, spec));
    exif_summary(&fs, Path::new("/a.jpg")).unwrap()
}

fn exposure(num: u32, den: u32) -> Option<String> {
    summary(ExifSpec {
        exif: vec![rational(0x829A, num, den)],
        ..ExifSpec::default()
    })
    .exposure
}

#[test]
fn full_summary() {
    let s = summary(ExifSpec {
        ifd0: vec![ascii(0x010F, "Canon"), ascii(0x0110, "Canon EOS R6")],
        exif: vec![
            rational(0x829A, 1, 250),
            rational(0x829D, 28, 10),
            short(0x8827, 400),
            ascii(0x9003, "2024:05:01 13:22:10"),
            rational(0x920A, 85, 1),
            ascii(0xA434, "RF85mm F2 MACRO IS STM"),
        ],
        ..ExifSpec::default()
    });
    assert_eq!(s.camera.as_deref(), Some("Canon EOS R6"));
    assert_eq!(s.lens.as_deref(), Some("RF85mm F2 MACRO IS STM"));
    assert_eq!(s.iso, Some(400));
    assert_eq!(s.exposure.as_deref(), Some("1/250"));
    assert_eq!(s.f_number, Some(2.8));
    assert_eq!(s.focal_mm, Some(85.0));
    assert_eq!(s.taken.as_deref(), Some("2024:05:01 13:22:10"));
}

#[test]
fn make_is_prefixed_unless_model_repeats_it() {
    let cam = |make, model| {
        summary(ExifSpec {
            ifd0: vec![ascii(0x010F, make), ascii(0x0110, model)],
            ..ExifSpec::default()
        })
        .camera
    };
    assert_eq!(
        cam("Canon", "Canon EOS R6").as_deref(),
        Some("Canon EOS R6")
    );
    assert_eq!(cam("SONY", "ILCE-7M4").as_deref(), Some("SONY ILCE-7M4"));
    assert_eq!(cam("FUJIFILM", "X-T5 ").as_deref(), Some("FUJIFILM X-T5"));
}

#[test]
fn exposure_formatting() {
    assert_eq!(exposure(1, 250).as_deref(), Some("1/250"));
    assert_eq!(exposure(10, 600).as_deref(), Some("1/60"));
    assert_eq!(exposure(5, 2).as_deref(), Some("2.5"));
    assert_eq!(exposure(30, 1).as_deref(), Some("30"));
    assert_eq!(exposure(1, 0), None, "zero denominator is garbage");
}

#[test]
fn jpeg_without_exif_gives_default() {
    let fs = CountFs::with("/a.jpg", &jpeg(&quadrants(64, 48), 85));
    let s = exif_summary(&fs, Path::new("/a.jpg")).unwrap();
    assert_eq!(s, ExifSummary::default());
}

#[test]
fn damaged_exif_and_non_images_give_default() {
    let mut jpg = jpeg_with_exif(&jpeg(&quadrants(64, 48), 85), ExifSpec::default());
    jpg.truncate(30); // APP1 cut mid-segment
    let fs = CountFs::with("/a.jpg", &jpg);
    fs.inner.file("/b.jpg", b"plain text, not an image");
    fs.inner.file("/c.jpg", b"");
    for p in ["/a.jpg", "/b.jpg", "/c.jpg"] {
        let s = exif_summary(&fs, Path::new(p)).unwrap();
        assert_eq!(s, ExifSummary::default(), "{p}");
    }
}

#[test]
fn missing_file_is_not_found() {
    let fs = CountFs::with("/a.jpg", b"x");
    let e = exif_summary(&fs, Path::new("/nope.jpg")).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn reads_a_bounded_head_only() {
    let mut jpg = jpeg_with_exif(&jpeg(&quadrants(64, 48), 85), ExifSpec::default());
    jpg.resize(4 << 20, 0);
    let fs = CountFs::with("/big.jpg", &jpg);
    exif_summary(&fs, Path::new("/big.jpg")).unwrap();
    assert!(fs.bytes_read() <= 1 << 20, "read {}", fs.bytes_read());
}
