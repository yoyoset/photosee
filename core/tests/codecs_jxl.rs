//! JPEG XL decoder tests. Samples and licenses: `data/codecs/README.md`.

use photosee_core::decode::jxl;
use photosee_core::{ErrorCode, Orientation, Size};
use std::time::Instant;

const DOC: &[u8] = include_bytes!("data/codecs/jxl_oxide_doc_240x135.jxl");
const PLAIN: &[u8] = include_bytes!("data/codecs/mimetype_jxl.jxl");

fn bits(b: &[u8]) -> Vec<bool> {
    b.iter()
        .flat_map(|x| (0..8).map(move |i| (x >> i) & 1 == 1))
        .collect()
}

fn bytes(v: &[bool]) -> Vec<u8> {
    let pack = |c: &[bool]| {
        c.iter()
            .enumerate()
            .fold(0u8, |a, (i, &b)| a | (u8::from(b) << i))
    };
    v.chunks(8).map(pack).collect()
}

fn pad(v: &mut Vec<bool>) {
    v.resize(v.len().div_ceil(8) * 8, false);
}

/// `DOC` with its header rewritten to carry EXIF orientation `o` (1..=8).
/// The original header has `all_default=0, extra_fields=0` (bit 32). Setting
/// `extra_fields` inserts orientation-1 (3 bits), have_intr_size, have_preview,
/// have_animation (all 0) right after it and forces a ToneMapping bundle
/// (`all_default=1`, one bit) before the extensions field. The header then
/// ends one byte later than before, and the frame bytes follow unchanged.
/// The bit positions were found by brute force against the decoder and are
/// confirmed by every pixel check below; they only hold for this sample.
fn with_orientation(o: u8) -> Vec<u8> {
    let ob = bits(DOC);
    let mut v: Vec<bool> = ob[..33].to_vec();
    v[32] = true;
    v.extend((0..3).map(|i| ((o - 1) >> i) & 1 == 1));
    v.extend([false; 3]);
    let rest = &ob[33..45];
    v.extend_from_slice(&rest[..7]);
    v.push(true);
    v.extend_from_slice(&rest[7..]);
    pad(&mut v);
    let mut out = bytes(&v);
    out.extend_from_slice(&DOC[6..]);
    out
}

/// Source pixel of the stored image for output pixel (x, y), per the EXIF
/// orientation table. `w`, `h` are the STORED dimensions.
fn source_of(o: u8, x: usize, y: usize, w: usize, h: usize) -> (usize, usize) {
    match o {
        1 => (x, y),
        2 => (w - 1 - x, y),
        3 => (w - 1 - x, h - 1 - y),
        4 => (x, h - 1 - y),
        5 => (y, x),
        6 => (y, h - 1 - x),
        7 => (w - 1 - y, h - 1 - x),
        _ => (w - 1 - y, x),
    }
}

#[test]
fn plain_image_info_and_decode() {
    let info = jxl::info(&PLAIN.to_vec()).unwrap();
    assert_eq!(
        (info.size, info.orientation, info.animated),
        (Size::new(1000, 1000), Orientation::Normal, false)
    );
    let t = Instant::now();
    let img = jxl::decode(&PLAIN.to_vec(), None).unwrap();
    println!("mimetype_jxl.jxl 1000x1000 decode: {:?}", t.elapsed());
    assert_eq!(img.size, Size::new(1000, 1000));
    assert_eq!(img.data.len(), 1000 * 1000 * 4);
    assert_eq!(&img.data[..4], &[0, 255, 255, 255]);
    let row500 = 500 * 1000 * 4;
    assert_eq!(&img.data[row500..row500 + 4], &[198, 224, 255, 255]);
}

