//! Format identification. `from_extension` is part of the contract (listing
//! depends on it); `sniff` and `is_browser_native` belong to package C.

use crate::decode::std_formats::RangeReader;
use crate::error::{Error, ErrorCode, Result};
use crate::io::ReadAt;
use crate::types::{Format, RawKind, Size};
use std::io::{BufRead, Read, Seek, SeekFrom};

/// Bytes `sniff` needs. CR3/AVIF/HEIC brands sit inside the first ftyp box.
pub const SNIFF_LEN: usize = 64;

/// Guess from the file name. Case-insensitive. `None` = not an image we show.
pub fn from_extension(name: &str) -> Option<Format> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" => Format::Jpeg,
        "png" => Format::Png,
        "gif" => Format::Gif,
        "webp" => Format::Webp,
        "avif" => Format::Avif,
        "bmp" => Format::Bmp,
        "ico" => Format::Ico,
        "svg" => Format::Svg,
        "tif" | "tiff" => Format::Tiff,
        "jxl" => Format::Jxl,
        "psd" => Format::Psd,
        "heic" | "heif" | "hif" => Format::Heic,
        "cr2" => Format::Raw(RawKind::Cr2),
        "cr3" => Format::Raw(RawKind::Cr3),
        "nef" | "nrw" => Format::Raw(RawKind::Nef),
        "arw" | "srf" | "sr2" => Format::Raw(RawKind::Arw),
        "dng" => Format::Raw(RawKind::Dng),
        "orf" => Format::Raw(RawKind::Orf),
        "rw2" => Format::Raw(RawKind::Rw2),
        "raf" => Format::Raw(RawKind::Raf),
        _ => return None,
    })
}

/// Identify by magic bytes; `head` is the first `SNIFF_LEN` bytes (may be
/// shorter for tiny files). Extension decides only when magic is ambiguous
/// (TIFF-based RAWs share the TIFF header).
pub fn sniff(head: &[u8], ext_guess: Option<Format>) -> Option<Format> {
    let starts = |m: &[u8]| head.starts_with(m);
    // Order matters: the RAW signatures below are more specific than TIFF,
    // and the JXL container / ftyp boxes start with zero bytes like ICO.
    let fixed: &[(&[u8], Format)] = &[
        (&[0xFF, 0xD8, 0xFF], Format::Jpeg),
        (b"\x89PNG\r\n\x1a\n", Format::Png),
        (b"GIF87a", Format::Gif),
        (b"GIF89a", Format::Gif),
        (b"8BPS", Format::Psd),
        (&[0xFF, 0x0A], Format::Jxl),
        (&JXL_BOX, Format::Jxl),
        (b"FUJIFILMCCD-RAW", Format::Raw(RawKind::Raf)),
        (b"IIRO", Format::Raw(RawKind::Orf)),
        (b"IIRS", Format::Raw(RawKind::Orf)),
        (b"MMOR", Format::Raw(RawKind::Orf)),
        (b"IIU\0", Format::Raw(RawKind::Rw2)),
    ];
    if let Some(&(_, f)) = fixed.iter().find(|(m, _)| starts(m)) {
        return Some(f);
    }
    if starts(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
        return Some(Format::Webp);
    }
    if head.get(4..8) == Some(b"ftyp") {
        return sniff_ftyp(head);
    }
    if starts(b"II*\0") || starts(b"MM\0*") {
        return Some(sniff_tiff(head, ext_guess));
    }
    if starts(b"BM") {
        return Some(Format::Bmp);
    }
    if starts(&[0, 0, 1, 0]) {
        return Some(Format::Ico);
    }
    sniff_svg(head, ext_guess)
}

const JXL_BOX: [u8; 12] = [
    0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A,
];

