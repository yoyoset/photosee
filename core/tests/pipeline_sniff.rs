//! Format sniffing, fit and delivery tables (package C).

use photosee_core::delivery::choose;
use photosee_core::format::sniff;
use photosee_core::resize::fit;
use photosee_core::{Delivery, Format, ImageInfo, Orientation, PreviewLoc, RawKind, Size};

fn ftyp(major: &[u8; 4], compat: &[&[u8; 4]]) -> Vec<u8> {
    let size = 16 + 4 * compat.len() as u32;
    let mut b = size.to_be_bytes().to_vec();
    b.extend_from_slice(b"ftyp");
    b.extend_from_slice(major);
    b.extend_from_slice(&[0, 0, 0, 0]);
    compat.iter().for_each(|c| b.extend_from_slice(*c));
    b
}

fn tiff_with(at8: &[u8], le: bool) -> Vec<u8> {
    let mut b = if le {
        b"II*\0".to_vec()
    } else {
        b"MM\0*".to_vec()
    };
    b.extend_from_slice(&[0, 0, 0, 16]);
    b.extend_from_slice(at8);
    b
}

type SniffCase = (Vec<u8>, Option<Format>, Option<Format>);

fn check_sniff(cases: &[SniffCase]) {
    for (i, (head, guess, want)) in cases.iter().enumerate() {
        assert_eq!(sniff(head, *guess), *want, "case {i}: {head:?}");
    }
}

#[test]
fn sniff_container_magic() {
    use Format as F;
    let raw = |k| Some(F::Raw(k));
    let _ = raw;
    check_sniff(&[
        (vec![0xFF, 0xD8, 0xFF, 0xE0], None, Some(F::Jpeg)),
        (b"\x89PNG\r\n\x1a\n....".to_vec(), None, Some(F::Png)),
        (b"GIF87a..".to_vec(), None, Some(F::Gif)),
        (b"GIF89a..".to_vec(), None, Some(F::Gif)),
        (b"RIFF\x10\0\0\0WEBPVP8 ".to_vec(), None, Some(F::Webp)),
        (b"BM\x36\0\0\0".to_vec(), None, Some(F::Bmp)),
        (vec![0, 0, 1, 0, 1, 0], None, Some(F::Ico)),
        (b"8BPS\0\x01".to_vec(), None, Some(F::Psd)),
        (vec![0xFF, 0x0A, 0xFA], None, Some(F::Jxl)),
        (b"\0\0\0\x0cJXL \r\n\x87\n".to_vec(), None, Some(F::Jxl)),
        (ftyp(b"avif", &[b"mif1", b"miaf"]), None, Some(F::Avif)),
        (ftyp(b"avis", &[]), None, Some(F::Avif)),
        (ftyp(b"mif1", &[b"avif"]), None, Some(F::Avif)),
        (ftyp(b"heic", &[b"mif1"]), None, Some(F::Heic)),
        (ftyp(b"mif1", &[b"heic"]), None, Some(F::Heic)),
        (ftyp(b"msf1", &[b"hevc"]), None, Some(F::Heic)),
        (ftyp(b"mif1", &[b"miaf"]), None, Some(F::Heic)),
        (ftyp(b"isom", &[b"mp42"]), None, None),
    ]);
}

#[test]
fn sniff_raw_tiff_svg_magic() {
    use Format as F;
    let raw = |k| Some(F::Raw(k));
    let _ = raw;
    check_sniff(&[
        (ftyp(b"crx ", &[b"isom"]), None, raw(RawKind::Cr3)),
        (b"FUJIFILMCCD-RAW 0201".to_vec(), None, raw(RawKind::Raf)),
        (b"IIRO\x08\0\0\0".to_vec(), None, raw(RawKind::Orf)),
        (b"IIRS\x08\0\0\0".to_vec(), None, raw(RawKind::Orf)),
        (b"MMOR\0\0\0\x08".to_vec(), None, raw(RawKind::Orf)),
        (b"IIU\0\x08\0\0\0".to_vec(), None, raw(RawKind::Rw2)),
        (tiff_with(b"CR\x02\0", true), None, raw(RawKind::Cr2)),
        (tiff_with(b"\0\0", true), None, Some(F::Tiff)),
        (tiff_with(b"\0\0", false), None, Some(F::Tiff)),
        (
            tiff_with(b"\0\0", true),
            raw(RawKind::Nef),
            raw(RawKind::Nef),
        ),
        (
            tiff_with(b"\0\0", false),
            raw(RawKind::Arw),
            raw(RawKind::Arw),
        ),
        (
            tiff_with(b"\0\0", true),
            raw(RawKind::Dng),
            raw(RawKind::Dng),
        ),
        // A TIFF named .cr2 without the CR marker is not trusted.
        (tiff_with(b"\0\0", true), raw(RawKind::Cr2), Some(F::Tiff)),
        (b"<svg xmlns=".to_vec(), None, Some(F::Svg)),
        (
            b"\xEF\xBB\xBF \n\t<?xml version".to_vec(),
            Some(F::Svg),
            Some(F::Svg),
        ),
        (b"<!-- generator -->".to_vec(), Some(F::Svg), Some(F::Svg)),
        (b"<?xml version".to_vec(), None, None),
    ]);
}

#[test]
fn sniff_magic_beats_extension() {
    // wrong-ext: JPEG bytes named .png
    assert_eq!(
        sniff(&[0xFF, 0xD8, 0xFF, 0xDB], Some(Format::Png)),
        Some(Format::Jpeg)
    );
    assert_eq!(sniff(b"GIF89a", Some(Format::Jpeg)), Some(Format::Gif));
}

