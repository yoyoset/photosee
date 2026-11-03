//! Olympus / OM System ORF: IFD0 -> Exif IFD -> MakerNote -> CameraSettings
//! (tag 0x2020) -> PreviewImageStart/Length (0x0101/0x0102).
//!
//! Pitfall: the MakerNote has three header styles and they disagree on what
//! offsets are relative to (per ExifTool's Olympus.pm):
//!   "OLYMP\0" + 2     IFD at +8,  offsets relative to the file's TIFF header
//!   "OLYMPUS\0II" + 2 IFD at +12, offsets relative to the MakerNote start
//!   "OM SYSTEM\0\0\0II" + 2 IFD at +16, offsets relative to the MakerNote start
//! The new styles also carry their own byte order.

use super::reader::{Endian, Reader, Span};
use super::tiff::{TAG_EXIF_IFD, Tiff};
use crate::error::{Error, Result};
use crate::types::Orientation;

const TAG_MAKER_NOTE: u16 = 0x927C;
const TAG_CAMERA_SETTINGS: u16 = 0x2020;
const TAG_PREVIEW_VALID: u16 = 0x0100;
const TAG_PREVIEW_START: u16 = 0x0101;
const TAG_PREVIEW_LENGTH: u16 = 0x0102;
/// Old makernotes keep the preview in the main makernote IFD instead.
const TAG_OLD_PREVIEW_START: u16 = 0x0088;
const TAG_OLD_PREVIEW_LENGTH: u16 = 0x0089;

pub(super) fn locate(r: &Reader) -> Result<(Span, Orientation)> {
    let (t, first) = Tiff::open(r, 0)?;
    let ifd0 = t.ifd(first)?;
    let orientation = t.orientation(&ifd0);
    let exif_off = t
        .first(&ifd0, TAG_EXIF_IFD)
        .ok_or_else(|| Error::unsupported("orf: no Exif IFD"))?;
    let exif = t.ifd(exif_off)?;
    let mn = exif
        .get(TAG_MAKER_NOTE)
        .ok_or_else(|| Error::unsupported("orf: no MakerNote"))?;
    let mn_at = t.at(mn.raw)?;
    let (mt, mn_ifd) = maker_tiff(r, mn_at, t.e)?;
    Ok((preview(&mt, mn_ifd)?, orientation))
}

/// The MakerNote's own TIFF view and its IFD offset (relative to that view).
fn maker_tiff<'r, 'a>(r: &'r Reader<'a>, at: u64, file_e: Endian) -> Result<(Tiff<'r, 'a>, u32)> {
    let h = r.bytes(at, 16)?;
    let order = |b: &[u8]| match b {
        b"II" => Ok(Endian::Little),
        b"MM" => Ok(Endian::Big),
        _ => Err(Error::corrupt("orf: bad MakerNote byte order")),
    };
    if h.starts_with(b"OLYMPUS\0") {
        let e = order(&h[8..10])?;
        Ok((Tiff { r, base: at, e }, 12))
    } else if h.starts_with(b"OM SYSTEM\0\0\0") {
        let e = order(&h[12..14])?;
        Ok((Tiff { r, base: at, e }, 16))
    } else if h.starts_with(b"OLYMP\0") {
        let off = u32::try_from(at + 8).map_err(|_| Error::corrupt("orf: MakerNote offset"))?;
        Ok((
            Tiff {
                r,
                base: 0,
                e: file_e,
            },
            off,
        ))
    } else {
        Err(Error::unsupported("orf: unknown MakerNote header"))
    }
}

fn preview(t: &Tiff, mn_ifd: u32) -> Result<Span> {
    let main = t.ifd(mn_ifd)?;
    // 0x2020 is type IFD/LONG (value = offset) or UNDEFINED (value = offset
    // of the blob, which is the IFD); the 4-byte field is the offset either way.
    if let Some(cs) = main.get(TAG_CAMERA_SETTINGS) {
        let cs = t.ifd(cs.raw)?;
        if t.first(&cs, TAG_PREVIEW_VALID) == Some(0) {
            return Err(Error::unsupported("orf: camera marked preview invalid"));
        }
        if let (Some(off), Some(len)) = (
            t.first(&cs, TAG_PREVIEW_START),
            t.first(&cs, TAG_PREVIEW_LENGTH),
        ) {
            return t.r.span(t.at(off)?, u64::from(len));
        }
    }
    match (
        t.first(&main, TAG_OLD_PREVIEW_START),
        t.first(&main, TAG_OLD_PREVIEW_LENGTH),
    ) {
        (Some(off), Some(len)) => t.r.span(t.at(off)?, u64::from(len)),
        _ => Err(Error::unsupported("orf: no preview in MakerNote")),
    }
}
