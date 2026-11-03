//! Minimal TIFF IFD walker for RAW headers (CR2, NEF, ARW, DNG, ORF, RW2,
//! and TIFF blobs embedded in CR3 CMT1 / JPEG APP1). All offsets are
//! relative to `base`, the position of the TIFF header (or, for Olympus
//! MakerNotes, the MakerNote start).

use super::reader::{Endian, Reader};
use crate::error::{Error, Result};
use crate::types::Orientation;

/// At most this many IFDs per walk; corrupt chains stop here.
pub(super) const MAX_IFDS: usize = 64;
/// At most this many entries per IFD.
const MAX_ENTRIES: u16 = 1024;
/// SubIFD/strip arrays longer than this are not plausible in a RAW.
const MAX_ARRAY: u32 = 64;

pub(super) const TAG_NEW_SUBFILE_TYPE: u16 = 0x00FE;
pub(super) const TAG_COMPRESSION: u16 = 0x0103;
pub(super) const TAG_STRIP_OFFSETS: u16 = 0x0111;
pub(super) const TAG_ORIENTATION: u16 = 0x0112;
pub(super) const TAG_STRIP_BYTE_COUNTS: u16 = 0x0117;
pub(super) const TAG_SUB_IFDS: u16 = 0x014A;
pub(super) const TAG_JPEG_OFFSET: u16 = 0x0201;
pub(super) const TAG_JPEG_LENGTH: u16 = 0x0202;
pub(super) const TAG_EXIF_IFD: u16 = 0x8769;

#[derive(Debug, Clone, Copy)]
pub(super) struct Entry {
    pub tag: u16,
    pub typ: u16,
    pub count: u32,
    /// The 4-byte value/offset field, already in host order as a u32.
    pub raw: u32,
    /// The same field as stored, for inline SHORTs.
    raw_bytes: [u8; 4],
}

#[derive(Debug, Clone)]
pub(super) struct Ifd {
    pub entries: Vec<Entry>,
    pub next: u32,
}

impl Ifd {
    pub fn get(&self, tag: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.tag == tag)
    }
}

pub(super) struct Tiff<'r, 'a> {
    pub r: &'r Reader<'a>,
    pub base: u64,
    pub e: Endian,
}

fn type_size(typ: u16) -> u32 {
    match typ {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => 0,
    }
}

impl<'r, 'a> Tiff<'r, 'a> {
    /// Parses the 8-byte header at `base`; returns the walker and IFD0's
    /// offset. Accepts the RAW magics: 42 (TIFF), 0x4F52/0x5352 (ORF "RO",
    /// "RS") and 0x55 (RW2).
    pub fn open(r: &'r Reader<'a>, base: u64) -> Result<(Self, u32)> {
        let h = r.bytes(base, 8)?;
        let e = match &h[..2] {
            b"II" => Endian::Little,
            b"MM" => Endian::Big,
            _ => return Err(Error::corrupt("raw: not a TIFF header")),
        };
        if !matches!(e.u16(&h, 2), 42 | 0x4F52 | 0x5352 | 0x55) {
            return Err(Error::corrupt("raw: bad TIFF magic"));
        }
        Ok((Self { r, base, e }, e.u32(&h, 4)))
    }

    fn abs(&self, off: u32) -> Result<u64> {
        self.base
            .checked_add(u64::from(off))
            .ok_or_else(|| Error::corrupt("raw: offset overflows"))
    }

    pub fn ifd(&self, off: u32) -> Result<Ifd> {
        let at = self.abs(off)?;
        let n = self.r.u16(at, self.e)?;
        if n == 0 || n > MAX_ENTRIES {
            return Err(Error::corrupt(format!("raw: IFD with {n} entries")));
        }
        let b = self.r.bytes(at + 2, usize::from(n) * 12 + 4)?;
        let entries = b
            .chunks_exact(12)
            .map(|c| Entry {
                tag: self.e.u16(c, 0),
                typ: self.e.u16(c, 2),
                count: self.e.u32(c, 4),
                raw: self.e.u32(c, 8),
                raw_bytes: [c[8], c[9], c[10], c[11]],
            })
            .collect();
        let next = self.e.u32(&b, usize::from(n) * 12);
        Ok(Ifd { entries, next })
    }

    /// Integer values of a SHORT/LONG/IFD entry (at most `MAX_ARRAY`).
    pub fn values(&self, en: &Entry) -> Result<Vec<u32>> {
        let size = type_size(en.typ);
        if !matches!(en.typ, 3 | 4 | 13) || en.count == 0 || en.count > MAX_ARRAY {
            return Err(Error::corrupt(format!(
                "raw: tag {:#06x} not an int array",
                en.tag
            )));
        }
        let total = (size * en.count) as usize;
        let b = if total <= 4 {
            en.raw_bytes[..total].to_vec()
        } else {
            self.r.bytes(self.abs(en.raw)?, total)?
        };
        let step = size as usize;
        Ok((0..en.count as usize)
            .map(|i| match size {
                2 => u32::from(self.e.u16(&b, i * step)),
                _ => self.e.u32(&b, i * step),
            })
            .collect())
    }

    /// First integer value of `tag`, if present and well-formed.
    pub fn first(&self, ifd: &Ifd, tag: u16) -> Option<u32> {
        let en = ifd.get(tag)?;
        self.values(en).ok()?.first().copied()
    }

    /// Absolute offset of a value that is itself relative to `base` (also
    /// the out-of-line data of UNDEFINED blobs such as MakerNote).
    pub fn at(&self, off: u32) -> Result<u64> {
        self.abs(off)
    }

    pub fn orientation(&self, ifd: &Ifd) -> Orientation {
        self.first(ifd, TAG_ORIENTATION)
            .and_then(|v| u16::try_from(v).ok())
            .map_or(Orientation::Normal, Orientation::from_exif)
    }

    /// IFD0's chain plus every SubIFD below it. Revisited offsets are
    /// skipped (corrupt files loop); more than `MAX_IFDS` distinct IFDs is
    /// an error. A broken IFD other than the first is skipped: makers ship
    /// odd private IFDs, and the preview usually sits in a healthy one.
    pub fn walk(&self, first: u32) -> Result<Vec<Ifd>> {
        let mut out: Vec<Ifd> = Vec::new();
        let mut queue = vec![first];
        let mut seen = Vec::new();
        while let Some(off) = queue.pop() {
            if off == 0 || seen.contains(&off) {
                continue;
            }
            seen.push(off);
            if seen.len() > MAX_IFDS {
                return Err(Error::corrupt("raw: too many IFDs"));
            }
            let ifd = match self.ifd(off) {
                Ok(ifd) => ifd,
                Err(e) if off == first => return Err(e),
                Err(_) => continue,
            };
            // Callers filter the whole set, so visiting order does not matter.
            if let Some(subs) = ifd.get(TAG_SUB_IFDS).and_then(|en| self.values(en).ok()) {
                queue.extend(subs);
            }
            queue.push(ifd.next);
            out.push(ifd);
        }
        Ok(out)
    }
}
