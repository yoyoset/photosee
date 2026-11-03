//! CR3 (BMFF) and RAF specifics.

use super::build::{bx, cat};
use super::build_kinds::{cr3_with, raf};
use super::locate_preview;
use super::tests_tiff::check;
use crate::error::ErrorCode;
use crate::types::{Orientation, RawKind};

#[test]
fn cr3_follows_ccdt_to_preview_big() {
    // trak 1 is a small decoy; CCDT type 2 names trak 2.
    check(RawKind::Cr3, &cr3_with(true));
}

#[test]
fn cr3_without_ccdt_falls_back_to_first_trak() {
    check(RawKind::Cr3, &cr3_with(false));
}

#[test]
fn cr3_needs_crx_brand() {
    let mut s = cr3_with(true).data;
    s[8..12].copy_from_slice(b"heic");
    let err = locate_preview(&s, RawKind::Cr3).unwrap_err();
    assert_eq!(err.code, ErrorCode::Corrupt);
}

#[test]
fn cr3_minimal_tree_without_canon_box() {
    // ftyp + moov{trak} + mdat: no Canon uuid, so Normal orientation.
    let j = super::build::jpeg(16, 16);
    let ftyp = bx(b"ftyp", b"crx \0\0\0\x01");
    let mk = |off: u64| {
        let stsz = super::build::full(
            b"stsz",
            &cat(&[&(j.len() as u32).to_be_bytes(), &1u32.to_be_bytes()]),
        );
        let stco = super::build::full(
            b"stco",
            &cat(&[&1u32.to_be_bytes(), &(off as u32).to_be_bytes()]),
        );
        let stbl = bx(b"stbl", &cat(&[&stsz, &stco]));
        bx(b"moov", &bx(b"trak", &bx(b"mdia", &bx(b"minf", &stbl))))
    };
    let at = (ftyp.len() + mk(0).len() + 8) as u64;
    let data = cat(&[&ftyp, &mk(at), &bx(b"mdat", &j)]);
    let loc = locate_preview(&data, RawKind::Cr3).expect("locate");
    assert_eq!((loc.offset, loc.len), (at, j.len() as u64));
    assert_eq!(loc.orientation, Orientation::Normal);
}

#[test]
fn cr3_largesize_and_size_zero_boxes() {
    // mdat with size 1 + 64-bit largesize, and a trailing size-0 box.
    let j = super::build::jpeg(16, 16);
    let ftyp = bx(b"ftyp", b"crx \0\0\0\x01");
    let mk = |off: u64| {
        let stsz = super::build::full(
            b"stsz",
            &cat(&[&(j.len() as u32).to_be_bytes(), &1u32.to_be_bytes()]),
        );
        let co64 = super::build::full(b"co64", &cat(&[&1u32.to_be_bytes(), &off.to_be_bytes()]));
        bx(
            b"moov",
            &bx(
                b"trak",
                &bx(b"mdia", &bx(b"minf", &bx(b"stbl", &cat(&[&stsz, &co64])))),
            ),
        )
    };
    let at = (ftyp.len() + mk(0).len() + 16) as u64;
    let mut mdat = 1u32.to_be_bytes().to_vec();
    mdat.extend_from_slice(b"mdat");
    mdat.extend_from_slice(&((j.len() + 16) as u64).to_be_bytes());
    mdat.extend_from_slice(&j);
    let tail = cat(&[&0u32.to_be_bytes(), b"free", &[0u8; 8]]);
    let data = cat(&[&ftyp, &mk(at), &mdat, &tail]);
    let loc = locate_preview(&data, RawKind::Cr3).expect("locate");
    assert_eq!(loc.offset, at);
}

#[test]
fn raf_header_locates_the_jpeg() {
    check(RawKind::Raf, &raf());
}

#[test]
fn raf_bad_magic_or_range() {
    let mut s = raf().data;
    s[0] = b'X';
    assert_eq!(
        locate_preview(&s, RawKind::Raf).unwrap_err().code,
        ErrorCode::Corrupt
    );
    let mut s = raf().data;
    s[88..92].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        locate_preview(&s, RawKind::Raf).unwrap_err().code,
        ErrorCode::Corrupt
    );
}
