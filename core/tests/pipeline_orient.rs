//! EXIF orientation: reading, applying, and exactly-once end to end.

#[path = "pipeline_util.rs"]
mod util;

use photosee_core::orient::{apply, read};
use photosee_core::pipeline::{Rendered, render};
use photosee_core::testfs::MemFs;
use photosee_core::{Format, Orientation, Rgba8, Size};
use std::path::Path;
use util::*;

/// 3x2 image of labelled pixels a..f (label in the red channel).
fn abc() -> Rgba8 {
    rgba(3, 2, |x, y| [b"abcdef"[(y * 3 + x) as usize], 0, 0, 255])
}

fn labels(img: &Rgba8) -> String {
    img.data.chunks_exact(4).map(|p| p[0] as char).collect()
}

#[test]
fn apply_matches_exif_definitions() {
    // Expected row-major output for each EXIF value, from the spec's
    // definitions (6 = rotate 90° clockwise to display, 8 = counter-clockwise).
    let want = [
        (1, (3, 2), "abcdef"),
        (2, (3, 2), "cbafed"),
        (3, (3, 2), "fedcba"),
        (4, (3, 2), "defabc"),
        (5, (2, 3), "adbecf"),
        (6, (2, 3), "daebfc"),
        (7, (2, 3), "fcebda"),
        (8, (2, 3), "cfbead"),
    ];
    for (v, (w, h), s) in want {
        let out = apply(abc(), Orientation::from_exif(v));
        assert_eq!(out.size, Size::new(w, h), "orientation {v}");
        assert_eq!(labels(&out), s, "orientation {v}");
    }
}

/// The orientation whose application undoes `v`.
fn inverse(v: u16) -> Orientation {
    Orientation::from_exif(match v {
        6 => 8,
        8 => 6,
        other => other,
    })
}

#[test]
fn read_jpeg_orientation_and_missing_tag() {
    let base = jpeg(&quadrants(16, 8), 90);
    for v in 1..=8 {
        let f = with_orientation(&base, v);
        assert_eq!(read(&f, Format::Jpeg).unwrap(), Orientation::from_exif(v));
    }
    assert_eq!(read(&base, Format::Jpeg).unwrap(), Orientation::Normal);
    // Damaged metadata is Normal, not an error.
    let mut broken = with_orientation(&base, 6);
    broken[12..20].fill(0xEE);
    assert_eq!(read(&broken, Format::Jpeg).unwrap(), Orientation::Normal);
}

#[test]
fn read_reads_only_the_head() {
    // 1 MiB of trailing data after a tagged JPEG: only the head is read.
    let mut f = with_orientation(&jpeg(&quadrants(16, 8), 90), 6);
    f.resize(f.len() + (1 << 20), 0);
    let src = Counting::new(f);
    assert_eq!(read(&src, Format::Jpeg).unwrap(), Orientation::Rotate90);
    assert!(src.bytes_read() <= 256 * 1024, "read {}", src.bytes_read());
}

/// Display image 64x48 (quadrants), stored rotated so that EXIF `v` brings
/// it back. After render: right size, right corners, no EXIF in the output.
#[test]
fn every_orientation_applied_exactly_once_end_to_end() {
    let fs = MemFs::new();
    let display = quadrants(64, 48);
    for v in 1..=8u16 {
        let stored = apply(display.clone(), inverse(v));
        let file = with_orientation(&jpeg(&stored, 95), v);
        fs.file(&format!("/p/o{v}.jpg"), &file);
    }
    for v in 1..=8u16 {
        let path = format!("/p/o{v}.jpg");
        let info = photosee_core::pipeline::info(&fs, Path::new(&path)).unwrap();
        assert_eq!(info.orientation, Orientation::from_exif(v));
        assert_eq!(info.display_size(), Size::new(64, 48), "info {v}");
        // 32x32 request: 3072 px > 1.5 x 1024, so it is transcoded.
        let r = render(&fs, Path::new(&path), Size::new(32, 32), &[]).unwrap();
        let Rendered::Transcoded { encoded, .. } = r else {
            panic!("orientation {v}: expected a transcode");
        };
        assert_eq!(encoded.mime, "image/jpeg");
        assert!(
            !has_exif(&encoded.bytes),
            "orientation {v}: output has EXIF"
        );
        let out = load(&encoded.bytes);
        assert_eq!(out.dimensions(), (32, 24), "orientation {v}");
        for (x, y, want) in [(6, 5, RED), (25, 5, GREEN), (6, 18, BLUE), (25, 18, WHITE)] {
            let got = out.get_pixel(x, y).0;
            assert!(
                near(&got, want, 40),
                "orientation {v} at ({x},{y}): {got:?}"
            );
        }
    }
}

#[test]
fn small_native_jpeg_passes_through_with_its_tag() {
    // The webview applies the tag; core must not touch the bytes.
    let fs = MemFs::new();
    fs.file(
        "/p/a.jpg",
        &with_orientation(&jpeg(&quadrants(64, 48), 90), 6),
    );
    let r = render(&fs, Path::new("/p/a.jpg"), Size::new(2560, 1440), &[]).unwrap();
    let Rendered::Passthrough { mime, info, .. } = r else {
        panic!("expected passthrough");
    };
    assert_eq!(mime, "image/jpeg");
    assert_eq!(info.display_size(), Size::new(48, 64));
}
