//! The list of every fixture, built in memory. Writing and comparing happen
//! elsewhere, so this module is pure: same call, same bytes.

use crate::codec::{self, Result};
use crate::fsio::Entry;
use crate::rng::{XorShift64, SEED};
use crate::{exif, pixels, tiff};

fn file(rel: &str, data: Vec<u8>) -> Entry {
    Entry::File(rel.to_string(), data)
}

/// `orient/1.jpg` .. `orient/8.jpg`, plus `wrong-ext.png` (a copy of 1.jpg).
fn orient_entries() -> Result<Vec<Entry>> {
    let base = codec::jpeg(&pixels::quadrants(300, 200), 90)?;
    let mut out = Vec::new();
    for n in 1..=8u16 {
        let data = exif::insert_orientation(&base, n).ok_or("encoder output is not a JPEG")?;
        if n == 1 {
            out.push(file("wrong-ext.png", data.clone()));
        }
        out.push(file(&format!("orient/{n}.jpg"), data));
    }
    Ok(out)
}

fn p3_entry() -> Result<Entry> {
    let img = pixels::solid_rgba(64, [255, 0, 0, 255]);
    Ok(file(
        "p3.png",
        codec::png_rgba(&img, Some(codec::display_p3_icc()?))?,
    ))
}

fn gif_entry() -> Result<Entry> {
    let frames: Vec<_> = pixels::GIF_COLORS
        .iter()
        .map(|c| pixels::solid_rgba(32, *c))
        .collect();
    Ok(file("anim.gif", codec::gif(&frames, 100)?))
}

/// `pano.jpg` and the 60% cut of it. The cut lands in entropy-coded data, so
/// the file keeps a valid header but loses EOI and the tail of the scan.
fn pano_entries() -> Result<Vec<Entry>> {
    let pano = codec::jpeg(&pixels::horizontal_gradient(6000, 1000), 85)?;
    let truncated = pano[..pano.len() * 6 / 10].to_vec();
    Ok(vec![
        file("pano.jpg", pano),
        file("truncated.jpg", truncated),
    ])
}

fn flat_entries() -> Result<Vec<Entry>> {
    let mut rng = XorShift64::new(SEED);
    let big = pixels::gradient_noise(6000, 4000, &mut rng, 16);
    // Separate stream so garbage.jpg does not depend on how much noise big24mp consumed.
    let mut rng = XorShift64::new(SEED);
    let garbage: Vec<u8> = (0..4096).map(|_| rng.next_byte()).collect();
    Ok(vec![
        file(
            "tall.png",
            codec::png_rgb(&pixels::vertical_gradient(400, 4000))?,
        ),
        file("tiny.png", codec::png_rgb(&pixels::checkerboard(16, 4))?),
        file("big24mp.jpg", codec::jpeg(&big, 90)?),
        file(
            "alpha.png",
            codec::png_rgba(&pixels::alpha_split(128), None)?,
        ),
        file("garbage.jpg", garbage),
        file("huge-header.tif", tiff::huge_header()),
    ])
}

/// Mirrors the golden tree in docs/api.md. Names exercise natural sort
/// (a2 before a10), hidden files, non-images, and empty/nested folders.
fn tree_entries() -> Result<Vec<Entry>> {
    let jpg = |tint| codec::jpeg(&pixels::tinted(8, tint), 90);
    let png = |tint| codec::png_rgb(&pixels::tinted(8, tint));
    Ok(vec![
        file("tree/a10.jpg", jpg(10)?),
        file("tree/a2.jpg", jpg(20)?),
        file("tree/.hidden.jpg", jpg(30)?),
        file("tree/notes.txt", b"x".to_vec()),
        file("tree/2023/b.png", png(40)?),
        Entry::Dir("tree/2024".to_string()),
        file("tree/2025/c.jpg", jpg(50)?),
    ])
}

