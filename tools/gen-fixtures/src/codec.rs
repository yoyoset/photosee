//! Thin wrappers over `image` encoders, all with fixed settings.

use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{Delay, ExtendedColorType, Frame, ImageEncoder, RgbImage, RgbaImage};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn jpeg(img: &RgbImage, quality: u8) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality).encode(
        img.as_raw(),
        img.width(),
        img.height(),
        ExtendedColorType::Rgb8,
    )?;
    Ok(buf)
}

pub fn png_rgb(img: &RgbImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    PngEncoder::new(&mut buf).write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        ExtendedColorType::Rgb8,
    )?;
    Ok(buf)
}

/// PNG with an optional `iCCP` chunk; the encoder zlib-compresses it itself.
pub fn png_rgba(img: &RgbaImage, icc: Option<Vec<u8>>) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut enc = PngEncoder::new(&mut buf);
    if let Some(profile) = icc {
        enc.set_icc_profile(profile)?;
    }
    enc.write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(buf)
}

/// Infinite-loop GIF, one frame per image, `delay_ms` each.
pub fn gif(frames: &[RgbaImage], delay_ms: u32) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut enc = GifEncoder::new(&mut buf);
        enc.set_repeat(Repeat::Infinite)?;
        for img in frames {
            let delay = Delay::from_numer_denom_ms(delay_ms, 1);
            enc.encode_frame(Frame::from_parts(img.clone(), 0, 0, delay))?;
        }
    }
    Ok(buf)
}

/// Fixed creation date written over the ICC header: 2024-01-01 00:00:00.
const ICC_FIXED_DATE: [u16; 6] = [2024, 1, 1, 0, 0, 0];

/// Display P3 profile serialized by moxcms, so no byte constant is needed.
///
/// Pitfall: `ColorProfile::encode` stamps the header with the current time
/// (moxcms 0.9.1 ignores the profile's own `creation_date_time`), which made
/// p3.png differ on every run. The date is a plain 12-byte field at offset 24
/// and profile_id is written as zeros (no checksum), so patching is safe.
pub fn display_p3_icc() -> Result<Vec<u8>> {
    let mut icc = moxcms::ColorProfile::new_display_p3().encode()?;
    let date = icc
        .get_mut(24..36)
        .ok_or("ICC header shorter than 128 bytes")?;
    for (slot, v) in date.chunks_exact_mut(2).zip(ICC_FIXED_DATE) {
        slot.copy_from_slice(&v.to_be_bytes());
    }
    Ok(icc)
}
