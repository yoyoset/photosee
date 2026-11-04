//! Timing on a 12 MP (4032x3024, iPhone main camera) HEIC made at test
//! time. Prints numbers for the report; asserts only correctness, since
//! timings depend on the machine. Own binary so parallel tests don't skew it.
#![cfg(windows)]

#[path = "heic_enc.rs"]
mod heic_enc;

use photosee_core::decode::heic_wic::{decode, info};
use photosee_core::types::Size;
use std::time::Instant;

/// A gradient with per-pixel noise: flat colour would make HEVC decode
/// unrealistically cheap.
fn photo_like(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    let mut s = 0x1234_5678u32;
    for y in 0..h {
        for x in 0..w {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let n = (s & 15) as u8;
            let b = ((x + y) / 28) as u8;
            px.extend_from_slice(&[(x / 16) as u8 ^ n, (y / 12) as u8 ^ n, b, 255]);
        }
    }
    px
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

#[test]
fn twelve_megapixel_timings() {
    let (w, h) = (4032, 3024);
    let Ok(heic) = heic_enc::encode_heic(w, h, &photo_like(w, h)) else {
        return println!("skipped: no WIC HEIF encoder");
    };
    println!("12 MP HEIC: {} bytes", heic.len());
    let t = Instant::now();
    assert_eq!(info(&heic).expect("info").size, Size::new(w, h));
    println!("first info (loads the extension): {:.1} ms", ms(t));
    for round in 0..3 {
        let t = Instant::now();
        info(&heic).expect("info");
        let ti = ms(t);
        let t = Instant::now();
        let full = decode(&heic, None).expect("decode");
        let tf = ms(t);
        assert_eq!(full.size, Size::new(w, h));
        let t = Instant::now();
        let fit = decode(&heic, Some(Size::new(1920, 1080))).expect("decode");
        let ts = ms(t);
        println!(
            "round {round}: info {ti:.1} ms, full decode {tf:.1} ms, max 1920x1080 -> {:?} {ts:.1} ms",
            fit.size
        );
    }
}