/// ISO-BMFF: the major brand, then the compatible brands of the first box.
/// A specific major brand decides; otherwise any specific compatible brand;
/// the generic HEIF brands (`mif1`, `msf1`) mean HEIC only when nothing says
/// AVIF, because AVIF files list them too.
fn sniff_ftyp(head: &[u8]) -> Option<Format> {
    let size = head
        .get(..4)
        .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let size = size as usize;
    let end = size.clamp(16, head.len().max(16)).min(head.len());
    let brands: Vec<&[u8]> = head
        .get(8..end)?
        .chunks_exact(4)
        .enumerate()
        .filter(|(i, _)| *i != 1) // bytes 12..16 are the minor version
        .map(|(_, b)| b)
        .collect();
    let major = brands.first().and_then(|b| specific_brand(b));
    major
        .or_else(|| brands.iter().find_map(|b| specific_brand(b)))
        .or_else(|| {
            let generic = brands.iter().any(|b| *b == b"mif1" || *b == b"msf1");
            generic.then_some(Format::Heic)
        })
}

fn specific_brand(b: &[u8]) -> Option<Format> {
    match b {
        b"crx " => Some(Format::Raw(RawKind::Cr3)),
        b"avif" | b"avis" => Some(Format::Avif),
        b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" => Some(Format::Heic),
        _ => None,
    }
}

/// CR2 marks itself; NEF/ARW/DNG are plain TIFF, so only the name tells.
fn sniff_tiff(head: &[u8], ext_guess: Option<Format>) -> Format {
    if head.get(8..10) == Some(b"CR") {
        return Format::Raw(RawKind::Cr2);
    }
    match ext_guess {
        Some(f @ Format::Raw(RawKind::Nef | RawKind::Arw | RawKind::Dng)) => f,
        _ => Format::Tiff,
    }
}

/// `<svg` is conclusive. `<?xml` and `<!--` start any XML file, so they
/// count only when the name says svg.
fn sniff_svg(head: &[u8], ext_guess: Option<Format>) -> Option<Format> {
    let body = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    let start = body.iter().position(|b| !b.is_ascii_whitespace())?;
    let body = &body[start..];
    let xmlish = body.starts_with(b"<?xml") || body.starts_with(b"<!--");
    (body.starts_with(b"<svg") || (xmlish && ext_guess == Some(Format::Svg))).then_some(Format::Svg)
}

/// Formats every supported webview decodes natively, regardless of `Accept`.
pub fn always_native(f: Format) -> bool {
    matches!(
        f,
        Format::Jpeg | Format::Png | Format::Gif | Format::Bmp | Format::Ico | Format::Svg
    )
}

// ---- Structure probes for `std_formats::info`: just enough parsing to
// answer without decoding pixels. Every loop consumes input, so EOF ends it.

/// JPEG frame size from the first SOF marker, skipping segments by length.
pub(crate) fn jpeg_size(r: &mut RangeReader) -> Result<Size> {
    let is_sof = |m: u8| matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC);
    if !is_sof(jpeg_until(r, |m| is_sof(m) || m == 0xDA)?) {
        return Err(Error::corrupt("JPEG: no SOF before scan"));
    }
    let sof = read_n::<7>(r)?; // length, precision, height, width
    let h = u32::from(u16::from_be_bytes([sof[3], sof[4]]));
    let w = u32::from(u16::from_be_bytes([sof[5], sof[6]]));
    Ok(Size::new(w, h))
}

/// True when an EOI follows the first scan. Needed because zune-jpeg runs in
/// lenient mode behind `image` (not configurable) and "decodes" a truncated
/// file by filling the missing rows with gray. Inside entropy-coded data an
/// 0xFF is always followed by 0x00 or RSTn, so FF D9 there is the real EOI;
/// an EXIF thumbnail's EOI sits before SOS and is skipped with its segment.
pub(crate) fn jpeg_has_eoi(r: &mut RangeReader) -> Result<bool> {
    if jpeg_until(r, |m| m == 0xDA)? != 0xDA {
        return Ok(false);
    }
    let mut prev_ff = false;
    loop {
        let buf = r.fill_buf().map_err(Error::from)?;
        if buf.is_empty() {
            return Ok(false);
        }
        if (prev_ff && buf[0] == 0xD9) || buf.windows(2).any(|w| w == [0xFF, 0xD9]) {
            return Ok(true);
        }
        prev_ff = buf.last() == Some(&0xFF);
        let n = buf.len();
        r.consume(n);
    }
}

