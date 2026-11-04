//! EXIF summary for the info overlay. Package F.

use crate::error::Result;
use crate::io::{FsBackend, ReadAt, read_exact_at};
use crate::types::ExifSummary;
use exif::{Exif, In, Tag, Value};
use std::path::Path;

/// Upper bound of any metadata head read. EXIF in JPEG/PNG/HEIC and the first
/// IFDs of TIFF-based RAW live well inside it; on SMB it is one round trip.
pub(crate) const EXIF_HEAD: u64 = 1 << 20;

/// Parses the EXIF in the first `limit` bytes of `src`. `None` when there is
/// none or it is damaged (a photo must never fail over its metadata); only IO
/// errors propagate. kamadak-exif sniffs JPEG, TIFF, HEIF, PNG and WebP.
pub(crate) fn read_exif(src: &dyn ReadAt, limit: u64) -> Result<Option<Exif>> {
    let len = src.len()?.min(limit) as usize;
    let head = read_exact_at(src, 0, len)?;
    Ok(exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(head))
        .ok())
}

pub fn exif_summary(fs: &dyn FsBackend, path: &Path) -> Result<ExifSummary> {
    let src = fs.open(path)?;
    Ok(read_exif(&*src, EXIF_HEAD)?.map_or_else(ExifSummary::default, |e| summarize(&e)))
}

fn summarize(e: &Exif) -> ExifSummary {
    ExifSummary {
        camera: camera(text(e, Tag::Make), text(e, Tag::Model)),
        lens: text(e, Tag::LensModel),
        iso: e
            .get_field(Tag::PhotographicSensitivity, In::PRIMARY)
            .and_then(|f| f.value.get_uint(0)),
        exposure: rational(e, Tag::ExposureTime).and_then(exposure),
        f_number: rational(e, Tag::FNumber).map(|v| v as f32),
        focal_mm: rational(e, Tag::FocalLength).map(|v| v as f32),
        taken: text(e, Tag::DateTimeOriginal),
    }
}

/// First ASCII string of a tag, trimmed; empty counts as missing. Cameras pad
/// with spaces or NULs.
fn text(e: &Exif, tag: Tag) -> Option<String> {
    match &e.get_field(tag, In::PRIMARY)?.value {
        Value::Ascii(v) => {
            let s = String::from_utf8_lossy(v.first()?);
            let s = s.trim_matches(|c: char| c.is_whitespace() || c == '\0');
            (!s.is_empty()).then(|| s.to_owned())
        }
        _ => None,
    }
}

/// First rational as a float. A zero denominator is garbage, not infinity.
fn rational(e: &Exif, tag: Tag) -> Option<f64> {
    match &e.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(v) => {
            let r = v.first()?;
            (r.denom != 0).then(|| f64::from(r.num) / f64::from(r.denom))
        }
        _ => None,
    }
}

/// Many makers repeat themselves in Model ("Canon" + "Canon EOS R6").
fn camera(make: Option<String>, model: Option<String>) -> Option<String> {
    match (make, model) {
        (Some(make), Some(model)) => {
            let same = model.len() >= make.len()
                && model.is_char_boundary(make.len())
                && model[..make.len()].eq_ignore_ascii_case(&make);
            Some(if same {
                model
            } else {
                format!("{make} {model}")
            })
        }
        (one, other) => one.or(other),
    }
}

/// "1/250" under a second (shown as 1/round(1/t)), seconds with at most one
/// decimal otherwise ("2.5", "30").
fn exposure(t: f64) -> Option<String> {
    if !t.is_finite() || t <= 0.0 {
        return None;
    }
    if t < 1.0 {
        return Some(format!("1/{}", (1.0 / t).round() as u64));
    }
    let s = format!("{t:.1}");
    Some(s.strip_suffix(".0").map_or(s.clone(), str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_formats() {
        assert_eq!(exposure(0.004).as_deref(), Some("1/250"));
        assert_eq!(exposure(2.5).as_deref(), Some("2.5"));
        assert_eq!(exposure(30.0).as_deref(), Some("30"));
        assert_eq!(exposure(1.0).as_deref(), Some("1"));
        assert_eq!(exposure(0.0), None);
        // 10/600 s = 1/60
        assert_eq!(exposure(10.0 / 600.0).as_deref(), Some("1/60"));
    }

    #[test]
    fn camera_dedupes_make() {
        let c = |a: &str, b: &str| camera(Some(a.into()), Some(b.into()));
        assert_eq!(c("Canon", "Canon EOS R6").as_deref(), Some("Canon EOS R6"));
        assert_eq!(c("NIKON", "Z 6").as_deref(), Some("NIKON Z 6"));
        assert_eq!(camera(None, Some("X".into())).as_deref(), Some("X"));
        assert_eq!(camera(Some("Y".into()), None).as_deref(), Some("Y"));
    }
}
