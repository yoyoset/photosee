//! Shared builders for the pipeline_* tests (included with `#[path]`). Also
//! compiled as its own empty test target, hence the allow.
#![allow(dead_code)]

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use photosee_core::io::ReadAt;
use photosee_core::{Result, Rgba8, Size};
use std::sync::atomic::{AtomicU64, Ordering};

pub const RED: [u8; 4] = [255, 0, 0, 255];
pub const GREEN: [u8; 4] = [0, 255, 0, 255];
pub const BLUE: [u8; 4] = [0, 0, 255, 255];
pub const WHITE: [u8; 4] = [255, 255, 255, 255];

/// `w`x`h` RGBA image whose pixels come from `f(x, y)`.
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

/// Quadrants red (top-left), green (top-right), blue (bottom-left), white.
/// All eight orientations map it to distinct images.
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
        .expect("encode jpeg");
    out
}

pub fn png(img: &Rgba8, icc: Option<Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = PngEncoder::new(&mut out);
    if let Some(icc) = icc {
        enc.set_icc_profile(icc).expect("png icc");
    }
    enc.write_image(&img.data, img.size.w, img.size.h, ExtendedColorType::Rgba8)
        .expect("encode png");
    out
}

/// Little-endian TIFF block with IFD0 = one Orientation entry.
pub fn exif_tiff(orientation: u16) -> Vec<u8> {
    let mut t = b"II*\0".to_vec();
    t.extend_from_slice(&8u32.to_le_bytes());
    t.extend_from_slice(&1u16.to_le_bytes());
    t.extend_from_slice(&0x0112u16.to_le_bytes());
    t.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    t.extend_from_slice(&1u32.to_le_bytes());
    t.extend_from_slice(&orientation.to_le_bytes());
    t.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // value padding + no next IFD
    t
}

/// Splices an APP1 Exif segment with the Orientation right after SOI.
pub fn with_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&exif_tiff(orientation));
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// True when the bytes carry an EXIF block anywhere.
pub fn has_exif(bytes: &[u8]) -> bool {
    bytes.windows(6).any(|w| w == b"Exif\0\0")
        || exif::Reader::new()
            .read_from_container(&mut std::io::Cursor::new(bytes))
            .is_ok()
}

pub fn load(bytes: &[u8]) -> image::RgbaImage {
    image::load_from_memory(bytes)
        .expect("decode output")
        .to_rgba8()
}

/// Every channel within `tol` of `want`.
pub fn near(got: &[u8], want: [u8; 4], tol: u8) -> bool {
    got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= tol)
}

/// `ReadAt` over bytes that counts what callers read.
pub struct Counting {
    pub data: Vec<u8>,
    pub read: AtomicU64,
}

impl Counting {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            read: AtomicU64::new(0),
        }
    }
    pub fn bytes_read(&self) -> u64 {
        self.read.load(Ordering::Relaxed)
    }
}

impl ReadAt for Counting {
    fn len(&self) -> Result<u64> {
        Ok(self.data.len() as u64)
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let n = self.data.read_at(offset, buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// TIFF entry: tag, type (3 SHORT, 4 LONG), values.
pub type Entry = (u16, u16, Vec<u32>);

/// Little-endian TIFF: `data` at offset 8, then IFD0 (entries sorted by
/// tag), then out-of-line values. StripOffsets (273) is filled in.
pub fn tiff(data: &[u8], entries: &[Entry]) -> Vec<u8> {
    let mut out = b"II*\0".to_vec();
    let ifd = 8 + data.len().next_multiple_of(2);
    out.extend_from_slice(&(ifd as u32).to_le_bytes());
    out.extend_from_slice(data);
    out.resize(ifd, 0);
    let mut extra_at = ifd + 2 + entries.len() * 12 + 4;
    let mut extra = Vec::new();
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (tag, ty, vals) in entries {
        let vals = if *tag == 273 { vec![8] } else { vals.clone() };
        let bytes: Vec<u8> = vals
            .iter()
            .flat_map(|&v| match ty {
                3 => (v as u16).to_le_bytes().to_vec(),
                _ => v.to_le_bytes().to_vec(),
            })
            .collect();
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&(vals.len() as u32).to_le_bytes());
        if bytes.len() <= 4 {
            let mut inline = bytes.clone();
            inline.resize(4, 0);
            out.extend_from_slice(&inline);
        } else {
            out.extend_from_slice(&(extra_at as u32).to_le_bytes());
            extra_at += bytes.len();
            extra.extend_from_slice(&bytes);
        }
    }
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&extra);
    out
}

/// Baseline uncompressed strip image entries for `w`x`h`.
pub fn tiff_entries(w: u32, h: u32, bits: Vec<u32>, photometric: u32, bytes: u32) -> Vec<Entry> {
    let spp = bits.len() as u32;
    vec![
        (256, 4, vec![w]),
        (257, 4, vec![h]),
        (258, 3, bits),
        (259, 3, vec![1]),
        (262, 3, vec![photometric]),
        (273, 4, vec![0]),
        (277, 3, vec![spp]),
        (278, 4, vec![h]),
        (279, 4, vec![bytes]),
    ]
}

/// Standard CRC-32 (PNG chunks); the png decoder verifies it.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in bytes {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Inserts a chunk right after IHDR.
pub fn png_with_chunk(png: &[u8], kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut chunk = (body.len() as u32).to_be_bytes().to_vec();
    let mut typed = kind.to_vec();
    typed.extend_from_slice(body);
    chunk.extend_from_slice(&typed);
    chunk.extend_from_slice(&crc32(&typed).to_be_bytes());
    let mut out = png[..33].to_vec(); // signature + IHDR
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[33..]);
    out
}