/// Walks marker segments from SOI to the first marker `stop` accepts (or
/// EOI), leaving `r` just after that marker byte.
fn jpeg_until(r: &mut RangeReader, stop: impl Fn(u8) -> bool) -> Result<u8> {
    if read_n::<2>(r)? != [0xFF, 0xD8] {
        return Err(Error::corrupt("JPEG: missing SOI"));
    }
    loop {
        if read_n::<1>(r)?[0] != 0xFF {
            return Err(Error::corrupt("JPEG: lost marker sync"));
        }
        // Markers may be padded with any number of 0xFF fill bytes.
        let mut m = 0xFF;
        while m == 0xFF {
            m = read_n::<1>(r)?[0];
        }
        if stop(m) || m == 0xD9 {
            return Ok(m);
        }
        if !matches!(m, 0x01 | 0xD0..=0xD8) {
            let len = u16::from_be_bytes(read_n::<2>(r)?);
            let skip = len
                .checked_sub(2)
                .ok_or_else(|| Error::corrupt("JPEG: segment length < 2"))?;
            r.seek(SeekFrom::Current(i64::from(skip)))
                .map_err(Error::from)?;
        }
    }
}

/// More than one image descriptor. Walks blocks without decoding LZW data;
/// the first frame's sub-blocks are still read, which GIF's layout forces.
pub(crate) fn gif_animated(r: &mut RangeReader) -> Result<bool> {
    let walk = |r: &mut RangeReader| -> Result<bool> {
        let lsd = read_n::<13>(r)?;
        skip_color_table(r, lsd[10])?;
        let mut frames = 0;
        loop {
            match read_n::<1>(r)?[0] {
                0x21 => {
                    read_n::<1>(r)?; // extension label
                    skip_sub_blocks(r)?;
                }
                0x2C => {
                    frames += 1;
                    if frames > 1 {
                        return Ok(true);
                    }
                    let desc = read_n::<9>(r)?;
                    skip_color_table(r, desc[8])?;
                    read_n::<1>(r)?; // LZW minimum code size
                    skip_sub_blocks(r)?;
                }
                _ => return Ok(false), // trailer or junk
            }
        }
    };
    match walk(r) {
        // A truncated or damaged tail still shows its first frame.
        Err(e) if e.code == ErrorCode::Corrupt => Ok(false),
        other => other,
    }
}

fn skip_color_table(r: &mut RangeReader, flags: u8) -> Result<()> {
    if flags & 0x80 != 0 {
        let len = 3i64 << ((flags & 7) + 1);
        r.seek(SeekFrom::Current(len)).map_err(Error::from)?;
    }
    Ok(())
}

fn skip_sub_blocks(r: &mut RangeReader) -> Result<()> {
    loop {
        let n = read_n::<1>(r)?[0];
        if n == 0 {
            return Ok(());
        }
        r.seek(SeekFrom::Current(i64::from(n)))
            .map_err(Error::from)?;
    }
}

/// The VP8X header's animation flag (bit 1 of the flags byte).
pub(crate) fn webp_animated(src: &dyn ReadAt) -> Result<bool> {
    let mut head = [0u8; 21];
    let n = src.read_at(0, &mut head)?;
    Ok(n == head.len() && &head[12..16] == b"VP8X" && head[20] & 0x02 != 0)
}

fn read_n<const N: usize>(r: &mut RangeReader) -> Result<[u8; N]> {
    let mut b = [0u8; N];
    r.read_exact(&mut b).map_err(Error::from)?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_case_insensitive_and_strict() {
        assert_eq!(from_extension("A.JPG"), Some(Format::Jpeg));
        assert_eq!(from_extension("x.CR3"), Some(Format::Raw(RawKind::Cr3)));
        assert_eq!(from_extension("notes.txt"), None);
        assert_eq!(from_extension("jpg"), None);
    }
}
