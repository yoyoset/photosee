//! PSD merged-image decoder tests. Files are built in the test; see the module
//! doc of `decode/psd.rs` for why zune-psd is not used.

use photosee_core::decode::psd;
use photosee_core::{Orientation, Size};

struct Spec {
    ch: u16,
    w: u32,
    h: u32,
    depth: u16,
    mode: u16,
    comp: u16,
    /// Layer-info count to store; `None` = empty layer-and-mask section.
    layers: Option<i16>,
}

impl Spec {
    fn rgb8(w: u32, h: u32) -> Self {
        Spec {
            ch: 3,
            w,
            h,
            depth: 8,
            mode: 3,
            comp: 0,
            layers: None,
        }
    }
    fn build(&self, body: &[u8]) -> Vec<u8> {
        let mut v = b"8BPS".to_vec();
        v.extend(1u16.to_be_bytes());
        v.extend([0u8; 6]);
        v.extend(self.ch.to_be_bytes());
        v.extend(self.h.to_be_bytes());
        v.extend(self.w.to_be_bytes());
        v.extend(self.depth.to_be_bytes());
        v.extend(self.mode.to_be_bytes());
        v.extend([0u8; 8]); // empty color-mode data and image resources
        match self.layers {
            None => v.extend(0u32.to_be_bytes()),
            Some(n) => {
                v.extend(6u32.to_be_bytes());
                v.extend(2u32.to_be_bytes()); // layer info length
                v.extend(n.to_be_bytes());
            }
        }
        v.extend(self.comp.to_be_bytes());
        v.extend(body);
        v
    }
}

/// PackBits for one row: runs of 3+ equal bytes, literals otherwise.
fn pack(row: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < row.len() {
        let run = row[i..]
            .iter()
            .take(128)
            .take_while(|&&b| b == row[i])
            .count();
        if run >= 3 {
            out.extend([(257 - run) as u8, row[i]]);
            i += run;
        } else {
            out.extend([0, row[i]]);
            i += 1;
        }
    }
    out
}

/// RLE body: row-length table, then every row of every channel.
fn rle_body(planes: &[Vec<u8>], row_bytes: usize) -> Vec<u8> {
    let rows: Vec<Vec<u8>> = planes
        .iter()
        .flat_map(|p| p.chunks(row_bytes).map(pack))
        .collect();
    let mut body: Vec<u8> = rows
        .iter()
        .flat_map(|r| (r.len() as u16).to_be_bytes())
        .collect();
    body.extend(rows.concat());
    body
}

fn px(img: &photosee_core::Rgba8, i: usize) -> [u8; 4] {
    img.data[i * 4..i * 4 + 4].try_into().expect("4 bytes")
}

#[test]
fn rgb8_raw_is_interleaved_with_opaque_alpha() {
    let body = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
    let file = Spec::rgb8(2, 2).build(&body);
    let info = psd::info(&file).unwrap();
    assert_eq!(
        (info.size, info.orientation, info.animated),
        (Size::new(2, 2), Orientation::Normal, false)
    );
    let img = psd::decode(&file, None).unwrap();
    assert_eq!(img.size, Size::new(2, 2));
    assert_eq!(px(&img, 0), [10, 50, 90, 255]);
    assert_eq!(px(&img, 3), [40, 80, 120, 255]);
}

#[test]
fn rgb8_rle_with_runs_and_literals() {
    // 5 wide, 2 rows: R has a run, G is a literal ramp, B alternates.
    let r = vec![7, 7, 7, 7, 7, 1, 2, 3, 3, 3];
    let g = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let b = vec![0, 255, 0, 255, 0, 255, 255, 255, 255, 255];
    let spec = Spec {
        comp: 1,
        ..Spec::rgb8(5, 2)
    };
    let file = spec.build(&rle_body(&[r.clone(), g.clone(), b.clone()], 5));
    let img = psd::decode(&file, None).unwrap();
    for i in 0..10 {
        assert_eq!(px(&img, i), [r[i], g[i], b[i], 255], "pixel {i}");
    }
}

#[test]
fn gray16_raw_reduces_to_8_bit_gray() {
    let samples: [u16; 4] = [0x0000, 0x8000, 0xffff, 0x0100];
    let body: Vec<u8> = samples.iter().flat_map(|s| s.to_be_bytes()).collect();
    let spec = Spec {
        ch: 1,
        depth: 16,
        mode: 1,
        ..Spec::rgb8(2, 2)
    };
    let img = psd::decode(&spec.build(&body), None).unwrap();
    let grays: Vec<u8> = (0..4).map(|i| px(&img, i)[0]).collect();
    assert_eq!(grays, [0, 128, 255, 1]);
    assert_eq!(px(&img, 1), [128, 128, 128, 255]);
}

#[test]
fn rgb16_rle_is_decoded() {
    // zune-psd answers "Not implemented" for this combination.
    let chan = |v: [u16; 2]| v.iter().flat_map(|s| s.to_be_bytes()).collect::<Vec<u8>>();
    let planes = [chan([0xffff, 0]), chan([0, 0xffff]), chan([0x8000, 0x8000])];
    let spec = Spec {
        comp: 1,
        depth: 16,
        ..Spec::rgb8(2, 1)
    };
    let img = psd::decode(&spec.build(&rle_body(&planes, 4)), None).unwrap();
    assert_eq!(px(&img, 0), [255, 0, 128, 255]);
    assert_eq!(px(&img, 1), [0, 255, 128, 255]);
}

fn rgba_planes() -> Vec<u8> {
    // 3 pixels: half-transparent red (white-matted), opaque green, alpha 0.
    // Matted channel = c*a + 255*(1-a) with a = 128/255.
    let r = [255, 0, 7];
    let g = [127, 255, 7];
    let b = [127, 0, 7];
    let a = [128, 255, 0];
    [r, g, b, a].concat()
}

#[test]
fn alpha_is_used_only_when_layer_count_is_negative_and_unmatted() {
    let with = Spec {
        ch: 4,
        layers: Some(-3),
        ..Spec::rgb8(3, 1)
    }
    .build(&rgba_planes());
    let img = psd::decode(&with, None).unwrap();
    let p = px(&img, 0);
    assert_eq!((p[0], p[3]), (255, 128));
    assert!(p[1] <= 1 && p[2] <= 1, "white matte not removed: {p:?}");
    assert_eq!(px(&img, 1), [0, 255, 0, 255]);

    for layers in [None, Some(2)] {
        let file = Spec {
            ch: 4,
            layers,
            ..Spec::rgb8(3, 1)
        }
        .build(&rgba_planes());
        let img = psd::decode(&file, None).unwrap();
        // The 4th channel is a saved selection here, not transparency.
        assert_eq!(px(&img, 0), [255, 127, 127, 255], "layers {layers:?}");
    }
}

#[test]
fn extra_channels_beyond_alpha_are_ignored_and_gray_alpha_works() {
    let five = Spec {
        ch: 5,
        layers: Some(-1),
        ..Spec::rgb8(3, 1)
    };
    let mut body = rgba_planes();
    body.extend([9, 9, 9]); // a spot channel zune-psd refuses outright
    let img = psd::decode(&five.build(&body), None).unwrap();
    assert_eq!(px(&img, 1), [0, 255, 0, 255]);

    let ga = Spec {
        ch: 2,
        mode: 1,
        layers: Some(-1),
        ..Spec::rgb8(2, 1)
    };
    let img = psd::decode(&ga.build(&[100, 200, 255, 64]), None).unwrap();
    assert_eq!(px(&img, 1)[3], 64);
    assert_eq!(px(&img, 0), [100, 100, 100, 255]);
}
