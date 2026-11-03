//! RAW embedded-preview extraction using range reads only. Package M0-R.
//! Never reads the whole file. Where the largest JPEG lives (see
//! docs/packages/M0-R.md): CR2 IFD0 strip; CR3 PreviewBig trak; NEF SubIFD
//! with NewSubFileType=1, largest; ARW IFD0 JPEGInterchangeFormat; DNG SubIFD;
//! ORF MakerNote; RW2 JpegData; RAF header offset 84.

use crate::error::{Error, Result};
use crate::io::ReadAt;
use crate::types::{ImageInfo, PreviewLoc, RawKind, Rgba8, Size};

/// Byte range of the largest embedded JPEG, plus the RAW's own orientation.
pub fn locate_preview(src: &dyn ReadAt, kind: RawKind) -> Result<PreviewLoc> {
    let _ = (src, kind);
    Err(Error::unsupported("raw: not implemented")) // package M0-R
}

/// Info of the embedded preview (size read from the JPEG header).
pub fn info(src: &dyn ReadAt, kind: RawKind) -> Result<ImageInfo> {
    let _ = (src, kind);
    Err(Error::unsupported("raw: not implemented")) // package M0-R
}

/// Decoded embedded preview, oriented.
pub fn decode(src: &dyn ReadAt, kind: RawKind, max: Option<Size>) -> Result<Rgba8> {
    let _ = (src, kind, max);
    Err(Error::unsupported("raw: not implemented")) // package M0-R
}
