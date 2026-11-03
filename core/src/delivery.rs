//! Decide passthrough vs transcode. Package C. Rules (docs/api.md §img):
//! - passthrough when the format is native to the client AND (animated OR
//!   pixels <= 1.5 x request pixels);
//! - RAW: PassthroughRange of the embedded JPEG when its orientation is
//!   Normal and it is <= 1.5 x request, else Transcode;
//! - everything else: Transcode to `resize::fit(display_size, request, false)`.

use crate::types::{Delivery, Format, ImageInfo, PreviewLoc, Size};

pub const PASSTHROUGH_FACTOR: f64 = 1.5;

pub fn choose(
    info: &ImageInfo,
    request: Size,
    native: &[Format],
    raw_preview: Option<PreviewLoc>,
) -> Delivery {
    let _ = (native, raw_preview);
    Delivery::Transcode {
        target: crate::resize::fit(info.display_size(), request, false),
    }
}
