//! Fujifilm RAF: a fixed big-endian header points straight at the JPEG.
//! RAF has no TIFF IFD0 at the front; the camera's orientation lives in
//! the embedded JPEG's own EXIF, so the RAW-level orientation is `Normal`
//! and the JPEG's tag does the work (see `PreviewLoc`).

use super::reader::{Endian, Reader, Span};
use crate::error::{Error, Result};
use crate::types::Orientation;

const MAGIC: &[u8; 16] = b"FUJIFILMCCD-RAW ";
const JPEG_OFFSET_AT: u64 = 84;
const JPEG_LENGTH_AT: u64 = 88;

pub(super) fn locate(r: &Reader) -> Result<(Span, Orientation)> {
    if r.len() < JPEG_LENGTH_AT + 4 || r.bytes(0, 16)? != MAGIC {
        return Err(Error::corrupt("raf: bad header"));
    }
    let off = r.u32(JPEG_OFFSET_AT, Endian::Big)?;
    let len = r.u32(JPEG_LENGTH_AT, Endian::Big)?;
    let span = r.span(u64::from(off), u64::from(len))?;
    Ok((span, Orientation::Normal))
}
