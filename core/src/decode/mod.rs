//! Decoder dispatch. Contract file: orchestrator only. Each format module
//! exposes the same two functions:
//!   `info(src) -> Result<ImageInfo>`     no pixel decode
//!   `decode(src, max) -> Result<Rgba8>`  sRGB, oriented, and (when the codec
//!                                        can do it cheaply) already reduced
//!                                        toward `max`
//! Callers still resize to the exact target afterwards.

pub mod heic;
#[cfg(windows)]
pub mod heic_wic;
#[cfg(windows)]
pub mod jpeg_wic;
pub mod jxl;
pub mod psd;
pub mod raw;
pub mod std_formats;
pub mod svg;

use crate::error::Result;
use crate::io::ReadAt;
use crate::types::{Format, ImageInfo, Rgba8, Size};

pub fn info(src: &dyn ReadAt, fmt: Format) -> Result<ImageInfo> {
    match fmt {
        Format::Jxl => jxl::info(src),
        Format::Psd => psd::info(src),
        Format::Svg => svg::info(src),
        Format::Heic => heic::info(src),
        Format::Raw(kind) => raw::info(src, kind),
        _ => std_formats::info(src, fmt),
    }
}

pub fn decode(src: &dyn ReadAt, fmt: Format, max: Option<Size>) -> Result<Rgba8> {
    match fmt {
        Format::Jxl => jxl::decode(src, max),
        Format::Psd => psd::decode(src, max),
        Format::Svg => svg::decode(src, max),
        Format::Heic => heic::decode(src, max),
        Format::Raw(kind) => raw::decode(src, kind, max),
        _ => std_formats::decode(src, fmt, max),
    }
}
