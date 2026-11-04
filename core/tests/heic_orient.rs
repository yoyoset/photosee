//! Does WIC apply HEIF `irot`/`imir` itself? The brief left it open; this
//! answers it by patching the transforms into an encoded file (the way an
//! iPhone records rotation) and checking the size and where the red marker
//! (top-left in the stored image) ends up.
//!
//! Measured on HEIF ext 1.2.62.0 + HEVC ext 2.4.149.0: yes, both are applied
//! to `GetSize` and to the pixels, so `info` reports `Normal` and the
//! reported size is already the display size.
#![cfg(windows)]

#[path = "heic_enc.rs"]
mod heic_enc;

use photosee_core::decode::heic_wic::{decode, info};
use photosee_core::types::{Orientation, Rgba8, Size};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Which corner holds the red marker; panics unless exactly one does.
fn red_corner(img: &Rgba8) -> Corner {
    let (w, h) = (img.size.w, img.size.h);
    let red = |x: u32, y: u32| {
        let o = ((y * w + x) * 4) as usize;
        img.data[o] > 200 && img.data[o + 1] < 60 && img.data[o + 2] < 60
    };
    let corners = [
        (Corner::TopLeft, red(4, 4)),
        (Corner::TopRight, red(w - 5, 4)),
        (Corner::BottomLeft, red(4, h - 5)),
        (Corner::BottomRight, red(w - 5, h - 5)),
    ];
    let hits: Vec<Corner> = corners.iter().filter(|c| c.1).map(|c| c.0).collect();
    assert_eq!(hits.len(), 1, "red corners: {hits:?}");
    hits[0]
}

fn check(props: &[(&[u8; 4], u8)], size: Size, corner: Corner) {
    let Ok(plain) = heic_enc::encode_heic(64, 48, &heic_enc::quadrant_bgra(64, 48)) else {
        return println!("skipped: no WIC HEIF encoder");
    };
    let heic = heic_enc::add_transforms(&plain, props).expect("WIC encoder layout changed");
    let inf = info(&heic).expect("info");
    assert_eq!(inf.orientation, Orientation::Normal, "{props:?}");
    assert_eq!(inf.size, size, "{props:?}: info size");
    let img = decode(&heic, None).expect("decode");
    assert_eq!(img.size, size, "{props:?}: decode size");
    assert_eq!(red_corner(&img), corner, "{props:?}");
}

#[test]
fn no_transform_keeps_the_marker_top_left() {
    check(&[], Size::new(64, 48), Corner::TopLeft);
}

#[test]
fn irot_is_applied_anticlockwise() {
    // irot angle is in units of 90 degrees anti-clockwise.
    check(&[(b"irot", 1)], Size::new(48, 64), Corner::BottomLeft);
    check(&[(b"irot", 2)], Size::new(64, 48), Corner::BottomRight);
    check(&[(b"irot", 3)], Size::new(48, 64), Corner::TopRight);
}

#[test]
fn imir_is_applied() {
    // Pitfall: WIC treats axis 0 as a top-bottom flip and 1 as left-right.
    check(&[(b"imir", 0)], Size::new(64, 48), Corner::BottomLeft);
    check(&[(b"imir", 1)], Size::new(64, 48), Corner::TopRight);
}

#[test]
fn transforms_apply_in_ipma_order() {
    // Rotate (marker to bottom-left of 48x64), then flip top-bottom.
    check(
        &[(b"irot", 1), (b"imir", 0)],
        Size::new(48, 64),
        Corner::TopLeft,
    );
}
