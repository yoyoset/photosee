//! Shared builders for the thumb_* tests (included with `#[path]`). Also
//! compiled as its own empty test target, hence the allows.
#![allow(dead_code)]
#![allow(clippy::unwrap_used, reason = "test helpers")]

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};
use photosee_core::io::{FsBackend, ReadAt};
use photosee_core::testfs::MemFs;
use photosee_core::{Entry, Result, Rgba8, Size};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const RED: [u8; 4] = [255, 0, 0, 255];
pub const GREEN: [u8; 4] = [0, 255, 0, 255];
pub const BLUE: [u8; 4] = [0, 0, 255, 255];
pub const WHITE: [u8; 4] = [255, 255, 255, 255];

pub fn rgba(w: u32, h: u32, mut f: impl FnMut(u32, u32) -> [u8; 4]) -> Rgba8 {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&f(x, y));
        }
    }
    Rgba8 {
        size: Size::new(w, h),
        data,
    }
}

/// Red top-left, green top-right, blue bottom-left, white bottom-right.
pub fn quadrants(w: u32, h: u32) -> Rgba8 {
    rgba(w, h, |x, y| match (x < w / 2, y < h / 2) {
        (true, true) => RED,
        (false, true) => GREEN,
        (true, false) => BLUE,
        (false, false) => WHITE,
    })
}

pub fn jpeg(img: &Rgba8, quality: u8) -> Vec<u8> {
    let rgb: Vec<u8> = img
        .data
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode(&rgb, img.size.w, img.size.h, ExtendedColorType::Rgb8)
        .unwrap();
    out
}

pub fn png(img: &Rgba8) -> Vec<u8> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&img.data, img.size.w, img.size.h, ExtendedColorType::Rgba8)
        .unwrap();
    out
}

pub fn px(img: &image::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    img.get_pixel(x, y).0
}

pub fn near(got: [u8; 4], want: [u8; 4], tol: u8) -> bool {
    got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= tol)
}

pub fn decode(bytes: &[u8]) -> image::RgbaImage {
    image::load_from_memory(bytes).unwrap().to_rgba8()
}

/// One IFD entry: tag, TIFF type, count, raw little-endian value bytes.
pub struct Ent(pub u16, pub u16, pub u32, pub Vec<u8>);

pub fn short(tag: u16, v: u16) -> Ent {
    Ent(tag, 3, 1, v.to_le_bytes().to_vec())
}
pub fn long(tag: u16, v: u32) -> Ent {
    Ent(tag, 4, 1, v.to_le_bytes().to_vec())
}
pub fn ascii(tag: u16, s: &str) -> Ent {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    Ent(tag, 2, b.len() as u32, b)
}
pub fn rational(tag: u16, num: u32, den: u32) -> Ent {
    let mut b = num.to_le_bytes().to_vec();
    b.extend_from_slice(&den.to_le_bytes());
    Ent(tag, 5, 1, b)
}

/// What goes into the EXIF block: IFD0 / ExifIFD entries (sorted by tag
/// here) and an optional IFD1 thumbnail. `thumb_range` overrides the
/// (offset, length) written for it, to build hostile files.
#[derive(Default)]
pub struct ExifSpec {
    pub ifd0: Vec<Ent>,
    pub exif: Vec<Ent>,
    pub thumb: Option<Vec<u8>>,
    pub thumb_range: Option<(u32, u32)>,
}

/// Little-endian TIFF: header, IFD0 (+ExifIFD pointer), IFD1, ExifIFD, then
/// the thumbnail and every out-of-line value.
pub fn tiff(spec: ExifSpec) -> Vec<u8> {
    let ExifSpec {
        mut ifd0,
        mut exif,
        thumb,
        thumb_range,
    } = spec;
    let size = |n: usize| 2 + 12 * n + 4;
    let has_exif = !exif.is_empty();
    let n0 = ifd0.len() + usize::from(has_exif);
    let n1 = if thumb.is_some() { 2 } else { 0 };
    let off0 = 8u32;
    let off1 = off0 + size(n0) as u32;
    let off_exif = off1 + if n1 > 0 { size(n1) as u32 } else { 0 };
    let extra_start = off_exif + if has_exif { size(exif.len()) as u32 } else { 0 };
    if has_exif {
        ifd0.push(long(0x8769, off_exif));
    }
    let mut ifd1 = Vec::new();
    let thumb_len = thumb.as_ref().map_or(0, |t| t.len() as u32);
    if thumb.is_some() {
        let (o, l) = thumb_range.unwrap_or((extra_start, thumb_len));
        ifd1 = vec![long(0x0201, o), long(0x0202, l)];
    }
    let mut extra = thumb.unwrap_or_default();
    if extra.len() % 2 == 1 {
        extra.push(0);
    }
    let mut out = b"II*\0".to_vec();
    out.extend_from_slice(&off0.to_le_bytes());
    let mut w = |entries: &mut Vec<Ent>, next: u32| {
        write_ifd(entries, next, &mut out, &mut extra, extra_start);
    };
    w(&mut ifd0, if n1 > 0 { off1 } else { 0 });
    if n1 > 0 {
        w(&mut ifd1, 0);
    }
    if has_exif {
        w(&mut exif, 0);
    }
    out.extend_from_slice(&extra);
    out
}

/// One IFD; values over 4 bytes are appended to `extra`, which lives at
/// `extra_start` in the final file.
fn write_ifd(
    entries: &mut [Ent],
    next: u32,
    out: &mut Vec<u8>,
    extra: &mut Vec<u8>,
    extra_start: u32,
) {
    entries.sort_by_key(|e| e.0);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for Ent(tag, ty, count, data) in entries.iter() {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if data.len() <= 4 {
            let mut v = data.clone();
            v.resize(4, 0);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&(extra_start + extra.len() as u32).to_le_bytes());
            extra.extend_from_slice(data);
            if data.len() % 2 == 1 {
                extra.push(0);
            }
        }
    }
    out.extend_from_slice(&next.to_le_bytes());
}

/// JPEG with an APP1 Exif segment spliced after SOI.
pub fn jpeg_with_exif(jpeg: &[u8], spec: ExifSpec) -> Vec<u8> {
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff(spec));
    assert!(payload.len() < 65_000, "APP1 too large for one segment");
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// Counts the bytes anything reads through `open`.
pub struct CountFs {
    pub inner: MemFs,
    pub read: Arc<AtomicU64>,
}

impl CountFs {
    pub fn with(path: &str, bytes: &[u8]) -> Self {
        let inner = MemFs::new();
        inner.file(path, bytes);
        Self {
            inner,
            read: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn bytes_read(&self) -> u64 {
        self.read.load(Ordering::Relaxed)
    }
}

struct Counting {
    inner: Box<dyn ReadAt>,
    read: Arc<AtomicU64>,
}

impl ReadAt for Counting {
    fn len(&self) -> Result<u64> {
        self.inner.len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let n = self.inner.read_at(offset, buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

impl FsBackend for CountFs {
    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>> {
        self.inner.read_dir(dir)
    }
    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>> {
        Ok(Box::new(Counting {
            inner: self.inner.open(path)?,
            read: self.read.clone(),
        }))
    }
    fn stat(&self, path: &Path) -> Result<Entry> {
        self.inner.stat(path)
    }
}
