//! PSD decoder: unsupported variants, hostile structures, truncation and
//! fuzzing. The builder is a trimmed copy of the one in `codecs_psd.rs`
//! (each integration-test file is its own crate).

use photosee_core::ErrorCode;
use photosee_core::decode::psd;

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

fn code(file: &[u8]) -> (ErrorCode, String) {
    let e = psd::decode(&file.to_vec(), None).expect_err("must fail");
    (e.code, e.message)
}

#[test]
fn unsupported_variants_name_themselves() {
    let body = [0u8; 64];
    for (mode, name) in [(4u16, "CMYK"), (9, "Lab"), (2, "Indexed"), (0, "Bitmap")] {
        let (c, m) = code(
            &Spec {
                ch: 4,
                mode,
                ..Spec::rgb8(2, 2)
            }
            .build(&body),
        );
        assert_eq!(c, ErrorCode::Unsupported);
        assert!(m.contains(name), "{m}");
    }
    let (c, m) = code(
        &Spec {
            depth: 32,
            ..Spec::rgb8(2, 2)
        }
        .build(&body),
    );
    assert!(
        (c, m.contains("32")) == (ErrorCode::Unsupported, true),
        "{m}"
    );
    for zip in [2, 3] {
        let (c, m) = code(
            &Spec {
                comp: zip,
                ..Spec::rgb8(2, 2)
            }
            .build(&body),
        );
        assert!(
            (c, m.contains("ZIP")) == (ErrorCode::Unsupported, true),
            "{m}"
        );
    }
    let mut psb = Spec::rgb8(2, 2).build(&body);
    psb[5] = 2;
    assert_eq!(psd::info(&psb).unwrap_err().code, ErrorCode::Unsupported);
    assert!(code(&psb).1.contains("PSB"));
}

#[test]
fn oversized_dimensions_are_too_large_before_allocating() {
    let file = Spec::rgb8(30_000, 30_000).build(&[0u8; 8]);
    assert_eq!(psd::info(&file).unwrap_err().code, ErrorCode::TooLarge);
    assert_eq!(code(&file).0, ErrorCode::TooLarge);
    // Inside the format's limit and the cap, but the file has no pixels.
    let lying = Spec::rgb8(10_000, 10_000).build(&[0u8; 8]);
    assert_eq!(code(&lying).0, ErrorCode::Corrupt);
}

#[test]
fn hostile_structures_are_corrupt() {
    let mut rle = Spec {
        comp: 1,
        ..Spec::rgb8(2, 1)
    }
    .build(&[0u8; 6]);
    rle.extend([0u8; 3]); // zero-length rows can never fill a 2-pixel row
    assert_eq!(code(&rle).0, ErrorCode::Corrupt);
    let mut huge_row = Spec {
        comp: 1,
        ..Spec::rgb8(2, 1)
    }
    .build(&[0xff; 6]);
    huge_row.extend([0u8; 16]);
    assert_eq!(code(&huge_row).0, ErrorCode::Corrupt);
    let mut sections = Spec::rgb8(2, 2).build(&[0u8; 12]);
    sections[30..34].copy_from_slice(&u32::MAX.to_be_bytes()); // resources length
    assert_eq!(code(&sections).0, ErrorCode::Corrupt);
    assert_eq!(code(b"8BPS").0, ErrorCode::Corrupt);
    assert_eq!(code(b"not a psd at all, just text").0, ErrorCode::Corrupt);
}

#[test]
fn every_truncation_is_an_error_and_fuzzing_never_panics() {
    let planes = [vec![1u8; 8], vec![2u8; 8], vec![3u8; 8], vec![200u8; 8]];
    let spec = Spec {
        comp: 1,
        ch: 4,
        layers: Some(-1),
        ..Spec::rgb8(4, 2)
    };
    let file = spec.build(&rle_body(&planes, 4));
    assert!(psd::decode(&file, None).is_ok());
    for len in 0..file.len() {
        assert!(
            psd::decode(&file[..len].to_vec(), None).is_err(),
            "{len}-byte prefix"
        );
    }
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..2000 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut f = file.clone();
        let i = (seed >> 33) as usize % f.len();
        f[i] = (seed >> 20) as u8;
        let _ = psd::decode(&f, None);
        let _ = psd::info(&f);
    }
}

/// Local-only: every `*.psd` in $PHOTOSEE_PRIVATE_FIXTURES must decode or fail
/// with a typed error. Skips when the variable is unset.
#[test]
fn private_fixtures_never_panic() {
    let Some(dir) = std::env::var_os("PHOTOSEE_PRIVATE_FIXTURES") else {
        eprintln!("skipped: PHOTOSEE_PRIVATE_FIXTURES not set");
        return;
    };
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("psd"))
            && e.metadata().unwrap().len() < 40_000_000
        {
            let data = std::fs::read(&p).unwrap();
            let _ = psd::decode(&data, None);
        }
    }
}
