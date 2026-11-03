//! HEIC dispatch by platform. Windows: WIC (package E). macOS/iOS: the webview
//! shows HEIC natively; ImageIO thumbnails come in M3. Linux/Docker: libheif
//! comes in M2. Until then those platforms report the missing codec instead
//! of failing silently.

use crate::error::Result;
use crate::io::ReadAt;
use crate::types::{ImageInfo, Rgba8, Size};

#[cfg(windows)]
pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    super::heic_wic::info(src)
}

#[cfg(windows)]
pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    super::heic_wic::decode(src, max)
}

#[cfg(not(windows))]
fn missing() -> crate::Error {
    use crate::error::{Codec, ErrorCode};
    crate::Error::new(
        ErrorCode::MissingCodec(Codec::Libheif),
        "HEIC decoding is not built for this platform yet",
    )
}

#[cfg(not(windows))]
pub fn info(_src: &dyn ReadAt) -> Result<ImageInfo> {
    Err(missing())
}

#[cfg(not(windows))]
pub fn decode(_src: &dyn ReadAt, _max: Option<Size>) -> Result<Rgba8> {
    Err(missing())
}
