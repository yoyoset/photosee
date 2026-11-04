//! Output encoding for transcodes. Package C. JPEG for opaque images, PNG
//! when any pixel has alpha < 255.
//!
//! Neither encoder writes EXIF unless asked (`set_exif_metadata`), so the
//! output never carries an Orientation tag: the pixels are already oriented.

use crate::error::{Error, Result};
use crate::types::{Encoded, Rgba8};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

pub fn encode(img: &Rgba8, jpeg_quality: u8) -> Result<Encoded> {
    let (w, h) = (img.size.w, img.size.h);
    if img.data.len() as u64 != img.size.pixels() * 4 || img.size.pixels() == 0 {
        // The encoders assert on this; turn an internal bug into an error.
        return Err(Error::corrupt(format!(
            "encode: {w}x{h} with {} bytes",
            img.data.len()
        )));
    }
    let mut bytes = Vec::new();
    let mime = if has_alpha(img) {
        // Fast compression: this is a display copy that lives for one
        // request, not an archive; size matters less than latency.
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, FilterType::Adaptive)
            .write_image(&img.data, w, h, ExtendedColorType::Rgba8)
            .map_err(encode_err)?;
        "image/png"
    } else {
        bytes = jpeg(img, jpeg_quality.clamp(1, 100))?;
        "image/jpeg"
    };
    Ok(Encoded { mime, bytes })
}

/// Opaque RGBA to JPEG. On Windows WIC's encoder goes first (154 -> 29 ms at
/// 2160x1440, see `pipeline_bench`); anything it refuses falls back to `image`.
fn jpeg(img: &Rgba8, quality: u8) -> Result<Vec<u8>> {
    #[cfg(windows)]
    if crate::decode::std_formats::wic_enabled()
        && let Ok(bytes) = crate::encode_wic::jpeg(img, quality)
    {
        return Ok(bytes);
    }
    jpeg_image(img, quality)
}

/// The `image` crate's JPEG encoder: the path on every other platform and
/// the fallback on Windows. Public so benchmarks can time it directly.
pub fn jpeg_image(img: &Rgba8, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (img.size.w, img.size.h);
    let rgb: Vec<u8> = img
        .data
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode(&rgb, w, h, ExtendedColorType::Rgb8)
        .map_err(encode_err)?;
    Ok(bytes)
}

/// True when any pixel is not fully opaque.
pub fn has_alpha(img: &Rgba8) -> bool {
    img.data.chunks_exact(4).any(|p| p[3] != 255)
}

/// Writing into a Vec cannot fail on IO; what remains is e.g. a JPEG side
/// over 65535 px (Unsupported) — same mapping as decoding.
fn encode_err(e: image::ImageError) -> Error {
    crate::decode::std_formats::map_err(e)
}
