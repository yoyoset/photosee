//! Synthetic-file tests for every kind, focused on the TIFF-based ones.

use super::build::{Counting, LONG, SHORT, TiffB, jpeg};
use super::build_kinds::{ALL, Sample, sample};
use super::{decode, info, locate_preview};
use crate::error::ErrorCode;
use crate::types::{Format, Orientation, RawKind, Size};

/// Locator, info and the extracted bytes all agree with what was built.
pub(super) fn check(kind: RawKind, s: &Sample) {
    let src = Counting::new(s.data.clone());
    let loc = locate_preview(&src, kind).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
    assert_eq!((loc.offset, loc.len), (s.off, s.len), "{kind:?} range");
    assert_eq!(
        loc.orientation,
        Orientation::from_exif(s.orientation),
        "{kind:?}"
    );
    let inf = info(&s.data, kind).expect("info");
    assert_eq!(inf.size, Size::new(s.w, s.h), "{kind:?} SOF size");
    assert_eq!(inf.format, Format::Raw(kind));
    assert_eq!(inf.orientation, loc.orientation);
    let range = &s.data[loc.offset as usize..(loc.offset + loc.len) as usize];
    let img = image::load_from_memory_with_format(range, image::ImageFormat::Jpeg)
        .expect("extracted range decodes as JPEG");
    assert_eq!((img.width(), img.height()), (s.w, s.h));
}

#[test]
fn every_kind_finds_the_right_preview() {
    for kind in ALL {
        check(kind, &sample(kind));
    }
}

#[test]
fn header_bytes_read_stay_small() {
    for kind in ALL {
        let s = sample(kind);
        let src = Counting::new(s.data.clone());
        info(&src, kind).expect("info");
        // Synthetic files are tiny, so a block read covers them; the point
        // is that IO never exceeds the file and no request is oversized.
        println!("{kind:?}: {} bytes read of {}", src.bytes(), s.data.len());
        assert!(src.bytes() <= 3 * s.data.len() as u64);
        assert!(src.largest() <= s.data.len());
    }
}

/// The same IFD layout in both byte orders.
#[test]
fn tiff_little_and_big_endian() {
    for le in [true, false] {
        let j = jpeg(30, 20);
        let mut t = TiffB::new(le, 42);
        let b = t.blob(&j);
        let e = [
            (0x0112, SHORT, 1, 5),
            (0x0201, LONG, 1, b),
            (0x0202, LONG, 1, j.len() as u32),
        ];
        let ifd0 = t.ifd(&e, 0);
        t.set_ifd0(ifd0);
        let len = j.len() as u64;
        let s = Sample {
            data: t.buf,
            off: u64::from(b),
            len,
            w: 30,
            h: 20,
            orientation: 5,
        };
        check(RawKind::Arw, &s);
    }
}

/// Real layout distances: IFD0 at the front, a large JPEG 4 MiB later.
/// Only the block at the file start and the one at the JPEG head are read.
#[test]
fn far_apart_structures_read_only_their_blocks() {
    let j = jpeg(1200, 800);
    let mut t = TiffB::new(true, 42);
    let e = [(0x0201, LONG, 1, 0), (0x0202, LONG, 1, j.len() as u32)];
    let ifd0 = t.ifd(&e, 0);
    t.set_ifd0(ifd0);
    t.blob(&vec![0u8; 4 << 20]);
    let at = t.blob(&j);
    t.patch32(TiffB::value_at(ifd0, 0), at);
    let src = Counting::new(t.buf);
    let inf = info(&src, RawKind::Arw).expect("info");
    assert_eq!(inf.size, Size::new(1200, 800));
    println!("4 MiB ARW-like file: {} header bytes read", src.bytes());
    assert!(src.bytes() <= 2 * 16 * 1024, "{} bytes", src.bytes());
}

#[test]
fn arw_reports_the_real_preview_size() {
    // ARW IFD0 previews are often 1616 wide; we report the SOF, not a guess.
    let inf = info(&sample(RawKind::Arw).data, RawKind::Arw).expect("info");
    assert_eq!(inf.size, Size::new(40, 24));
}

#[test]
fn dng_without_jpeg_preview_is_unsupported() {
    let mut t = TiffB::new(true, 42);
    let data = t.blob(&[0u8; 64]);
    // NewSubFileType=1 but Compression=1 (uncompressed); JXL is the same case.
    let e = [
        (0x00FE, LONG, 1, 1),
        (0x0103, SHORT, 1, 1),
        (0x0111, LONG, 1, data),
        (0x0117, LONG, 1, 64),
    ];
    let sub = t.ifd(&e, 0);
    let ifd0 = t.ifd(&[(0x014A, LONG, 1, sub)], 0);
    t.set_ifd0(ifd0);
    let err = locate_preview(&t.buf, RawKind::Dng).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported, "{err}");
}

#[test]
fn cr2_with_non_jpeg_ifd0_is_unsupported() {
    let mut t = TiffB::new(true, 42);
    let ifd0 = t.ifd(&[(0x0103, SHORT, 1, 1)], 0);
    t.set_ifd0(ifd0);
    let err = locate_preview(&t.buf, RawKind::Cr2).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
}

#[test]
fn nef_without_reduced_ifd_is_unsupported() {
    let mut t = TiffB::new(false, 42);
    let ifd0 = t.ifd(&[(0x0112, SHORT, 1, 1)], 0);
    t.set_ifd0(ifd0);
    let err = locate_preview(&t.buf, RawKind::Nef).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
}

#[test]
fn preview_pointing_at_non_jpeg_is_corrupt() {
    let mut t = TiffB::new(true, 42);
    let b = t.blob(&[0x42u8; 32]);
    let ifd0 = t.ifd(&[(0x0201, LONG, 1, b), (0x0202, LONG, 1, 32)], 0);
    t.set_ifd0(ifd0);
    let err = locate_preview(&t.buf, RawKind::Arw).unwrap_err();
    assert_eq!(err.code, ErrorCode::Corrupt);
}

#[test]
#[ignore = "needs package C (std_formats::decode, orient::apply)"]
fn decode_returns_oriented_pixels() {
    let s = sample(RawKind::Nef); // 72x40, Orientation=6 (Rotate90)
    let img = decode(&s.data, RawKind::Nef, None).expect("decode");
    assert_eq!(img.size, Size::new(40, 72));
}
