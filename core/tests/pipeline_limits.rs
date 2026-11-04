//! Hostile input: giant headers, truncation, garbage. Errors, never panics
//! or huge allocations.

#[path = "pipeline_util.rs"]
mod util;

use photosee_core::decode::{decode, info};
use photosee_core::pipeline::render;
use photosee_core::testfs::MemFs;
use photosee_core::{ErrorCode, Format, Size};
use std::path::Path;
use util::*;

#[test]
fn giant_tiff_header_is_too_large_without_allocating() {
    let entries = tiff_entries(100_000, 100_000, vec![8], 1, 2);
    let src = Counting::new(tiff(&[0, 0], &entries));
    let e = info(&src, Format::Tiff).unwrap_err();
    assert_eq!(e.code, ErrorCode::TooLarge, "{e}");
    let e = decode(&src, Format::Tiff, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::TooLarge, "{e}");
    // Header-only work: the reader never fetched more than a few buffers.
    assert!(src.bytes_read() < 1 << 20, "read {}", src.bytes_read());
}

#[test]
fn pixels_over_budget_but_sides_small_is_too_large() {
    // 20000 x 20000 = 400 MP: each side is fine, the product is not.
    let entries = tiff_entries(20_000, 20_000, vec![8, 8, 8], 2, 6);
    let e = decode(&tiff(&[0; 6], &entries), Format::Tiff, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::TooLarge, "{e}");
}

#[test]
fn giant_jpeg_sof_is_too_large() {
    // A real JPEG whose SOF claims 65000 x 65000.
    let mut f = jpeg(&quadrants(16, 16), 90);
    let sof = f.windows(2).position(|w| w == [0xFF, 0xC0]).expect("SOF0");
    f[sof + 5..sof + 9].copy_from_slice(&[0xFD, 0xE8, 0xFD, 0xE8]);
    let e = info(&f, Format::Jpeg).unwrap_err();
    assert_eq!(e.code, ErrorCode::TooLarge, "{e}");
    let e = decode(&f, Format::Jpeg, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::TooLarge, "{e}");
}

#[test]
fn truncated_jpeg_is_corrupt() {
    let noise = rgba(256, 256, |x, y| {
        let v = (x * 7919 + y * 104_729) as u8;
        [v, v.wrapping_mul(3), v ^ 0x5A, 255]
    });
    let full = jpeg(&noise, 90);
    for cut in [full.len() * 6 / 10, 40, 3] {
        let e = decode(&full[..cut].to_vec(), Format::Jpeg, None).unwrap_err();
        assert_eq!(e.code, ErrorCode::Corrupt, "cut {cut}: {e}");
    }
    // info only needs the headers: it fails only when they are cut.
    for cut in [20, 3, 0] {
        let e = info(&full[..cut].to_vec(), Format::Jpeg).unwrap_err();
        assert_eq!(e.code, ErrorCode::Corrupt, "info cut {cut}: {e}");
    }
}

#[test]
fn jpeg_with_trailer_after_eoi_still_decodes() {
    // Motion photos append a video after EOI; EXIF thumbnails carry their
    // own EOI before the main scan. Neither may confuse truncation checks.
    let full = jpeg(&quadrants(64, 64), 90);
    let mut trailer = full.clone();
    trailer.extend_from_slice(b"ftypmp42 pretend video bytes");
    assert_eq!(
        decode(&trailer, Format::Jpeg, None).unwrap().size,
        Size::new(64, 64)
    );
    let mut padded = full.clone();
    padded.extend_from_slice(&[0; 100]);
    assert!(decode(&padded, Format::Jpeg, None).is_ok());
    let thumb = jpeg(&quadrants(8, 8), 80);
    let mut app1 = vec![0xFF, 0xE1];
    app1.extend_from_slice(&((thumb.len() + 2) as u16).to_be_bytes());
    app1.extend_from_slice(&thumb);
    let cut = full.len() * 6 / 10;
    let truncated: Vec<u8> = [&full[..2], &app1[..], &full[2..cut]].concat();
    let e = decode(&truncated, Format::Jpeg, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::Corrupt, "{e}");
}

#[test]
fn truncated_png_and_tiff_are_corrupt() {
    let img = quadrants(64, 64);
    let p = png(&img, None);
    let e = decode(&p[..p.len() / 2].to_vec(), Format::Png, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::Corrupt, "{e}");
    let e = info(&p[..20].to_vec(), Format::Png).unwrap_err();
    assert_eq!(e.code, ErrorCode::Corrupt, "{e}");
    // The strip claims 6000 bytes; the file holds ~130.
    let t = tiff(&[1, 2], &tiff_entries(40, 50, vec![8, 8, 8], 2, 6000));
    let e = decode(&t, Format::Tiff, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::Corrupt, "{e}");
}

#[test]
fn garbage_named_as_image_is_corrupt() {
    let fs = MemFs::new();
    fs.file("/p/x.jpg", b"this is not an image at all, just text");
    fs.file("/p/empty.png", b"");
    fs.file("/p/x.dat", b"\x01\x02\x03");
    let r = |p: &str| render(&fs, Path::new(p), Size::new(100, 100), &[]);
    assert_eq!(
        r("/p/x.jpg").err().map(|e| e.code),
        Some(ErrorCode::Corrupt)
    );
    assert_eq!(
        r("/p/empty.png").err().map(|e| e.code),
        Some(ErrorCode::Corrupt)
    );
    assert_eq!(
        r("/p/x.dat").err().map(|e| e.code),
        Some(ErrorCode::Unsupported)
    );
    assert_eq!(
        r("/p/missing.jpg").err().map(|e| e.code),
        Some(ErrorCode::NotFound)
    );
}

#[test]
fn garbage_after_a_valid_signature_never_panics() {
    // Deterministic pseudo-random bodies behind each signature.
    let heads: [&[u8]; 6] = [
        &[0xFF, 0xD8, 0xFF],
        b"\x89PNG\r\n\x1a\n",
        b"GIF89a",
        b"RIFF\x40\0\0\0WEBPVP8X",
        b"II*\0\x08\0\0\0",
        b"BM",
    ];
    let fmts = [
        Format::Jpeg,
        Format::Png,
        Format::Gif,
        Format::Webp,
        Format::Tiff,
        Format::Bmp,
    ];
    let mut seed = 0x1234_5678u32;
    for (head, fmt) in heads.iter().zip(fmts) {
        for _ in 0..50 {
            let mut f = head.to_vec();
            for _ in 0..200 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                f.push((seed >> 24) as u8);
            }
            let _ = info(&f, fmt);
            let _ = decode(&f, fmt, None);
        }
    }
}
