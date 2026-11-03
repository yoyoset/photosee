//! Truncated, garbage and adversarial input: always `Err`, never a panic,
//! never a read request larger than the file.

use super::build::{Counting, LONG, SHORT, TiffB, bx, cat, noise};
use super::build_kinds::{ALL, sample};
use super::{info, locate_preview};
use crate::error::ErrorCode;
use crate::types::RawKind;

/// Runs both entry points; returns whether either succeeded. The caller
/// decides whether success is acceptable.
fn probe(data: Vec<u8>, kind: RawKind) -> bool {
    let len = data.len();
    let src = Counting::new(data);
    let ok = locate_preview(&src, kind).is_ok() | info(&src, kind).is_ok();
    assert!(
        src.largest() <= len.max(1),
        "{kind:?}: request {} > file {len}",
        src.largest()
    );
    ok
}

#[test]
fn every_truncation_is_an_error() {
    for kind in ALL {
        let s = sample(kind);
        let jpeg_end = (s.off + s.len) as usize;
        for cut in 0..jpeg_end {
            let ok = probe(s.data[..cut].to_vec(), kind);
            assert!(
                !ok,
                "{kind:?} truncated to {cut} of {jpeg_end} still succeeded"
            );
        }
    }
}

#[test]
fn garbage_never_panics() {
    for kind in ALL {
        assert!(!probe(Vec::new(), kind));
        for seed in 1..40u64 {
            assert!(
                !probe(noise(seed, 64 + seed as usize * 97), kind),
                "{kind:?} seed {seed}"
            );
        }
    }
}

#[test]
fn flipped_bytes_never_panic() {
    // Corrupt one byte at a time in the structural part of every sample.
    for kind in ALL {
        let s = sample(kind);
        let header_end = s.data.len().min(4096);
        for i in (0..header_end).step_by(3) {
            for v in [0x00, 0xFF, 0x80] {
                let mut d = s.data.clone();
                d[i] = v;
                let _ = probe(d, kind);
            }
        }
    }
}

#[test]
fn ifd_loop_to_itself_terminates() {
    let mut t = TiffB::new(true, 42);
    let ifd0 = t.ifd(&[(0x0112, SHORT, 1, 1)], 0);
    t.patch32(TiffB::next_at(ifd0, 1), ifd0);
    // A SubIFD that points back to IFD0 as well.
    let arr = t.u32s(&[ifd0]);
    let sub = t.ifd(&[(0x014A, LONG, 1, arr), (0x00FE, LONG, 1, 1)], ifd0);
    t.patch32(TiffB::value_at(sub, 0), ifd0);
    t.set_ifd0(sub);
    for kind in [RawKind::Nef, RawKind::Dng] {
        let err = locate_preview(&t.buf, kind).unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported, "{kind:?}: {err}");
    }
}

#[test]
fn long_ifd_chain_is_capped() {
    let mut t = TiffB::new(true, 42);
    let mut next = 0;
    for _ in 0..200 {
        next = t.ifd(&[(0x0112, SHORT, 1, 1)], next);
    }
    t.set_ifd0(next);
    let err = locate_preview(&t.buf, RawKind::Nef).unwrap_err();
    assert_eq!(err.code, ErrorCode::Corrupt, "{err}");
}

#[test]
fn huge_counts_and_lengths_do_not_allocate() {
    let mut t = TiffB::new(true, 42);
    let arr = t.u32s(&[8]);
    let e = [
        (0x014A, LONG, u32::MAX, arr),
        (0x0201, LONG, 1, 8),
        (0x0202, LONG, 1, u32::MAX),
    ];
    let ifd0 = t.ifd(&e, 0);
    t.set_ifd0(ifd0);
    for kind in [
        RawKind::Arw,
        RawKind::Nef,
        RawKind::Dng,
        RawKind::Cr2,
        RawKind::Orf,
    ] {
        assert!(!probe(t.buf.clone(), kind), "{kind:?}");
    }
    // An IFD claiming 65535 entries in a tiny file.
    let mut d = b"II*\0\x08\0\0\0".to_vec();
    d.extend_from_slice(&[0xFF, 0xFF, 0, 0]);
    assert!(!probe(d, RawKind::Arw));
}

#[test]
fn hostile_boxes_are_rejected() {
    let ftyp = bx(b"ftyp", b"crx \0\0\0\x01");
    let huge = cat(&[&ftyp, &u32::MAX.to_be_bytes(), b"moov"]);
    let large = cat(&[&ftyp, &1u32.to_be_bytes(), b"moov", &u64::MAX.to_be_bytes()]);
    let tiny = cat(&[&ftyp, &4u32.to_be_bytes(), b"moov"]);
    let empty_moov = cat(&[&ftyp, &bx(b"moov", &[])]);
    let nested = cat(&[&ftyp, &bx(b"moov", &bx(b"trak", &u32::MAX.to_be_bytes()))]);
    for d in [huge, large, tiny, empty_moov, nested] {
        assert!(!probe(d, RawKind::Cr3));
    }
}

#[test]
fn many_small_boxes_are_capped() {
    let ftyp = bx(b"ftyp", b"crx \0\0\0\x01");
    let frees: Vec<u8> = (0..1000).flat_map(|_| bx(b"free", &[])).collect();
    assert!(!probe(cat(&[&ftyp, &frees]), RawKind::Cr3));
}
