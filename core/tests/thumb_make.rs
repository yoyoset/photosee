//! `thumb::make`: the IFD1 shortcut, the decode path, orientation, alpha.
#![allow(clippy::unwrap_used, reason = "test code")]

#[path = "thumb_util.rs"]
mod util;

use image::codecs::gif::GifEncoder;
use image::{Delay, Frame};
use photosee_core::thumb::make;
use photosee_core::{ErrorCode, Rgba8};
use std::path::Path;
use util::*;

const MAIN: (u32, u32) = (3000, 2000);

/// A 3:2 photo with an IFD1 thumbnail of `tw`x`th` px and orientation `o`.
fn photo(o: u16, tw: u32, th: u32) -> Vec<u8> {
    let main = jpeg(&quadrants(MAIN.0, MAIN.1), 85);
    let spec = ExifSpec {
        ifd0: vec![short(0x0112, o)],
        exif: vec![long(0xA002, MAIN.0), long(0xA003, MAIN.1)],
        thumb: Some(jpeg(&quadrants(tw, th), 85)),
        thumb_range: None,
    };
    jpeg_with_exif(&main, spec)
}

fn run(bytes: &[u8], name: &str, edge: u32) -> (image::RgbaImage, u64) {
    let fs = CountFs::with(name, bytes);
    let out = make(&fs, Path::new(name), edge).unwrap();
    assert_eq!(out.mime, "image/jpeg");
    assert!(out.bytes.starts_with(&[0xFF, 0xD8]));
    (decode(&out.bytes), fs.bytes_read())
}

#[test]
fn ifd1_thumbnail_is_used_and_reads_little() {
    let bytes = photo(1, 160, 107);
    let (img, read) = run(&bytes, "/a.jpg", 256);
    assert_eq!((img.width(), img.height()), (160, 107), "no upscale");
    assert!(near(px(&img, 5, 5), RED, 40));
    assert!(near(px(&img, 150, 100), WHITE, 40));
    // Head read (<= 128 KiB) plus the sniff; the body is never touched.
    assert!(read <= 140 * 1024, "read {read} of {}", bytes.len());
}

#[test]
fn small_edge_downsizes_the_embedded_thumbnail() {
    let (img, _) = run(&photo(1, 160, 107), "/a.jpg", 64);
    assert_eq!(img.width().max(img.height()), 64);
}

#[test]
fn orientation_6_gives_portrait_with_marker_top_left() {
    // Rotate90 (CW): the stored bottom-left (blue) becomes top-left.
    let (img, read) = run(&photo(6, 160, 107), "/a.jpg", 256);
    assert!(read <= 140 * 1024, "IFD1 path expected, read {read}");
    assert!(
        img.height() > img.width(),
        "{}x{}",
        img.width(),
        img.height()
    );
    assert!(near(px(&img, 3, 3), BLUE, 40));
    assert!(near(px(&img, img.width() - 4, 3), RED, 40));
}

#[test]
fn decode_path_agrees_on_orientation() {
    // Same photo without IFD1: the main image is decoded, top-left still blue.
    let main = jpeg(&quadrants(1200, 800), 85);
    let spec = ExifSpec {
        ifd0: vec![short(0x0112, 6)],
        ..ExifSpec::default()
    };
    let bytes = jpeg_with_exif(&main, spec);
    let (img, read) = run(&bytes, "/a.jpg", 256);
    assert!(img.height() > img.width());
    assert_eq!(img.height(), 256);
    assert!(near(px(&img, 3, 3), BLUE, 40));
    assert!(read >= bytes.len() as u64 / 2, "decode path reads the file");
}

#[test]
fn too_small_thumbnail_falls_back_to_decode() {
    // 40 px < edge / 2.
    let bytes = photo(1, 40, 27);
    let (img, read) = run(&bytes, "/a.jpg", 256);
    assert_eq!(img.width(), 256);
    assert!(read >= bytes.len() as u64 / 2);
}

#[test]
fn letterboxed_thumbnail_falls_back_to_decode() {
    // A 4:3 thumbnail inside a 3:2 photo would show bars.
    let bytes = photo(1, 160, 120);
    let (img, read) = run(&bytes, "/a.jpg", 256);
    assert_eq!((img.width(), img.height()), (256, 171));
    assert!(read >= bytes.len() as u64 / 2);
}

#[test]
fn hostile_ifd1_range_falls_back_without_panic() {
    let ranges = [
        (0xFFFF_FFF0, 0x100),
        (10, 0xFFFF_FFFF),
        (0, 0),
        (u32::MAX, u32::MAX),
    ];
    for range in ranges {
        let main = jpeg(&quadrants(900, 600), 85);
        let spec = ExifSpec {
            thumb: Some(jpeg(&quadrants(160, 107), 85)),
            thumb_range: Some(range),
            ..ExifSpec::default()
        };
        let (img, _) = run(&jpeg_with_exif(&main, spec), "/a.jpg", 256);
        assert_eq!(img.width(), 256, "{range:?}");
    }
}

