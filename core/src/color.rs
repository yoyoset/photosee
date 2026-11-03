//! Color management: convert embedded ICC profiles to sRGB with moxcms so P3
//! (iPhone) and AdobeRGB photos don't look washed out after transcoding.
//! Package C.

use crate::error::Result;
use crate::types::Rgba8;

pub fn to_srgb(img: &mut Rgba8, icc: &[u8]) -> Result<()> {
    let _ = (img, icc);
    Ok(()) // package C
}
