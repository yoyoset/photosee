//! Test-only builders for hand-made TIFF/BMFF byte buffers, plus a
//! counting `ReadAt` that records IO volume and the largest request.

use crate::error::Result;
use crate::io::ReadAt;
use image::codecs::jpeg::JpegEncoder;
use std::sync::Mutex;

pub const SHORT: u16 = 3;
pub const LONG: u16 = 4;
pub const UNDEF: u16 = 7;
pub const IFD: u16 = 13;

/// A real, decodable baseline JPEG of `w`x`h`.
pub fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h * 3).map(|i| (i % 251) as u8).collect();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 80)
        .encode(&px, w, h, image::ExtendedColorType::Rgb8)
        .expect("encode test jpeg");
    out
}

/// `jpeg` with an APP1 EXIF segment (IFD0: Orientation=`o`) after SOI.
pub fn jpeg_with_orientation(w: u32, h: u32, o: u16) -> Vec<u8> {
    let src = jpeg(w, h);
    let mut tiff = TiffB::new(true, 42);
    let ifd0 = tiff.ifd(&[(0x0112, SHORT, 1, u32::from(o))], 0);
    tiff.set_ifd0(ifd0);
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff.buf);
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&src[2..]);
    out
}

/// (tag, type, count, value-or-offset). SHORT count 1 is stored inline.
pub type E = (u16, u16, u32, u32);

/// Appends structures to a growing buffer; returned offsets are relative to
/// the buffer start (the TIFF header, or a MakerNote start).
pub struct TiffB {
    pub le: bool,
    pub buf: Vec<u8>,
}

impl TiffB {
    pub fn new(le: bool, magic: u16) -> Self {
        let mut t = Self::raw(le, if le { b"II" } else { b"MM" });
        t.put16(magic);
        t.put32(0);
        t
    }

    /// No TIFF header: for MakerNotes that start with a maker signature.
    pub fn raw(le: bool, prefix: &[u8]) -> Self {
        Self {
            le,
            buf: prefix.to_vec(),
        }
    }

    pub fn pos(&self) -> u32 {
        self.buf.len() as u32
    }

    fn put16(&mut self, v: u16) {
        let b = if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        };
        self.buf.extend_from_slice(&b);
    }

    fn put32(&mut self, v: u32) {
        let b = if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        };
        self.buf.extend_from_slice(&b);
    }

    pub fn patch32(&mut self, at: u32, v: u32) {
        let b = if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        };
        self.buf[at as usize..at as usize + 4].copy_from_slice(&b);
    }

    pub fn set_ifd0(&mut self, off: u32) {
        self.patch32(4, off);
    }

    pub fn blob(&mut self, data: &[u8]) -> u32 {
        if self.buf.len() % 2 == 1 {
            self.buf.push(0);
        }
        let at = self.pos();
        self.buf.extend_from_slice(data);
        at
    }

    pub fn u32s(&mut self, vals: &[u32]) -> u32 {
        let at = self.blob(&[]);
        vals.iter().for_each(|v| self.put32(*v));
        at
    }

    pub fn ifd(&mut self, entries: &[E], next: u32) -> u32 {
        let at = self.blob(&[]);
        self.put16(entries.len() as u16);
        for &(tag, typ, count, val) in entries {
            self.put16(tag);
            self.put16(typ);
            self.put32(count);
            if typ == SHORT && count == 1 {
                self.put16(val as u16);
                self.put16(0);
            } else {
                self.put32(val);
            }
        }
        self.put32(next);
        at
    }

    /// Offset of entry `idx`'s value field in the IFD at `ifd`.
    pub fn value_at(ifd: u32, idx: u32) -> u32 {
        ifd + 2 + idx * 12 + 8
    }

    /// Offset of the next-IFD pointer of an IFD with `n` entries.
    pub fn next_at(ifd: u32, n: u32) -> u32 {
        ifd + 2 + n * 12
    }
}

pub fn bx(typ: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let mut out = ((content.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(typ);
    out.extend_from_slice(content);
    out
}

/// A full box: version 0, flags 0, then `content`.
pub fn full(typ: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let mut c = vec![0u8; 4];
    c.extend_from_slice(content);
    bx(typ, &c)
}

pub fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// Wraps a buffer and records how much is read through it.
pub struct Counting {
    pub data: Vec<u8>,
    pub stats: Mutex<(u64, usize, usize)>, // (bytes, calls, largest request)
}

impl Counting {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            stats: Mutex::new((0, 0, 0)),
        }
    }

    pub fn bytes(&self) -> u64 {
        self.stats.lock().expect("stats").0
    }

    pub fn largest(&self) -> usize {
        self.stats.lock().expect("stats").2
    }
}

impl ReadAt for Counting {
    fn len(&self) -> Result<u64> {
        ReadAt::len(&self.data)
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let n = self.data.read_at(offset, buf)?;
        let mut s = self.stats.lock().expect("stats");
        s.0 += n as u64;
        s.1 += 1;
        s.2 = s.2.max(buf.len());
        Ok(n)
    }
}

/// Deterministic noise for garbage-input tests.
pub fn noise(seed: u64, n: usize) -> Vec<u8> {
    let mut x = seed | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}
