//! RAW embedded-preview extraction using range reads only. Package M0-R.
//! Never reads the whole file. Where the largest JPEG lives (see
//! docs/packages/M0-R.md): CR2 IFD0 strip; CR3 PreviewBig trak; NEF SubIFD
//! with NewSubFileType=1, largest; ARW IFD0 JPEGInterchangeFormat; DNG SubIFD;
//! ORF MakerNote; RW2 JpegData; RAF header offset 84.
//!
//! Layout: `reader` (bounded, cached range reads), `tiff` (IFD walking),
//! `tiff_raw` (CR2/NEF/ARW/DNG/RW2), `orf`, `bmff` (CR3), `raf`, `jpeg`
//! (SOF size, APP1 orientation). Every locator returns a range that
//! `Reader::span` has already checked against the file length.

mod bmff;
mod jpeg;
mod orf;
mod raf;
mod reader;
mod tiff;
mod tiff_raw;

#[cfg(test)]
mod build;
#[cfg(test)]
mod build_kinds;
#[cfg(test)]
mod tests_bmff_raf;
#[cfg(test)]
mod tests_hostile;
#[cfg(test)]
mod tests_tiff;

use crate::error::Result;
use crate::io::ReadAt;
use crate::types::{Format, ImageInfo, PreviewLoc, RawKind, Rgba8, Size};
use reader::{Reader, Span};

fn locate_in(r: &Reader, kind: RawKind) -> Result<(Span, PreviewLoc)> {
    let (span, orientation) = match kind {
        RawKind::Cr3 => bmff::locate(r)?,
        RawKind::Raf => raf::locate(r)?,
        RawKind::Orf => orf::locate(r)?,
        RawKind::Cr2 | RawKind::Nef | RawKind::Arw | RawKind::Dng | RawKind::Rw2 => {
            tiff_raw::locate(r, kind)?
        }
    };
    // Cheap guard against a well-formed header pointing at non-JPEG bytes:
    // the server would otherwise stream garbage labelled image/jpeg.
    jpeg::check_soi(r, span)?;
    let loc = PreviewLoc {
        offset: span.off,
        len: span.len,
        orientation,
    };
    Ok((span, loc))
}

/// Byte range of the largest embedded JPEG, plus the RAW's own orientation.
pub fn locate_preview(src: &dyn ReadAt, kind: RawKind) -> Result<PreviewLoc> {
    let r = Reader::new(src)?;
    Ok(locate_in(&r, kind)?.1)
}

/// Info of the embedded preview (size read from the JPEG header).
pub fn info(src: &dyn ReadAt, kind: RawKind) -> Result<ImageInfo> {
    let r = Reader::new(src)?;
    let (span, loc) = locate_in(&r, kind)?;
    let head = jpeg::scan(&r, span)?;
    Ok(ImageInfo {
        format: Format::Raw(kind),
        size: head.size,
        orientation: loc.orientation,
        animated: false,
    })
}

/// Decoded embedded preview, oriented.
pub fn decode(src: &dyn ReadAt, kind: RawKind, max: Option<Size>) -> Result<Rgba8> {
    let loc = locate_preview(src, kind)?;
    let sub = SubRange {
        src,
        off: loc.offset,
        len: loc.len,
    };
    let img = crate::decode::std_formats::decode(&sub, Format::Jpeg, max)?;
    Ok(crate::orient::apply(img, loc.orientation))
}

/// The preview's bytes seen as a standalone file, so the JPEG decoder can
/// range-read it without copying it out first.
struct SubRange<'a> {
    src: &'a dyn ReadAt,
    off: u64,
    len: u64,
}

impl ReadAt for SubRange<'_> {
    fn len(&self) -> Result<u64> {
        Ok(self.len)
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let Some(rest) = self.len.checked_sub(offset).filter(|&r| r > 0) else {
            return Ok(0);
        };
        let n = usize::try_from(rest).map_or(buf.len(), |rest| rest.min(buf.len()));
        // `locate_preview` proved off+len <= file length, so no overflow.
        self.src.read_at(self.off + offset, &mut buf[..n])
    }
}
