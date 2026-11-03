//! JPEG header scanning inside a byte range: SOI check, SOF size and the
//! APP1 EXIF orientation. Never touches entropy-coded data.

use super::reader::{Reader, Span};
use super::tiff::Tiff;
use crate::error::{Error, Result};
use crate::types::{Orientation, Size};

/// Segments before SOF in camera JPEGs are a handful (APP0/1/2, DQT, DHT);
/// this cap only bounds hostile input.
const MAX_SEGMENTS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Head {
    pub size: Size,
    /// The JPEG's own EXIF orientation; `Normal` when it has none.
    pub orientation: Orientation,
}

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
pub(super) fn scan(r: &Reader, span: Span) -> Result<Head> {
    check_soi(r, span)?;
    let end = span.off + span.len;
    let mut pos = span.off + 2;
    let mut orientation = Orientation::Normal;
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
                    return Ok(Head {
                        size: sof_size(r, pos, end)?,
                        orientation,
                    });
                }
                if marker == 0xE1 && orientation == Orientation::Normal {
                    orientation = exif_orientation(r, pos + 4, seg - 2, end);
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

/// Orientation from an APP1 payload at `at`. Any defect means `Normal`:
/// a broken EXIF block must not hide an otherwise good preview.
fn exif_orientation(r: &Reader, at: u64, len: u64, end: u64) -> Orientation {
    if len < 14 || at + len > end {
        return Orientation::Normal;
    }
    match r.bytes(at, 6) {
        Ok(sig) if sig == b"Exif\0\0" => {}
        _ => return Orientation::Normal,
    }
    let read = || -> Result<Orientation> {
        let (t, ifd0) = Tiff::open(r, at + 6)?;
        Ok(t.orientation(&t.ifd(ifd0)?))
    };
    read().unwrap_or(Orientation::Normal)
}
