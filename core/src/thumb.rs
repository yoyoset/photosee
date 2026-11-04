//! Thumbnails for the filmstrip. Package F. Prefer the EXIF IFD1 embedded
//! thumbnail (a few KB, no decode); otherwise decode and downscale. Animated
//! images use their first frame.

use crate::decode::{self, std_formats};
use crate::error::{Error, Result};
use crate::format::{SNIFF_LEN, from_extension, sniff};
use crate::io::{FsBackend, ReadAt};
use crate::meta::{self, EXIF_HEAD};
use crate::types::{Encoded, Format, Orientation, RawKind, Rgba8, Size};
use exif::{In, Tag};
use std::path::Path;

/// Longest edge of a generated thumbnail, in pixels.
pub const DEFAULT_EDGE: u32 = 256;

/// Quality of the thumbnail JPEG. Thumbnails are small and viewed small, so
/// this is lower than the full-size transcode quality.
pub const THUMB_QUALITY: u8 = 80;

/// First head read for a JPEG: APP1 is at most 64 KiB and sits near the
/// start, so the 1 MiB read is only a retry for files with big APPn first.
const JPEG_HEAD: u64 = 128 * 1024;

/// An IFD1 thumbnail whose aspect ratio differs from the photo's by more
/// than this (relative) is a letterboxed or cropped preview: not usable.
const ASPECT_TOLERANCE: f64 = 0.03;

/// JPEG bytes of at most `edge` px on the long side (never upscaled).
pub fn make(fs: &dyn FsBackend, path: &Path, edge: u32) -> Result<Encoded> {
    let edge = edge.max(1);
    let src = fs.open(path)?;
    let src: &dyn ReadAt = &*src;
    let fmt = format_of(src, path)?;
    let img = match embedded(src, fmt, edge) {
        Some(img) => img,
        None => decode::decode(src, fmt, Some(Size::new(edge, edge)))?,
    };
    finish(img, edge)
}

/// Magic bytes decide the format; the extension only breaks ties (same rule
/// as `pipeline::info`, whose probe is private to package C).
fn format_of(src: &dyn ReadAt, path: &Path) -> Result<Format> {
    let mut head = [0u8; SNIFF_LEN];
    let mut n = 0;
    while n < head.len() {
        let got = src.read_at(n as u64, &mut head[n..])?;
        if got == 0 {
            break;
        }
        n += got;
    }
    let guess = path
        .file_name()
        .and_then(|f| f.to_str())
        .and_then(from_extension);
    sniff(&head[..n], guess).ok_or_else(|| match guess {
        Some(g) => Error::corrupt(format!("not a {g:?} file (no known signature)")),
        None => Error::unsupported("unknown image format"),
    })
}

fn has_ifd1_jpeg(fmt: Format) -> bool {
    matches!(
        fmt,
        Format::Jpeg
            | Format::Tiff
            | Format::Raw(RawKind::Cr2 | RawKind::Nef | RawKind::Arw | RawKind::Dng)
    )
}

/// The IFD1 thumbnail, decoded and oriented, or `None` for "use the slow
/// path": no thumbnail, too small, wrong aspect, damaged, or unreadable. A
/// thumbnail shortcut must never be able to fail a thumbnail.
fn embedded(src: &dyn ReadAt, fmt: Format, edge: u32) -> Option<Rgba8> {
    if !has_ifd1_jpeg(fmt) {
        return None;
    }
    let exif = read_head(src, fmt).ok()??;
    let bytes = ifd1_bytes(&exif)?;
    let thumb = bytes.to_vec();
    let size = std_formats::info(&thumb, Format::Jpeg).ok()?.size;
    if u64::from(size.w.max(size.h)) * 2 < u64::from(edge) {
        return None;
    }
    if !aspect_matches(&exif, size) {
        return None;
    }
    let img = std_formats::decode(&thumb, Format::Jpeg, None).ok()?;
    // IFD1 is stored unrotated, so the photo's orientation applies. A
    // thumbnail that carries its own tag was already rotated by the decoder.
    let own = crate::orient::read(&thumb, Format::Jpeg).ok()?;
    if own != Orientation::Normal {
        return Some(img);
    }
    Some(crate::orient::apply(img, orientation_of(&exif)))
}

fn read_head(src: &dyn ReadAt, fmt: Format) -> Result<Option<exif::Exif>> {
    if fmt == Format::Jpeg
        && let Some(e) = meta::read_exif(src, JPEG_HEAD)?
    {
        return Ok(Some(e));
    }
    meta::read_exif(src, EXIF_HEAD)
}

/// The thumbnail's byte range inside the EXIF block. Offsets are relative to
/// the TIFF header, which is where `Exif::buf` starts. Hostile files can put
/// any numbers here.
fn ifd1_bytes(exif: &exif::Exif) -> Option<&[u8]> {
    let get = |tag| exif.get_field(tag, In::THUMBNAIL)?.value.get_uint(0);
    let off = usize::try_from(get(Tag::JPEGInterchangeFormat)?).ok()?;
    let len = usize::try_from(get(Tag::JPEGInterchangeFormatLength)?).ok()?;
    let bytes = exif.buf().get(off..off.checked_add(len)?)?;
    bytes.starts_with(&[0xFF, 0xD8]).then_some(bytes)
}

fn orientation_of(exif: &exif::Exif) -> Orientation {
    exif.get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .and_then(|v| u16::try_from(v).ok())
        .map_or(Orientation::Normal, Orientation::from_exif)
}

/// Compares against the photo's stored pixel size (ExifIFD
/// PixelXDimension/PixelYDimension) when the file states it. Long/short
/// ratio, so a rotated thumbnail still compares equal. Unstated → accept.
fn aspect_matches(exif: &exif::Exif, thumb: Size) -> bool {
    let dim = |tag| {
        exif.get_field(tag, In::PRIMARY)?
            .value
            .get_uint(0)
            .filter(|&v| v > 0)
    };
    let (Some(w), Some(h)) = (dim(Tag::PixelXDimension), dim(Tag::PixelYDimension)) else {
        return true;
    };
    let ratio = |a: u32, b: u32| f64::from(a.max(b)) / f64::from(a.min(b).max(1));
    let want = ratio(w, h);
    (ratio(thumb.w, thumb.h) - want).abs() / want <= ASPECT_TOLERANCE
}

/// Fit into `edge`, flatten alpha onto black, encode.
fn finish(mut img: Rgba8, edge: u32) -> Result<Encoded> {
    let to = crate::resize::fit(img.size, Size::new(edge, edge), false);
    if to != img.size {
        img = crate::resize::resize(&img, to)?;
    }
    flatten_on_black(&mut img);
    crate::encode::encode(&img, THUMB_QUALITY)
}

/// The stage is black, so that is what transparency should look like. This
/// also keeps `encode` on its JPEG path (it picks PNG for any alpha).
fn flatten_on_black(img: &mut Rgba8) {
    for p in img.data.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a != 255 {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * a + 127) / 255) as u8;
            }
            p[3] = 255;
        }
    }
}