/// Everything the manifest lists, except `anim.webp` (see main.rs).
pub fn build() -> Result<Vec<Entry>> {
    let mut all = orient_entries()?;
    all.push(p3_entry()?);
    all.push(gif_entry()?);
    all.extend(pano_entries()?);
    all.extend(flat_entries()?);
    all.extend(tree_entries()?);
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{codecs::png::PngDecoder, ImageDecoder, ImageFormat};

    fn find<'a>(all: &'a [Entry], rel: &str) -> &'a [u8] {
        all.iter()
            .find_map(|e| match e {
                Entry::File(p, d) if p == rel => Some(d.as_slice()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{rel} not generated"))
    }

    #[test]
    fn orient_files_carry_their_number_and_decode() {
        let all = orient_entries().unwrap();
        for n in 1..=8u16 {
            let bytes = find(&all, &format!("orient/{n}.jpg"));
            assert_eq!(exif::parse_orientation(bytes), Some(n));
            let img = image::load_from_memory_with_format(bytes, ImageFormat::Jpeg).unwrap();
            assert_eq!((img.width(), img.height()), (300, 200));
        }
        assert_eq!(find(&all, "wrong-ext.png"), find(&all, "orient/1.jpg"));
    }

    #[test]
    fn p3_png_embeds_the_icc_profile() {
        let Entry::File(_, bytes) = p3_entry().unwrap() else {
            panic!("not a file")
        };
        let mut dec = PngDecoder::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(dec.dimensions(), (64, 64));
        let icc = dec.icc_profile().unwrap().expect("iCCP chunk missing");
        assert_eq!(icc, codec::display_p3_icc().unwrap());
        assert_eq!(&icc[36..40], b"acsp");
        // Creation date must be the fixed one, or p3.png changes every run.
        assert_eq!(&icc[24..28], [0x07, 0xE8, 0, 1]);
    }

    #[test]
    fn gif_has_three_frames_that_loop() {
        let Entry::File(_, bytes) = gif_entry().unwrap() else {
            panic!("not a file")
        };
        assert_eq!(&bytes[..6], b"GIF89a");
        // NETSCAPE2.0 application extension marks the loop; count 0 = forever.
        let at = bytes.windows(11).position(|w| w == b"NETSCAPE2.0").unwrap();
        assert_eq!(&bytes[at + 11..at + 16], [3, 1, 0, 0, 0]);
        let frames = bytes.windows(2).filter(|w| *w == [0x21, 0xF9]).count();
        assert_eq!(frames, 3);
        // Quantisation must not shift the pure colours the tests rely on.
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes)).unwrap();
        let decoded = image::AnimationDecoder::into_frames(dec)
            .collect_frames()
            .unwrap();
        assert_eq!(decoded.len(), 3);
        for (f, want) in decoded.iter().zip(pixels::GIF_COLORS) {
            assert_eq!(f.buffer().get_pixel(16, 16).0, want);
            assert_eq!(f.delay().numer_denom_ms(), (100, 1));
        }
    }

    #[test]
    fn truncated_is_a_prefix_at_60_percent() {
        let all = pano_entries().unwrap();
        let (pano, cut) = (find(&all, "pano.jpg"), find(&all, "truncated.jpg"));
        assert_eq!(cut.len(), pano.len() * 6 / 10);
        assert!(pano.starts_with(cut));
    }

    #[test]
    fn tiff_fixture_is_the_hand_written_header() {
        let all = flat_entries().unwrap();
        assert_eq!(
            find(&all, "huge-header.tif"),
            tiff::huge_header().as_slice()
        );
        assert_eq!(find(&all, "garbage.jpg").len(), 4096);
    }

    #[test]
    fn tree_has_the_golden_shape() {
        let all = tree_entries().unwrap();
        assert_eq!(all.len(), 7);
        let img = image::load_from_memory(find(&all, "tree/2023/b.png")).unwrap();
        assert_eq!((img.width(), img.height()), (8, 8));
        assert!(all
            .iter()
            .any(|e| matches!(e, Entry::Dir(p) if p == "tree/2024")));
        assert_eq!(find(&all, "tree/notes.txt"), b"x");
    }
}
