//! Failure paths: the missing-codec table (pure, so no Store package has to
//! be uninstalled), the HEIF sniff, and hostile bytes that must come back as
//! `Err`, never a panic and never a "missing codec" for a broken file.
#![cfg(windows)]

#[path = "heic_enc.rs"]
mod heic_enc;

use photosee_core::decode::heic_wic::{Stage, classify, decode, info, is_heif};
use photosee_core::error::{Codec, ErrorCode};

const COMPONENT_NOT_FOUND: i32 = 0x8898_2F50_u32 as i32;
const TOPO_CODEC_NOT_FOUND: i32 = 0xC00D_5212_u32 as i32;
const WIN32_NOT_FOUND: i32 = 0x8007_0490_u32 as i32;
/// MF_E_INVALID_FILE_FORMAT, observed for broken HEIF metadata.
const INVALID_FILE_FORMAT: i32 = 0xC00D_36BE_u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;

#[test]
fn classify_table() {
    let heif = ErrorCode::MissingCodec(Codec::HeifExt);
    let hevc = ErrorCode::MissingCodec(Codec::HevcExt);
    let cases = [
        (Stage::Open, COMPONENT_NOT_FOUND, heif),
        (Stage::Open, TOPO_CODEC_NOT_FOUND, ErrorCode::Corrupt),
        (Stage::Open, INVALID_FILE_FORMAT, ErrorCode::Corrupt),
        (Stage::Decode, COMPONENT_NOT_FOUND, hevc),
        (Stage::Decode, TOPO_CODEC_NOT_FOUND, hevc),
        (Stage::Decode, WIN32_NOT_FOUND, hevc),
        (Stage::Decode, INVALID_FILE_FORMAT, ErrorCode::Corrupt),
        (Stage::Decode, E_INVALIDARG, ErrorCode::Corrupt),
        (Stage::Setup, COMPONENT_NOT_FOUND, ErrorCode::Unsupported),
        (Stage::Setup, E_INVALIDARG, ErrorCode::Unsupported),
    ];
    for (stage, hr, want) in cases {
        assert_eq!(classify(stage, hr), want, "{stage:?} 0x{:08X}", hr as u32);
    }
}

fn ftyp(brands: &[&[u8; 4]]) -> Vec<u8> {
    let mut b = (8 + 4 * brands.len() as u32 + 4).to_be_bytes().to_vec();
    b.extend_from_slice(b"ftyp");
    b.extend_from_slice(brands[0]);
    b.extend_from_slice(&[0, 0, 0, 0]); // minor version, never a brand
    for br in &brands[1..] {
        b.extend_from_slice(*br);
    }
    b
}

#[test]
fn is_heif_needs_a_heif_brand() {
    assert!(is_heif(&ftyp(&[b"heic", b"mif1", b"heic"])));
    assert!(is_heif(&ftyp(&[b"isom", b"mif1"])));
    assert!(is_heif(&ftyp(&[b"avif", b"mif1", b"miaf"])));
    assert!(!is_heif(&ftyp(&[b"isom", b"isom", b"mp41"])));
    assert!(!is_heif(b""));
    assert!(!is_heif(b"\0\0\0\x08ftyp"));
    assert!(!is_heif(b"\xff\xd8\xff\xe0 JFIF garbage"));
    // A size field pointing past the end must not panic.
    let mut huge = ftyp(&[b"heic"]);
    huge[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(is_heif(&huge));
}

fn assert_broken(bytes: &[u8], what: &str) {
    for res in [
        info(&bytes.to_vec()).map(|_| ()),
        decode(&bytes.to_vec(), None).map(|_| ()),
    ] {
        let err = res.expect_err(what);
        // Broken files must never send the user to the Store.
        assert!(
            !matches!(err.code, ErrorCode::MissingCodec(_)),
            "{what}: {err:?}"
        );
    }
}

#[test]
fn garbage_is_corrupt() {
    assert_broken(b"", "empty");
    assert_broken(b"not an image at all", "text");
    assert_broken(&[0xAB; 4096], "noise");
    let mut mp4 = ftyp(&[b"isom", b"isom", b"mp41"]);
    mp4.extend_from_slice(&[0x5A; 2048]);
    assert_broken(&mp4, "mp4 brand");
    let mut fake = ftyp(&[b"heic", b"mif1"]);
    fake.extend_from_slice(&[0x5A; 2048]);
    assert_broken(&fake, "heic ftyp + garbage");
}

#[test]
fn truncated_heic_is_corrupt_at_every_cut() {
    let Ok(heic) = heic_enc::encode_heic(64, 48, &heic_enc::quadrant_bgra(64, 48)) else {
        return println!("skipped: no WIC HEIF encoder");
    };
    // Every 16th cut covers ftyp, meta, iloc, the ipco properties and mdat.
    for cut in (0..heic.len() - 8).step_by(16) {
        let res = decode(&heic[..cut].to_vec(), None);
        let err = res.expect_err(&format!("cut at {cut} of {}", heic.len()));
        assert_eq!(err.code, ErrorCode::Corrupt, "cut {cut}: {}", err.message);
    }
}
