//! EXIF summary for the info overlay. Package F.

use crate::error::Result;
use crate::io::FsBackend;
use crate::types::ExifSummary;
use std::path::Path;

pub fn exif_summary(fs: &dyn FsBackend, path: &Path) -> Result<ExifSummary> {
    let _ = (fs, path);
    Ok(ExifSummary::default()) // package F
}
