//! Shared value types. This file is a contract (see docs/api.md): only the
//! orchestrator changes it, in a dedicated commit that bumps API_VERSION.

use serde::{Deserialize, Serialize};

/// Bumped on any breaking change to these types or to the HTTP API.
pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RawKind {
    Cr2,
    Cr3,
    Nef,
    Arw,
    Dng,
    Orf,
    Rw2,
    Raf,
}

/// Image container format. Listing guesses it from the extension (opening
/// every file on SMB is too slow); decoding confirms it by magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Jpeg,
    Png,
    Gif,
    Webp,
    Avif,
    Bmp,
    Ico,
    Svg,
    Tiff,
    Jxl,
    Psd,
    Heic,
    Raw(RawKind),
}

/// EXIF orientation 1..=8. `Normal` is 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Normal,
    FlipH,
    Rotate180,
    FlipV,
    Transpose,
    Rotate90,
    Transverse,
    Rotate270,
}

impl Orientation {
    pub fn from_exif(v: u16) -> Self {
        match v {
            2 => Self::FlipH,
            3 => Self::Rotate180,
            4 => Self::FlipV,
            5 => Self::Transpose,
            6 => Self::Rotate90,
            7 => Self::Transverse,
            8 => Self::Rotate270,
            _ => Self::Normal,
        }
    }

    /// True when width and height trade places after applying it.
    pub fn swaps_axes(self) -> bool {
        matches!(
            self,
            Self::Transpose | Self::Rotate90 | Self::Transverse | Self::Rotate270
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

impl Size {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }
    pub fn pixels(self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Image(Format),
    Dir,
}

/// One directory entry. `mtime_ms` is Unix epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub mtime_ms: i64,
}

/// A directory's images and subdirectories, each naturally sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    pub images: Vec<Entry>,
    pub dirs: Vec<Entry>,
}

/// Facts about an image without decoding its pixels. `size` is the stored
/// pixel size *before* orientation is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageInfo {
    pub format: Format,
    pub size: Size,
    pub orientation: Orientation,
    pub animated: bool,
}

impl ImageInfo {
    /// Size as the viewer sees it, after orientation.
    pub fn display_size(&self) -> Size {
        if self.orientation.swaps_axes() {
            Size::new(self.size.h, self.size.w)
        } else {
            self.size
        }
    }
}

/// Decoded pixels: 8-bit RGBA, sRGB, orientation already applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8 {
    pub size: Size,
    pub data: Vec<u8>,
}

/// An encoded image ready to send. `mime` is e.g. "image/jpeg".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

/// How the server answers an image request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Send the file bytes unchanged; the webview decodes and orients them.
    Passthrough,
    /// Send a byte range of the file unchanged (RAW embedded JPEG).
    PassthroughRange { offset: u64, len: u64 },
    /// Decode in core, orient, convert to sRGB, resize to fit `target`, encode.
    Transcode { target: Size },
}

/// Location of a RAW file's largest embedded preview. `orientation` comes from
/// the RAW's own metadata; the embedded JPEG usually carries none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewLoc {
    pub offset: u64,
    pub len: u64,
    pub orientation: Orientation,
}

/// Human-facing EXIF facts for the info overlay (key I). All optional:
/// screenshots and exports usually carry none.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExifSummary {
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    /// e.g. "1/250"
    pub exposure: Option<String>,
    pub f_number: Option<f32>,
    pub focal_mm: Option<f32>,
    /// As stored, e.g. "2024:05:01 13:22:10".
    pub taken: Option<String>,
}
