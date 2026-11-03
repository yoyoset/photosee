//! EXIF orientation. Package C. Invariant: orientation is applied exactly
//! once — by the webview for passthrough, by core for transcodes (and the
//! transcoded output carries no Orientation tag).

use crate::error::Result;
use crate::io::ReadAt;
use crate::types::{Format, Orientation, Rgba8};

/// Orientation stored in the file's metadata (`Normal` if none). HEIC returns
/// `Normal`: its irot/imir transforms are applied by the HEIC decoder.
pub fn read(src: &dyn ReadAt, fmt: Format) -> Result<Orientation> {
    let _ = (src, fmt);
    Ok(Orientation::Normal) // package C
}

pub fn apply(img: Rgba8, o: Orientation) -> Rgba8 {
    let _ = o;
    img // package C
}
