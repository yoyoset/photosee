//! End-to-end image rendering used by the server's /img and /meta routes.
//! Package C. Synchronous: the server runs it on a blocking thread.

use crate::decode;
use crate::error::{Error, Result};
use crate::format::{SNIFF_LEN, from_extension, sniff};
use crate::io::{FsBackend, ReadAt};
use crate::types::{Delivery, Encoded, Format, ImageInfo, Size};
use std::path::Path;

/// Transcode JPEG quality: visually lossless at display size, and the
/// encode time barely moves between 80 and 90.
pub const JPEG_QUALITY: u8 = 85;

pub enum Rendered {
    /// Serve the file (or a byte range of it) unchanged.
    Passthrough {
        delivery: Delivery,
        mime: &'static str,
        info: ImageInfo,
    },
    /// Serve these encoded bytes.
    Transcoded { encoded: Encoded, info: ImageInfo },
}

/// Sniff, read info, choose delivery and — when transcoding — decode, orient,
/// color-convert, resize and encode. `native` = formats the client decodes.
pub fn render(
    fs: &dyn FsBackend,
    path: &Path,
    request: Size,
    native: &[Format],
) -> Result<Rendered> {
    let src = fs.open(path)?;
    let src: &dyn ReadAt = &*src;
    let (fmt, info) = probe(src, path)?;
    let preview = match fmt {
        Format::Raw(kind) => Some(decode::raw::locate_preview(src, kind)?),
        _ => None,
    };
    let delivery = crate::delivery::choose(&info, request, native, preview);
    let target = match delivery {
        Delivery::Transcode { target } => target,
        Delivery::Passthrough | Delivery::PassthroughRange { .. } => {
            return Ok(Rendered::Passthrough {
                delivery,
                mime: mime(fmt),
                info,
            });
        }
    };
    let mut img = decode::decode(src, fmt, Some(target))?;
    if img.size.w > target.w || img.size.h > target.h {
        img = crate::resize::resize(&img, target)?;
    }
    Ok(Rendered::Transcoded {
        encoded: crate::encode::encode(&img, JPEG_QUALITY)?,
        info,
    })
}

/// Image facts without decoding pixels.
pub fn info(fs: &dyn FsBackend, path: &Path) -> Result<ImageInfo> {
    let src = fs.open(path)?;
    Ok(probe(&*src, path)?.1)
}

/// Magic bytes decide the format; the extension only breaks ties.
fn probe(src: &dyn ReadAt, path: &Path) -> Result<(Format, ImageInfo)> {
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
    let fmt = sniff(&head[..n], guess).ok_or_else(|| match guess {
        // Named like an image but not one: broken, not merely unknown.
        Some(g) => Error::corrupt(format!("not a {g:?} file (no known signature)")),
        None => Error::unsupported("unknown image format"),
    })?;
    Ok((fmt, decode::info(src, fmt)?))
}

/// MIME of the bytes sent unchanged. A RAW is only ever sent as its
/// embedded JPEG range.
fn mime(fmt: Format) -> &'static str {
    match fmt {
        Format::Jpeg | Format::Raw(_) => "image/jpeg",
        Format::Png => "image/png",
        Format::Gif => "image/gif",
        Format::Webp => "image/webp",
        Format::Avif => "image/avif",
        Format::Bmp => "image/bmp",
        Format::Ico => "image/x-icon",
        Format::Svg => "image/svg+xml",
        Format::Tiff => "image/tiff",
        Format::Jxl => "image/jxl",
        Format::Psd => "image/vnd.adobe.photoshop",
        Format::Heic => "image/heic",
    }
}