#[test]
fn doc_sample_info_reports_stored_size() {
    let info = jxl::info(&DOC.to_vec()).unwrap();
    assert_eq!(
        (info.size, info.orientation),
        (Size::new(240, 135), Orientation::Normal)
    );
    let t = Instant::now();
    let img = jxl::decode(&DOC.to_vec(), None).unwrap();
    println!("jxl_oxide_doc_240x135.jxl decode: {:?}", t.elapsed());
    assert_eq!(img.size, Size::new(240, 135));
    assert_eq!(&img.data[..8], &[6, 6, 6, 255, 12, 12, 12, 255]);
}

#[test]
fn orientation_is_applied_exactly_once_for_all_eight() {
    let reference = jxl::decode(&DOC.to_vec(), None).unwrap();
    let (w, h) = (240usize, 135usize);
    for o in 1..=8u8 {
        let file = with_orientation(o);
        let info = jxl::info(&file).unwrap();
        // Stored size never swaps; the orientation is reported, not applied.
        assert_eq!(info.size, Size::new(240, 135), "orientation {o}");
        assert_eq!(
            info.orientation,
            Orientation::from_exif(u16::from(o)),
            "orientation {o}"
        );
        let img = jxl::decode(&file, None).unwrap();
        let (ow, oh) = if info.orientation.swaps_axes() {
            (h, w)
        } else {
            (w, h)
        };
        assert_eq!(img.size, Size::new(ow as u32, oh as u32), "orientation {o}");
        assert_eq!(img.size, info.display_size(), "orientation {o}");
        for y in 0..oh {
            for x in 0..ow {
                let (sx, sy) = source_of(o, x, y, w, h);
                let want = &reference.data[(sy * w + sx) * 4..][..4];
                assert_eq!(&img.data[(y * ow + x) * 4..][..4], want, "o={o} at {x},{y}");
            }
        }
    }
}

/// Header-only file: signature, 20000x20000 (400 MP > the 200 MP cap),
/// all-default metadata, no frames.
fn huge_header() -> Vec<u8> {
    let mut v = bits(&[0xff, 0x0a]);
    v.push(false); // div8 = 0
    let dim = |v: &mut Vec<bool>| {
        v.extend([true, true]); // selector 3: 1 + u(30)
        v.extend((0..30).map(|i| (19_999u32 >> i) & 1 == 1));
    };
    dim(&mut v);
    v.extend([false; 3]); // ratio 0: explicit width follows
    dim(&mut v);
    v.extend([true, true]); // metadata all_default, then default_m (read even so)
    pad(&mut v);
    bytes(&v)
}

#[test]
fn pixel_cap_is_checked_before_any_frame_data() {
    let file = huge_header();
    assert_eq!(jxl::info(&file).unwrap().size, Size::new(20_000, 20_000));
    let err = jxl::decode(&file, None).unwrap_err();
    assert_eq!(err.code, ErrorCode::TooLarge, "{err}");
}

#[test]
fn every_truncation_is_an_error_not_a_panic() {
    for len in 0..DOC.len() {
        let cut = DOC[..len].to_vec();
        assert!(
            jxl::decode(&cut, None).is_err(),
            "decode of {len}-byte prefix"
        );
        // info may succeed once the header is complete.
        let _ = jxl::info(&cut);
    }
    for len in (0..PLAIN.len()).step_by(7) {
        assert!(
            jxl::decode(&PLAIN[..len].to_vec(), None).is_err(),
            "{len}-byte prefix"
        );
    }
}

#[test]
fn garbage_and_bit_flips_never_panic() {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..50 {
        let n = (next() % 200) as usize;
        let junk: Vec<u8> = (0..n).map(|_| next() as u8).collect();
        assert!(jxl::decode(&junk, None).is_err());
        let _ = jxl::info(&junk);
    }
    assert!(jxl::decode(&vec![0xffu8, 0x0a], None).is_err());
    for _ in 0..300 {
        let mut f = DOC.to_vec();
        let bit = (next() % (f.len() as u64 * 8)) as usize;
        f[bit / 8] ^= 1 << (bit % 8);
        // Ok or Err are both fine; what matters is returning at all.
        let _ = jxl::decode(&f, None);
    }
}
