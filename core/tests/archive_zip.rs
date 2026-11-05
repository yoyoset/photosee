//! ZIP reader against hostile input: every failure is an `Error`, never a
//! panic or an unbounded allocation (AGENTS.md rule 4).
#![allow(clippy::unwrap_used, reason = "test helpers")]

#[path = "archive_util.rs"]
mod util;

use photosee_core::ErrorCode;
use photosee_core::archive::zip::{self, Member};
use util::*;

fn eocd_at(z: &[u8]) -> usize {
    z.windows(4)
        .rposition(|w| w == [0x50, 0x4b, 0x05, 0x06])
        .unwrap()
}

#[test]
fn reads_members_past_a_trailing_comment() {
    let z = zip(&[stored("a.png", b"aaaa"), deflated("b.png", &[7u8; 5000])]);
    let ms = zip::members(&z).unwrap();
    assert_eq!(ms.len(), 2);
    assert_eq!(ms[1].name, "b.png");
    assert_eq!(zip::read_member(&z, &ms[0]).unwrap(), b"aaaa");
    assert_eq!(zip::read_member(&z, &ms[1]).unwrap(), vec![7u8; 5000]);
}

#[test]
fn every_truncation_fails_cleanly() {
    let z = zip(&[stored("a.png", b"aaaa"), deflated("b.png", b"bbbbbbbb")]);
    for cut in 0..z.len() {
        let short = &z[..cut];
        if let Ok(ms) = zip::members(&short.to_vec()) {
            for m in &ms {
                let _ = zip::read_member(&short.to_vec(), m);
            }
        }
    }
}

#[test]
fn every_single_byte_corruption_fails_cleanly() {
    let z = zip(&[stored("a.png", b"aaaa"), deflated("b.png", &[1u8; 300])]);
    for i in 0..z.len() {
        for v in [0x00, 0xFF, 0x7F] {
            let mut bad = z.clone();
            bad[i] = v;
            if let Ok(ms) = zip::members(&bad) {
                for m in &ms {
                    let _ = zip::read_member(&bad, m);
                }
            }
        }
    }
}

#[test]
fn central_directory_out_of_bounds_is_corrupt() {
    let mut z = zip(&[stored("a.png", b"aaaa")]);
    let e = eocd_at(&z);
    z[e + 16..e + 20].copy_from_slice(&0x7FFF_0000u32.to_le_bytes());
    assert_eq!(zip::members(&z).unwrap_err().code, ErrorCode::Corrupt);
}

#[test]
fn zip64_marker_is_unsupported() {
    let mut z = zip(&[stored("a.png", b"aaaa")]);
    let e = eocd_at(&z);
    z[e + 16..e + 20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(zip::members(&z).unwrap_err().code, ErrorCode::Unsupported);
}

#[test]
fn bomb_sized_member_is_refused_before_allocating() {
    let z = zip(&[deflated("a.png", b"aaaa")]);
    let mut m: Member = zip::members(&z).unwrap().remove(0);
    m.size = zip::MAX_MEMBER_BYTES + 1;
    assert_eq!(
        zip::read_member(&z, &m).unwrap_err().code,
        ErrorCode::TooLarge
    );
}

#[test]
fn inflating_more_than_declared_is_corrupt() {
    let z = zip(&[deflated("a.png", &[3u8; 4096])]);
    let mut m = zip::members(&z).unwrap().remove(0);
    m.size = 100;
    assert_eq!(
        zip::read_member(&z, &m).unwrap_err().code,
        ErrorCode::Corrupt
    );
}

#[test]
fn unknown_method_and_encryption_are_unsupported() {
    let z = zip(&[stored("a.png", b"aaaa")]);
    let mut m = zip::members(&z).unwrap().remove(0);
    m.method = 12; // bzip2
    assert_eq!(
        zip::read_member(&z, &m).unwrap_err().code,
        ErrorCode::Unsupported
    );
    m.method = 0;
    m.encrypted = true;
    assert_eq!(
        zip::read_member(&z, &m).unwrap_err().code,
        ErrorCode::Unsupported
    );
}

#[test]
fn not_a_zip_is_corrupt() {
    assert_eq!(
        zip::members(&b"hello".to_vec()).unwrap_err().code,
        ErrorCode::Corrupt
    );
    assert_eq!(
        zip::members(&vec![0u8; 100_000]).unwrap_err().code,
        ErrorCode::Corrupt
    );
}