#[test]
fn sniff_short_empty_and_garbage() {
    assert_eq!(sniff(&[], None), None);
    assert_eq!(sniff(&[], Some(Format::Jpeg)), None);
    assert_eq!(sniff(&[0xFF], Some(Format::Jpeg)), None);
    assert_eq!(sniff(&[0xFF, 0xD8], None), None);
    assert_eq!(sniff(b"RIFF\0\0", None), None);
    assert_eq!(sniff(b"\0\0\0\x18ftyp", None), None);
    assert_eq!(
        sniff(b"hello, world. not an image", Some(Format::Png)),
        None
    );
    assert_eq!(sniff(&[0x13; 64], None), None);
}

#[test]
fn fit_table() {
    let s = Size::new;
    let cases = [
        (s(6000, 4000), s(2560, 1440), false, s(2160, 1440)),
        (s(4000, 6000), s(2560, 1440), false, s(960, 1440)),
        (s(6000, 4000), s(2560, 1707), false, s(2560, 1707)),
        (s(100, 50), s(2560, 1440), false, s(100, 50)),
        (s(100, 50), s(400, 400), true, s(400, 200)),
        (s(10_000, 1), s(100, 100), false, s(100, 1)),
        (s(3, 3), s(0, 0), false, s(1, 1)),
        (s(0, 5), s(10, 10), false, s(1, 5)),
        (s(1000, 1000), s(1000, 1), false, s(1, 1)),
    ];
    for (src, bound, up, want) in cases {
        assert_eq!(fit(src, bound, up), want, "{src:?} in {bound:?}");
        let t = |x: Size| Size::new(x.h, x.w);
        assert_eq!(fit(t(src), t(bound), up), t(want), "transposed {src:?}");
    }
}

fn info(format: Format, w: u32, h: u32, o: Orientation, animated: bool) -> ImageInfo {
    ImageInfo {
        format,
        size: Size::new(w, h),
        orientation: o,
        animated,
    }
}

type DeliveryCase<'a> = (ImageInfo, &'a [Format], Option<PreviewLoc>, Delivery);

const REQ: Size = Size { w: 2560, h: 1440 };
const RAW: Format = Format::Raw(RawKind::Nef);
const WEBP: [Format; 1] = [Format::Webp];

fn loc(o: Orientation) -> Option<PreviewLoc> {
    Some(PreviewLoc {
        offset: 1000,
        len: 5000,
        orientation: o,
    })
}

fn tc(w: u32, h: u32) -> Delivery {
    Delivery::Transcode {
        target: Size::new(w, h),
    }
}

fn check_delivery(cases: Vec<DeliveryCase<'_>>) {
    for (i, (inf, native, preview, want)) in cases.into_iter().enumerate() {
        assert_eq!(
            choose(&inf, REQ, native, preview),
            want,
            "case {i}: {inf:?}"
        );
    }
}

#[test]
fn delivery_native_sizes() {
    use Orientation::*;
    check_delivery(vec![
        // native small (exactly 1.5x the request is still small)
        (
            info(Format::Jpeg, 2560, 1440, Normal, false),
            &[][..],
            None,
            Delivery::Passthrough,
        ),
        (
            info(Format::Png, 3200, 1728, Normal, false),
            &[],
            None,
            Delivery::Passthrough,
        ),
        // native large
        (
            info(Format::Jpeg, 6000, 4000, Normal, false),
            &[],
            None,
            tc(2160, 1440),
        ),
        (
            info(Format::Jpeg, 4000, 6000, Rotate90, false),
            &[],
            None,
            tc(2160, 1440),
        ),
        // animated large stays native: transcoding would drop the animation
        (
            info(Format::Gif, 6000, 4000, Normal, true),
            &[],
            None,
            Delivery::Passthrough,
        ),
    ]);
}

#[test]
fn delivery_webp_accept() {
    use Orientation::*;
    check_delivery(vec![
        // webp with and without Accept
        (
            info(Format::Webp, 800, 600, Normal, false),
            &WEBP,
            None,
            Delivery::Passthrough,
        ),
        (
            info(Format::Webp, 800, 600, Normal, false),
            &[],
            None,
            tc(800, 600),
        ),
        (
            info(Format::Webp, 800, 600, Normal, true),
            &[],
            None,
            tc(800, 600),
        ),
    ]);
}

#[test]
fn delivery_raw_and_tiff() {
    use Orientation::*;
    check_delivery(vec![
        // RAW: Normal small preview goes as a range, rotated one is decoded
        (
            info(RAW, 1620, 1080, Normal, false),
            &[],
            loc(Normal),
            Delivery::PassthroughRange {
                offset: 1000,
                len: 5000,
            },
        ),
        (
            info(RAW, 1620, 1080, Rotate90, false),
            &[],
            loc(Rotate90),
            tc(960, 1440),
        ),
        (
            info(RAW, 6000, 4000, Normal, false),
            &[],
            loc(Normal),
            tc(2160, 1440),
        ),
        (
            info(RAW, 1620, 1080, Normal, false),
            &[],
            None,
            tc(1620, 1080),
        ),
        // Tiff is never native
        (
            info(Format::Tiff, 800, 600, Normal, false),
            &[Format::Jpeg],
            None,
            tc(800, 600),
        ),
    ]);
}