#[test]
fn garbage_thumbnail_bytes_fall_back() {
    let main = jpeg(&quadrants(900, 600), 85);
    let spec = ExifSpec {
        thumb: Some(vec![0xFF, 0xD8, 1, 2, 3, 4, 5, 6]),
        ..ExifSpec::default()
    };
    let (img, _) = run(&jpeg_with_exif(&main, spec), "/a.jpg", 256);
    assert_eq!(img.width(), 256);
}

#[test]
fn tiff_based_raw_uses_ifd1() {
    // A DNG-shaped TIFF: IFD0 + IFD1 thumbnail, no real raw data. The decode
    // path would fail on it, so success proves the shortcut.
    let spec = ExifSpec {
        ifd0: vec![short(0x0112, 1), ascii(0x010F, "Test")],
        exif: vec![long(0xA002, 6000), long(0xA003, 4000)],
        thumb: Some(jpeg(&quadrants(240, 160), 85)),
        thumb_range: None,
    };
    let (img, _) = run(&tiff(spec), "/a.dng", 256);
    assert_eq!((img.width(), img.height()), (240, 160));
}

#[test]
fn alpha_png_is_flattened_on_black() {
    // Left half opaque red, right half fully transparent white.
    let img = rgba(
        200,
        100,
        |x, _| {
            if x < 100 { RED } else { [255, 255, 255, 0] }
        },
    );
    let (out, _) = run(&png(&img), "/a.png", 64);
    assert_eq!((out.width(), out.height()), (64, 32));
    assert!(near(px(&out, 8, 16), RED, 30));
    let clear = px(&out, 56, 16);
    assert!(near(clear, [0, 0, 0, 255], 12), "{clear:?}");
}

#[test]
fn small_images_are_not_upscaled() {
    let (out, _) = run(&png(&quadrants(50, 40)), "/a.png", 256);
    assert_eq!((out.width(), out.height()), (50, 40));
}

#[test]
fn animated_gif_gives_first_frame() {
    let frame = |c: [u8; 4]| {
        let img = image::RgbaImage::from_pixel(80, 60, image::Rgba(c));
        Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100, 1))
    };
    let mut gif = Vec::new();
    GifEncoder::new(&mut gif)
        .encode_frames([frame(RED), frame(BLUE)])
        .unwrap();
    let (out, _) = run(&gif, "/a.gif", 256);
    let mid = px(&out, 40, 30);
    assert!(near(mid, RED, 30), "{mid:?}");
}

#[test]
fn errors_are_typed() {
    let fs = CountFs::with("/x.jpg", b"not an image at all, just text.");
    let e = make(&fs, Path::new("/x.jpg"), 256).unwrap_err();
    assert_eq!(e.code, ErrorCode::Corrupt);
    let e = make(&fs, Path::new("/missing.jpg"), 256).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

/// Prints timing and bytes read for 24 MP files; run by hand with
/// `cargo test --release -p photosee-core --test thumb_make -- --ignored --nocapture`.
#[test]
#[ignore = "timing report, run by hand"]
fn report_24mp() {
    let (w, h) = (6000u32, 4000u32);
    // Smooth gradients plus a hash texture, so the JPEG has a realistic size.
    let img: Rgba8 = rgba(w, h, |x, y| {
        let n = (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) >> 28;
        [
            ((x / 24) as u8).wrapping_add(n as u8),
            ((y / 16) as u8).wrapping_add(n as u8),
            ((x + y) / 40) as u8,
            255,
        ]
    });
    let plain = jpeg(&img, 90);
    let thumb = jpeg(&rgba(160, 107, |x, y| [x as u8, y as u8, 0, 255]), 85);
    let with = jpeg_with_exif(
        &plain,
        ExifSpec {
            ifd0: vec![short(0x0112, 6)],
            exif: vec![long(0xA002, w), long(0xA003, h)],
            thumb: Some(thumb),
            thumb_range: None,
        },
    );
    for (label, bytes) in [("with IFD1", &with), ("without IFD1", &plain)] {
        let fs = CountFs::with("/p.jpg", bytes);
        let t = std::time::Instant::now();
        make(&fs, Path::new("/p.jpg"), 256).unwrap();
        println!(
            "24MP {label}: {} ms, read {} of {} bytes",
            t.elapsed().as_millis(),
            fs.bytes_read(),
            bytes.len()
        );
    }
}
