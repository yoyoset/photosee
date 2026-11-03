//! Preview location for the plain TIFF-based RAWs: CR2, NEF, ARW, DNG, RW2.
//! Orientation for all of them is IFD0 tag 0x0112.

use super::reader::{Reader, Span};
use super::tiff::{
    Ifd, TAG_COMPRESSION, TAG_JPEG_LENGTH, TAG_JPEG_OFFSET, TAG_NEW_SUBFILE_TYPE,
    TAG_STRIP_BYTE_COUNTS, TAG_STRIP_OFFSETS, Tiff,
};
use crate::error::{Error, Result};
use crate::types::{Orientation, RawKind};

/// Panasonic RW2 IFD0 tag holding a whole JPEG as an UNDEFINED blob.
const TAG_RW2_JPEG: u16 = 0x002E;
/// Old-style JPEG (CR2 IFD0) and new-style JPEG (DNG previews).
const COMPRESSION_OJPEG: u32 = 6;
const COMPRESSION_JPEG: u32 = 7;

pub(super) fn locate(r: &Reader, kind: RawKind) -> Result<(Span, Orientation)> {
    let (t, first) = Tiff::open(r, 0)?;
    let ifd0 = t.ifd(first)?;
    let orientation = t.orientation(&ifd0);
    let span = match kind {
        RawKind::Cr2 => cr2(&t, &ifd0)?,
        RawKind::Arw => {
            jif(&t, &ifd0)?.ok_or_else(|| Error::unsupported("arw: IFD0 has no JPEG preview"))?
        }
        RawKind::Rw2 => rw2(&t, &ifd0)?,
        RawKind::Nef => nef(&t, first)?,
        RawKind::Dng => dng(&t, first)?,
        _ => return Err(Error::unsupported("raw: not a TIFF-based kind")),
    };
    Ok((span, orientation))
}

/// JPEGInterchangeFormat / JPEGInterchangeFormatLength of one IFD.
fn jif(t: &Tiff, ifd: &Ifd) -> Result<Option<Span>> {
    match (t.first(ifd, TAG_JPEG_OFFSET), t.first(ifd, TAG_JPEG_LENGTH)) {
        (Some(off), Some(len)) => Ok(Some(t.r.span(t.at(off)?, u64::from(len))?)),
        _ => Ok(None),
    }
}

/// A single-strip image's byte range. Multi-strip images are not one JPEG
/// stream, so they cannot be passed through.
fn strip(t: &Tiff, ifd: &Ifd) -> Result<Option<Span>> {
    let (Some(offs), Some(lens)) = (ifd.get(TAG_STRIP_OFFSETS), ifd.get(TAG_STRIP_BYTE_COUNTS))
    else {
        return Ok(None);
    };
    let (offs, lens) = (t.values(offs)?, t.values(lens)?);
    match (offs.as_slice(), lens.as_slice()) {
        ([off], [len]) => Ok(Some(t.r.span(t.at(*off)?, u64::from(*len))?)),
        _ => Ok(None),
    }
}

fn cr2(t: &Tiff, ifd0: &Ifd) -> Result<Span> {
    // IFD0 of every CR2 holds the full-size preview as an old-style JPEG
    // strip; the JPEGInterchangeFormat in IFD1 is only the 160x120 thumb.
    let comp = t.first(ifd0, TAG_COMPRESSION);
    if comp != Some(COMPRESSION_OJPEG) {
        return Err(Error::unsupported(format!(
            "cr2: IFD0 compression {comp:?} is not JPEG"
        )));
    }
    strip(t, ifd0)?.ok_or_else(|| Error::unsupported("cr2: IFD0 has no single JPEG strip"))
}

fn rw2(t: &Tiff, ifd0: &Ifd) -> Result<Span> {
    let en = ifd0
        .get(TAG_RW2_JPEG)
        .ok_or_else(|| Error::unsupported("rw2: no JpegData tag"))?;
    // UNDEFINED, count = byte length; the value field is the data offset.
    t.r.span(t.at(en.raw)?, u64::from(en.count))
}

fn is_reduced(t: &Tiff, ifd: &Ifd) -> bool {
    t.first(ifd, TAG_NEW_SUBFILE_TYPE) == Some(1)
}

fn largest(spans: impl Iterator<Item = Span>) -> Option<Span> {
    spans.max_by_key(|s| s.len)
}

fn nef(t: &Tiff, first: u32) -> Result<Span> {
    // Same rule as rawler: among reduced-resolution IFDs anywhere in the
    // tree, the largest JPEGInterchangeFormat stream wins. Out-of-file
    // candidates are dropped rather than failing the whole file.
    let ifds = t.walk(first)?;
    let spans = ifds
        .iter()
        .filter(|ifd| is_reduced(t, ifd))
        .filter_map(|ifd| jif(t, ifd).ok().flatten());
    largest(spans).ok_or_else(|| Error::unsupported("nef: no JPEG preview IFD"))
}

fn dng(t: &Tiff, first: u32) -> Result<Span> {
    let ifds = t.walk(first)?;
    let previews: Vec<&Ifd> = ifds.iter().filter(|ifd| is_reduced(t, ifd)).collect();
    let spans = previews
        .iter()
        .filter(|ifd| t.first(ifd, TAG_COMPRESSION) == Some(COMPRESSION_JPEG))
        .filter_map(|ifd| match jif(t, ifd) {
            Ok(Some(s)) => Some(s),
            _ => strip(t, ifd).ok().flatten(),
        });
    largest(spans).ok_or_else(|| {
        // Uncompressed (1), JPEG XL (52546) or tiled previews land here.
        let comps: Vec<_> = previews
            .iter()
            .filter_map(|ifd| t.first(ifd, TAG_COMPRESSION))
            .collect();
        Error::unsupported(format!(
            "dng: no single-strip JPEG preview (compressions {comps:?})"
        ))
    })
}
