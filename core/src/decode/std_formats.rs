//! JPEG PNG GIF WebP BMP ICO TIFF via the `image` crate. Package C.
//! Set `image::Limits` (MAX_PIXELS) so a hostile or giant TIFF cannot exhaust
//! memory. AVIF decoding is not compiled in (dav1d needs a C build we don't
//! ship): AVIF returns Unsupported here and is only passed through.
//!
//! Pitfall: `image`'s JPEG decoder reads the whole file into memory in its
//! constructor, so `info` never builds one; it scans for SOF itself.

use crate::error::{Error, ErrorCode, Result};
use crate::io::ReadAt;
use crate::types::{Format, ImageInfo, Rgba8, Size};
use image::error::LimitErrorKind;
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits};
use std::io::{self, BufRead, Read, Seek, SeekFrom};

/// 200 MP: above any current camera, below what exhausts a 4GB phone.
pub const MAX_PIXELS: u64 = 200_000_000;

/// Decoder allocation budget: the RGBA8 output of a MAX_PIXELS image. A
/// 16-bit source spends this twice as fast, so it may hold half the pixels.
const MAX_ALLOC: u64 = MAX_PIXELS * 4;

const BUF_LEN: usize = 64 * 1024;

pub fn info(src: &dyn ReadAt, fmt: Format) -> Result<ImageInfo> {
    let size = match fmt {
        Format::Jpeg => crate::format::jpeg_size(&mut RangeReader::new(src)?)?,
        _ => {
            let (w, h) = decoder(src, fmt)?.dimensions();
            Size::new(w, h)
        }
    };
    check_pixels(size)?;
    let animated = match fmt {
        Format::Gif => crate::format::gif_animated(&mut RangeReader::new(src)?)?,
        Format::Webp => crate::format::webp_animated(src)?,
        _ => false,
    };
    Ok(ImageInfo {
        format: fmt,
        size,
        orientation: crate::orient::read(src, fmt)?,
        animated,
    })
}

/// `max` is a hint: on Windows a JPEG comes back DCT-reduced toward it by
/// WIC (`jpeg_wic`), everything else full-size (zune-jpeg and the other
/// decoders cannot reduce cheaply). Callers resize afterwards either way.
///
/// A WIC failure never surfaces while `image` can still decode the file.
/// Only a backend error (the bytes were unreadable, not undecodable) is
/// returned as is: the fallback would just read them again and fail again.
pub fn decode(src: &dyn ReadAt, fmt: Format, max: Option<Size>) -> Result<Rgba8> {
    #[cfg(windows)]
    if fmt == Format::Jpeg && wic_enabled() {
        if !jpeg_complete(src)? {
            return Err(Error::corrupt("JPEG: truncated (no EOI after the scan)"));
        }
        match crate::decode::jpeg_wic::decode(src, max) {
            Ok(img) => return Ok(img),
            Err(e) if is_backend(&e) => return Err(e),
            Err(_) => {}
        }
    }
    #[cfg(not(windows))]
    let _ = max; // only the WIC path uses the hint
    decode_image(src, fmt)
}

#[cfg(windows)]
fn is_backend(e: &Error) -> bool {
    matches!(
        e.code,
        ErrorCode::NotFound | ErrorCode::Forbidden | ErrorCode::Timeout | ErrorCode::Io
    )
}

/// Benchmarks and tests switch the WIC path off to measure or exercise the
/// pure-Rust one. Read per call (a map lookup, nothing next to a decode).
#[cfg(windows)]
pub(crate) fn wic_enabled() -> bool {
    std::env::var_os("PHOTOSEE_NO_WIC").is_none()
}

/// The `image` crate path: every format, full size, the same on all platforms.
pub fn decode_image(src: &dyn ReadAt, fmt: Format) -> Result<Rgba8> {
    if fmt == Format::Jpeg && !jpeg_complete(src)? {
        return Err(Error::corrupt("JPEG: truncated (no EOI after the scan)"));
    }
    let mut dec = decoder(src, fmt)?;
    let (w, h) = dec.dimensions();
    check_pixels(Size::new(w, h))?;
    if dec.total_bytes() > MAX_ALLOC {
        return Err(too_large(format!(
            "{w}x{h} needs {} bytes",
            dec.total_bytes()
        )));
    }
    // A broken profile must never fail a photo: treat it as absent.
    let icc = dec.icc_profile().ok().flatten();
    let img = DynamicImage::from_decoder(dec).map_err(map_err)?;
    let mut out = Rgba8 {
        size: Size::new(w, h),
        data: into_rgba8(img),
    };
    if let Some(icc) = icc {
        crate::color::to_srgb(&mut out, &icc)?;
    }
    Ok(crate::orient::apply(out, crate::orient::read(src, fmt)?))
}

/// Fast path: the file ends in EOI (allowing zero padding). Otherwise a
/// trailer may follow EOI (motion photos append an MP4), so look properly.
fn jpeg_complete(src: &dyn ReadAt) -> Result<bool> {
    let len = src.len()?;
    let tail_len = len.min(64);
    let tail = crate::io::read_exact_at(src, len - tail_len, tail_len as usize)?;
    let end = tail.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    if tail[..end].ends_with(&[0xFF, 0xD9]) {
        return Ok(true);
    }
    crate::format::jpeg_has_eoi(&mut RangeReader::new(src)?)
}

