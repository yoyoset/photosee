//! The PreviewLoc orientation contract (types.rs): report only what is
//! still to apply on top of the embedded JPEG's own EXIF Orientation tag.

use super::build::{Counting, LONG, SHORT, TiffB, jpeg, jpeg_with_orientation};
use super::build_kinds::raf;
use super::jpeg::orientation_tag;
use super::{info, locate_preview};
use crate::types::{Orientation, RawKind};

/// ARW-shaped file: IFD0 Orientation=`raw_o`, preview JPEG `j`.
fn arw_with(raw_o: u16, j: &[u8]) -> Vec<u8> {
    let mut t = TiffB::new(true, 42);
    let b = t.blob(j);
    let e = [
        (0x0112, SHORT, 1, u32::from(raw_o)),
        (0x0201, LONG, 1, b),
        (0x0202, LONG, 1, j.len() as u32),
    ];
    let ifd0 = t.ifd(&e, 0);
    t.set_ifd0(ifd0);
    t.buf
}

#[test]
fn jpeg_with_own_tag_reports_normal() {
    // (a) the JPEG says 3, the RAW says 6: the JPEG's decoder applies 3,
    // so nothing is left to apply.
    let data = arw_with(6, &jpeg_with_orientation(30, 20, 3));
    let loc = locate_preview(&data, RawKind::Arw).expect("locate");
    assert_eq!(loc.orientation, Orientation::Normal);
    let inf = info(&data, RawKind::Arw).expect("info");
    assert_eq!(
        inf.orientation,
        Orientation::Rotate180,
        "effective orientation"
    );
}

#[test]
fn own_tag_with_value_one_still_counts_as_present() {
    let data = arw_with(6, &jpeg_with_orientation(30, 20, 1));
    let loc = locate_preview(&data, RawKind::Arw).expect("locate");
    assert_eq!(loc.orientation, Orientation::Normal);
}

#[test]
fn jpeg_without_tag_inherits_raw_ifd0() {
    // (b)
    let data = arw_with(6, &jpeg(30, 20));
    let loc = locate_preview(&data, RawKind::Arw).expect("locate");
    assert_eq!(loc.orientation, Orientation::Rotate90);
    assert_eq!(
        info(&data, RawKind::Arw).expect("info").orientation,
        Orientation::Rotate90
    );
}

#[test]
fn raf_orientation_lives_in_the_jpeg() {
    // (c) RAF has no RAW-level tag; the JPEG carries 6.
    let s = raf();
    let loc = locate_preview(&s.data, RawKind::Raf).expect("locate");
    assert_eq!(loc.orientation, Orientation::Normal);
    let range = &s.data[loc.offset as usize..(loc.offset + loc.len) as usize];
    assert_eq!(orientation_tag(range), Some(6));
    let inf = info(&s.data, RawKind::Raf).expect("info");
    assert_eq!(inf.orientation, Orientation::Rotate90);
}

/// SOI + the given segments, then the rest of a real JPEG.
fn with_segments(segs: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    for (m, payload) in segs {
        out.extend_from_slice(&[0xFF, *m]);
        out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(payload);
    }
    out.extend_from_slice(&jpeg(8, 8)[2..]);
    out
}

fn exif(le: bool, o: u16) -> Vec<u8> {
    let mut t = TiffB::new(le, 42);
    let ifd0 = t.ifd(&[(0x0100, LONG, 1, 9), (0x0112, SHORT, 1, u32::from(o))], 0);
    t.set_ifd0(ifd0);
    [b"Exif\0\0".to_vec(), t.buf].concat()
}

#[test]
fn tag_found_after_jfif_and_xmp_in_both_byte_orders() {
    let xmp = b"http://ns.adobe.com/xap/1.0/\0<x/>".to_vec();
    for le in [true, false] {
        let j = with_segments(&[
            (0xE0, b"JFIF\0\x01\x01".to_vec()),
            (0xE1, xmp.clone()),
            (0xE1, exif(le, 8)),
        ]);
        assert_eq!(orientation_tag(&j), Some(8), "le={le}");
    }
}

#[test]
fn no_tag_or_broken_exif_means_none() {
    assert_eq!(orientation_tag(&jpeg(8, 8)), None);
    let mut no_tag = exif(true, 1);
    no_tag.truncate(6 + 8 + 2 + 12 + 4); // only the first entry (0x0100)
    no_tag[14] = 1; // entry count 1
    assert_eq!(orientation_tag(&with_segments(&[(0xE1, no_tag)])), None);
    let broken = [b"Exif\0\0".to_vec(), b"XX*\0\x08\0\0\0".to_vec()].concat();
    assert_eq!(orientation_tag(&with_segments(&[(0xE1, broken)])), None);
    let wild = [b"Exif\0\0II*\0".to_vec(), u32::MAX.to_le_bytes().to_vec()].concat();
    assert_eq!(orientation_tag(&with_segments(&[(0xE1, wild)])), None);
    // EXIF after the first table segment is not looked for.
    let late = with_segments(&[(0xDB, vec![0; 65]), (0xE1, exif(true, 6))]);
    assert_eq!(orientation_tag(&late), None);
}

#[test]
fn every_prefix_of_the_head_is_safe() {
    let j = with_segments(&[(0xE1, exif(false, 5))]);
    for cut in 0..j.len() {
        let _ = orientation_tag(&j[..cut]);
    }
}

#[test]
fn tag_detection_reads_at_most_64_kib() {
    // Two 60 KiB APP2 segments push the EXIF past the window: not found,
    // and no single request exceeds the window.
    let pad = vec![0u8; 60 * 1024];
    let j = with_segments(&[(0xE2, pad.clone()), (0xE2, pad), (0xE1, exif(true, 6))]);
    let src = Counting::new(arw_with(6, &j));
    let loc = locate_preview(&src, RawKind::Arw).expect("locate");
    assert_eq!(loc.orientation, Orientation::Rotate90, "tag outside window");
    assert!(
        src.largest() <= 64 * 1024,
        "largest request {}",
        src.largest()
    );
}
