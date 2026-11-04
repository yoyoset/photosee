//! Decide passthrough vs transcode. Package C. Rules (docs/api.md §img):
//! - passthrough when the format is native to the client AND (animated OR
//!   pixels <= 1.5 x request pixels);
//! - RAW: PassthroughRange of the embedded JPEG when its orientation is
//!   Normal and it is <= 1.5 x request, else Transcode;
//! - everything else: Transcode to `resize::fit(display_size, request, false)`.

use crate::format::always_native;
use crate::types::{Delivery, Format, ImageInfo, Orientation, PreviewLoc, Size};

/// Kept for callers; `choose` uses the exact integer form 3/2 below.
pub const PASSTHROUGH_FACTOR: f64 = 1.5;

/// `info` for a RAW is the embedded preview's (see `decode::raw::info`), so
/// its display size is what a range passthrough would show.
pub fn choose(
    info: &ImageInfo,
    request: Size,
    native: &[Format],
    raw_preview: Option<PreviewLoc>,
) -> Delivery {
    let display = info.display_size();
    // Integer form of `display <= PASSTHROUGH_FACTOR * request` (1.5 = 3/2):
    // u32 x u32 products exceed f64's exact range, u128 holds 3x them.
    let small = 2 * u128::from(display.pixels()) <= 3 * u128::from(request.pixels());
    let transcode = Delivery::Transcode {
        target: crate::resize::fit(display, request, false),
    };
    if let Format::Raw(_) = info.format {
        return match raw_preview {
            // Normal = nothing left to apply on top of the JPEG's own tag,
            // which the webview honours.
            Some(p) if p.orientation == Orientation::Normal && small => {
                Delivery::PassthroughRange {
                    offset: p.offset,
                    len: p.len,
                }
            }
            _ => transcode,
        };
    }
    let native = always_native(info.format) || native.contains(&info.format);
    if native && (info.animated || small) {
        Delivery::Passthrough
    } else {
        transcode
    }
}
