//! A JPEG with a stray byte between marker segments, as found in a real
//! comic archive (Viewfinder T07 p.21, 2026-11-05): `FF FE 00 03 00` (COM,
//! one payload byte) then `0A`, then `FF DB`. libjpeg/PIL decode it with a
//! warning; the header walker used to call it corrupt.
#![allow(clippy::unwrap_used, reason = "test helpers")]

#[path = "pipeline_util.rs"]
mod util;

use photosee_core::decode::{decode, info};
use photosee_core::{ErrorCode, Format, Size};
use util::*;

/// Inserts `COM(1 byte) + stray` right after the APP0 segment.
fn with_stray(stray: &[u8]) -> Vec<u8> {
    let j = jpeg(&quadrants(32, 16), 90);
    // SOI (2) + APP0: FF E0 + big-endian length.
    let app0_end = 4 + usize::from(u16::from_be_bytes([j[4], j[5]]));
    let mut out = j[..app0_end].to_vec();
    out.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x03, 0x00]);
    out.extend_from_slice(stray);
    out.extend_from_slice(&j[app0_end..]);
    out
}

#[test]
fn stray_byte_before_a_marker_is_skipped() {
    let j = with_stray(&[0x0A]);
    let i = info(&j, Format::Jpeg).unwrap();
    assert_eq!(i.size, Size::new(32, 16));
    assert_eq!(
        decode(&j, Format::Jpeg, None).unwrap().size,
        Size::new(32, 16)
    );
}

#[test]
fn long_garbage_run_is_still_corrupt() {
    let j = with_stray(&[0x0A; 5000]);
    assert_eq!(info(&j, Format::Jpeg).unwrap_err().code, ErrorCode::Corrupt);
}