fn into_rgba8(img: DynamicImage) -> Vec<u8> {
    match img {
        DynamicImage::ImageRgba8(buf) => buf.into_raw(),
        // Covers 16-bit, gray, and CMYK (which `image` already turned into RGB).
        other => other.to_rgba8().into_raw(),
    }
}

/// Header-only decoder with our limits applied (dimension check included).
pub(crate) fn decoder<'a>(src: &'a dyn ReadAt, fmt: Format) -> Result<impl ImageDecoder + 'a> {
    let mut reader = ImageReader::with_format(RangeReader::new(src)?, image_format(fmt)?);
    let mut limits = Limits::default();
    // One side may be as long as the whole pixel budget (panoramas);
    // `check_pixels` bounds the product.
    let side = u32::try_from(MAX_PIXELS).unwrap_or(u32::MAX);
    limits.max_image_width = Some(side);
    limits.max_image_height = Some(side);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    reader.into_decoder().map_err(map_err)
}

fn image_format(fmt: Format) -> Result<ImageFormat> {
    Ok(match fmt {
        Format::Jpeg => ImageFormat::Jpeg,
        Format::Png => ImageFormat::Png,
        Format::Gif => ImageFormat::Gif,
        Format::Webp => ImageFormat::WebP,
        Format::Bmp => ImageFormat::Bmp,
        Format::Ico => ImageFormat::Ico,
        Format::Tiff => ImageFormat::Tiff,
        Format::Avif => return Err(Error::unsupported("AVIF decoding is not built in")),
        other => return Err(Error::unsupported(format!("{other:?} is not a std format"))),
    })
}

pub(crate) fn check_pixels(s: Size) -> Result<()> {
    if s.pixels() > MAX_PIXELS {
        return Err(too_large(format!(
            "{}x{} exceeds {MAX_PIXELS} pixels",
            s.w, s.h
        )));
    }
    if s.w == 0 || s.h == 0 {
        return Err(Error::corrupt(format!("zero image size {}x{}", s.w, s.h)));
    }
    Ok(())
}

fn too_large(msg: String) -> Error {
    Error::new(ErrorCode::TooLarge, msg)
}

pub(crate) fn map_err(e: ImageError) -> Error {
    match e {
        ImageError::Limits(l) => match l.kind() {
            LimitErrorKind::Unsupported { .. } => Error::unsupported(l.to_string()),
            _ => too_large(l.to_string()),
        },
        ImageError::Unsupported(u) => Error::unsupported(u.to_string()),
        // Our reader turns ReadAt errors into io::Error with a matching kind,
        // so Timeout/Forbidden survive the round trip through the decoder.
        ImageError::IoError(io) => Error::from(io),
        other => Error::corrupt(other.to_string()),
    }
}

/// Buffered `Read + BufRead + Seek` over `ReadAt`. Unlike `std::io::BufReader`
/// a seek keeps the buffer, so decoders that hop between nearby chunks
/// (WebP, TIFF, JPEG markers) cost one range read per 64 KiB, not per hop.
pub(crate) struct RangeReader<'a> {
    src: &'a dyn ReadAt,
    len: u64,
    pos: u64,
    buf: Vec<u8>,
    buf_start: u64,
    buf_len: usize,
}

impl<'a> RangeReader<'a> {
    pub(crate) fn new(src: &'a dyn ReadAt) -> Result<Self> {
        Ok(Self {
            src,
            len: src.len()?,
            pos: 0,
            buf: Vec::new(),
            buf_start: 0,
            buf_len: 0,
        })
    }

    /// Offset of `pos` inside the buffer, if it is there.
    fn in_buf(&self) -> Option<usize> {
        let off = self.pos.checked_sub(self.buf_start)?;
        usize::try_from(off).ok().filter(|&o| o < self.buf_len)
    }
}

fn to_io(e: Error) -> io::Error {
    let kind = match e.code {
        ErrorCode::NotFound => io::ErrorKind::NotFound,
        ErrorCode::Forbidden => io::ErrorKind::PermissionDenied,
        ErrorCode::Timeout => io::ErrorKind::TimedOut,
        ErrorCode::Corrupt => io::ErrorKind::InvalidData,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, e.message)
}

impl Read for RangeReader<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.in_buf().is_none() && out.len() >= BUF_LEN {
            // Large reads (JPEG's read_to_end, TIFF strips) skip the copy.
            let n = self.src.read_at(self.pos, out).map_err(to_io)?;
            self.pos += n as u64;
            return Ok(n);
        }
        let n = {
            let avail = self.fill_buf()?;
            let n = avail.len().min(out.len());
            out[..n].copy_from_slice(&avail[..n]);
            n
        };
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for RangeReader<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let off = match self.in_buf() {
            Some(off) => off,
            None => {
                self.buf.resize(BUF_LEN, 0);
                self.buf_len = self.src.read_at(self.pos, &mut self.buf).map_err(to_io)?;
                self.buf_start = self.pos;
                0
            }
        };
        Ok(&self.buf[off..self.buf_len])
    }

    fn consume(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n as u64);
    }
}

impl Seek for RangeReader<'_> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        self.pos = target
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(self.pos)
    }
}
