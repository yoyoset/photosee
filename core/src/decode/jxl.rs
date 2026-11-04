//! JPEG XL via jxl-oxide. Package D.
//!
//! Orientation: measured, not assumed. `Render::stream()` (used here) applies
//! the header orientation itself and swaps width/height for orientations 5-8,
//! so this module must not apply it again. `ImageInfo::size` is the opposite:
//! the stored size from the header, before orientation (the `ImageInfo`
//! contract), whereas `JxlImage::width()/height()` already include the
//! rotation and are therefore not used.
//!
//! IO: the file is fed to jxl-oxide in 64 KiB chunks and jxl-oxide buffers the
//! codestream itself, so there is no extra whole-file copy here. `info` stops
//! as soon as the header (and ICC / preview frame, if any) has been parsed.
//! There is no cheap reduced decode in jxl-oxide's public API, so `decode`
//! ignores `max` and returns full size; callers resize afterwards.
//!
//! Color: sRGB is requested for the output. With no CMS compiled in
//! (`moxcms`/`lcms2` features are off) jxl-oxide only converts between
//! well-known encodings, which covers every XYB-encoded file (the normal
//! lossy case). Odd ICC profiles on non-XYB files and CMYK come back as errors.

use std::panic::{AssertUnwindSafe, catch_unwind};

use jxl_oxide::{
    AllocTracker, EnumColourEncoding, InitializeResult, JxlImage, PixelFormat, RenderingIntent,
};

use super::std_formats::MAX_PIXELS;
use crate::error::{Error, ErrorCode, Result};
use crate::io::ReadAt;
use crate::types::{ImageInfo, Orientation, Rgba8, Size};

const CHUNK: usize = 64 * 1024;
/// jxl-oxide keeps the codestream in memory, so the file size bounds that.
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
/// Upper bound for jxl-oxide's own sample buffers (f32 planes). 200 MP RGB
/// needs 2.4 GB, which is why the pixel cap below is checked first.
const ALLOC_LIMIT: usize = 6 * 1024 * 1024 * 1024;

fn corrupt(e: impl std::fmt::Display) -> Error {
    Error::corrupt(format!("jxl: {e}"))
}

/// Parses the header (reading `src` as little as possible). Returns the image
/// the file offset read so far and the bytes read but not yet consumed.
fn open_header(src: &dyn ReadAt) -> Result<(JxlImage, u64, Vec<u8>)> {
    if src.len()? > MAX_FILE_BYTES {
        return Err(Error::new(
            ErrorCode::TooLarge,
            "jxl: file larger than 512 MiB",
        ));
    }
    let tracker = AllocTracker::with_limit(ALLOC_LIMIT);
    let mut uninit = JxlImage::builder().alloc_tracker(tracker).build_uninit();
    let (mut pending, mut off) = (Vec::new(), 0u64);
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let n = src.read_at(off, &mut chunk)?;
        if n == 0 {
            return Err(corrupt("file ends before the image header is complete"));
        }
        off += n as u64;
        pending.extend_from_slice(&chunk[..n]);
        let used = uninit.feed_bytes(&pending).map_err(corrupt)?;
        pending.drain(..used);
        match uninit.try_init().map_err(corrupt)? {
            InitializeResult::NeedMoreData(u) => uninit = u,
            InitializeResult::Initialized(mut image) => {
                // Bytes already read but not consumed (a partial container
                // box) must be re-fed together with the next chunk.
                let used = image.feed_bytes(&pending).map_err(corrupt)?;
                pending.drain(..used);
                return Ok((image, off, pending));
            }
        }
    }
}

fn header_info(image: &JxlImage) -> ImageInfo {
    let h = image.image_header();
    ImageInfo {
        format: crate::types::Format::Jxl,
        size: Size::new(h.size.width, h.size.height),
        orientation: Orientation::from_exif(u16::try_from(h.metadata.orientation).unwrap_or(1)),
        animated: h.metadata.animation.is_some(),
    }
}

fn guard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    // A decoder bug on hostile input must not take the server down.
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(corrupt("decoder panicked")))
}

pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    guard(|| Ok(header_info(&open_header(src)?.0)))
}

pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let _ = max; // no cheap reduced decode; the caller resizes
    guard(|| decode_first_frame(src))
}

fn decode_first_frame(src: &dyn ReadAt) -> Result<Rgba8> {
    let (mut image, mut off, mut pending) = open_header(src)?;
    // Before the frame data is even read, let alone rendered.
    let stored = header_info(&image).size;
    if stored.pixels() > MAX_PIXELS {
        return Err(Error::new(
            ErrorCode::TooLarge,
            format!("jxl: {}x{} exceeds the pixel cap", stored.w, stored.h),
        ));
    }
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let n = src.read_at(off, &mut chunk)?;
        if n == 0 {
            break;
        }
        off += n as u64;
        pending.extend_from_slice(&chunk[..n]);
        let used = image.feed_bytes(&pending).map_err(corrupt)?;
        pending.drain(..used);
    }
    image.finalize().map_err(corrupt)?;
    if image.num_loaded_keyframes() == 0 {
        return Err(corrupt("no complete frame (truncated file?)"));
    }
    image.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Relative));
    render_rgba8(&image)
}

fn render_rgba8(image: &JxlImage) -> Result<Rgba8> {
    let format = image.pixel_format();
    if matches!(format, PixelFormat::Cmyk | PixelFormat::Cmyka) {
        return Err(Error::unsupported("jxl: CMYK output is not supported"));
    }
    let render = image.render_frame(0).map_err(corrupt)?;
    let mut stream = render.stream();
    let (w, h, ch) = (stream.width(), stream.height(), stream.channels() as usize);
    let samples = (w as usize)
        .checked_mul(h as usize)
        .and_then(|p| p.checked_mul(ch))
        .ok_or_else(|| Error::new(ErrorCode::TooLarge, "jxl: image too large"))?;
    let mut raw = vec![0u8; samples];
    if stream.write_to_buffer::<u8>(&mut raw) != samples {
        return Err(corrupt("short pixel stream"));
    }
    let mut data = expand_to_rgba(&raw, ch);
    if associated_alpha(image) {
        unpremultiply(&mut data);
    }
    Ok(Rgba8 {
        size: Size::new(w, h),
        data,
    })
}

fn expand_to_rgba(raw: &[u8], ch: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() / ch * 4);
    for px in raw.chunks_exact(ch) {
        match ch {
            1 => out.extend_from_slice(&[px[0], px[0], px[0], 255]),
            2 => out.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
            3 => out.extend_from_slice(&[px[0], px[1], px[2], 255]),
            _ => out.extend_from_slice(&px[..4]),
        }
    }
    out
}

/// True when the first alpha channel is stored premultiplied; `Rgba8` is
/// straight alpha. jxl-oxide hands the samples over as stored.
fn associated_alpha(image: &JxlImage) -> bool {
    let ec = &image.image_header().metadata.ec_info;
    ec.iter()
        .find(|e| e.is_alpha())
        .and_then(|e| e.alpha_associated())
        .unwrap_or(false)
}

fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = u32::from(px[3]);
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}
