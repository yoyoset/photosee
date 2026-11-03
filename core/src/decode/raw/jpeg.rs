//! JPEG header scanning inside a byte range: SOI check, SOF size and
//! whether the JPEG carries its own EXIF Orientation tag. Never touches
//! entropy-coded data.

use super::reader::{Endian, Reader, Span};
use crate::error::{Error, Result};
use crate::types::Size;

/// Segments before SOF in camera JPEGs are a handful (APP0/1/2, DQT, DHT);
/// this cap only bounds hostile input.
const MAX_SEGMENTS: usize = 256;

/// APP0/APP1 sit at the very start; 64 KiB is the largest APP1 there is.
const OWN_TAG_WINDOW: u64 = 64 * 1024;

pub(super) fn check_soi(r: &Reader, span: Span) -> Result<()> {
    if span.len < 4 || r.bytes(span.off, 2)? != [0xFF, 0xD8] {
        return Err(Error::corrupt("raw: embedded preview is not a JPEG"));
    }
    Ok(())
}

fn is_sof(m: u8) -> bool {
    // C4 (DHT), C8 (JPG) and CC (DAC) share the range but are not frames.
    matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC)
}

/// Walks markers from SOI to the first SOF. The EXIF thumbnail's own SOF
/// sits inside the APP1 payload, which we skip by length, so the first SOF
/// seen at segment level is the main image's.
pub(super) fn scan(r: &Reader, span: Span) -> Result<Size> {
    check_soi(r, span)?;
    let end = span.off + span.len;
    let mut pos = span.off + 2;
    for _ in 0..MAX_SEGMENTS {
        if pos + 4 > end {
            break;
        }
        let m = r.bytes(pos, 4)?;
        if m[0] != 0xFF {
            return Err(Error::corrupt("raw: JPEG marker expected"));
        }
        match m[1] {
            0xFF => pos += 1, // fill byte
            0x01 | 0xD0..=0xD7 => pos += 2,
            0xD9 | 0xDA => break,
            marker => {
                let seg = u64::from(u16::from_be_bytes([m[2], m[3]]));
                if seg < 2 {
                    return Err(Error::corrupt("raw: JPEG segment length < 2"));
                }
                if is_sof(marker) {
                    return sof_size(r, pos, end);
                }
                pos += 2 + seg;
            }
        }
    }
    Err(Error::corrupt("raw: no SOF marker in embedded JPEG"))
}

fn sof_size(r: &Reader, pos: u64, end: u64) -> Result<Size> {
    if pos + 9 > end {
        return Err(Error::corrupt("raw: truncated SOF"));
    }
    let b = r.bytes(pos + 4, 5)?;
    let h = u16::from_be_bytes([b[1], b[2]]);
    let w = u16::from_be_bytes([b[3], b[4]]);
    if w == 0 || h == 0 {
        // h == 0 means "see DNL"; no camera preview does that.
        return Err(Error::corrupt("raw: JPEG with zero dimension"));
    }
    Ok(Size::new(u32::from(w), u32::from(h)))
}

/// The value of the JPEG's own IFD0 Orientation tag, if it has one. Reads
/// at most the first 64 KiB of the range in one request.
pub(super) fn own_orientation(r: &Reader, span: Span) -> Result<Option<u16>> {
    let n = span.len.min(OWN_TAG_WINDOW);
    // `n` <= 64 KiB, so it fits in usize everywhere.
    let head = r.bytes(span.off, n as usize)?;
    Ok(orientation_tag(&head))
}

/// Pure parse of a JPEG head: SOI, then APP segments up to the first
/// non-APP marker; the first APP1 `Exif\0\0` decides. Any defect means
/// "no tag": a broken EXIF block must not hide a good preview.
pub(super) fn orientation_tag(head: &[u8]) -> Option<u16> {
    if !head.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    for _ in 0..MAX_SEGMENTS {
        let m = head.get(pos..pos + 4)?;
        if m[0] != 0xFF || !(0xE0..=0xEF).contains(&m[1]) {
            return None; // EXIF lives in APPn segments before any table/frame
        }
        let seg = usize::from(u16::from_be_bytes([m[2], m[3]]));
        let payload = head.get(pos + 4..)?;
        let payload = &payload[..payload.len().min(seg.saturating_sub(2))];
        if m[1] == 0xE1 && payload.starts_with(b"Exif\0\0") {
            return tiff_orientation(&payload[6..]);
        }
        pos += 2 + seg.max(2);
    }
    None
}

/// IFD0 tag 0x0112 of an in-memory TIFF blob.
fn tiff_orientation(t: &[u8]) -> Option<u16> {
    let e = match t.get(..2)? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    let ifd0 = usize::try_from(e.u32(t, 4)).ok()?;
    let count = usize::from(e.u16(t.get(ifd0..ifd0 + 2)?, 0)).min(1024);
    (0..count).find_map(|i| {
        let en = t.get(ifd0 + 2 + i * 12..ifd0 + 14 + i * 12)?;
        (e.u16(en, 0) == 0x0112 && e.u16(en, 2) == 3).then(|| e.u16(en, 8))
    })
}
