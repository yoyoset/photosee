//! Canon CR3 (ISO-BMFF). Layout, per rawler and the canon_cr3 notes:
//!   ftyp("crx ") moov{ uuid(Canon){ CNCV CCTP{CCDT..} CTBO CMT1..4 THMB }
//!                      trak.. } uuid(XMP) uuid(PRVW) mdat
//! CCTP's CCDT records map image types to trak numbers; type 2 is the
//! full-size JPEG ("PreviewBig"). Its first sample is the JPEG we want:
//! offset = co64/stco[0], length = stsz[0]. THMB (160x120) and PRVW
//! (1620x1080) are smaller previews and deliberately ignored.

use super::reader::{Endian, Reader, Span};
use super::tiff::Tiff;
use crate::error::{Error, Result};
use crate::types::Orientation;

/// Children scanned per container; real CR3 boxes have fewer than 16.
const MAX_CHILDREN: usize = 256;
const CANON_UUID: [u8; 16] = [
    0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48,
];
const PREVIEW_BIG: u64 = 2;
const BE: Endian = Endian::Big;

#[derive(Debug, Clone, Copy)]
struct Bx {
    typ: [u8; 4],
    /// First content byte (after size/type/largesize).
    body: u64,
    end: u64,
}

fn read_box(r: &Reader, at: u64, limit: u64) -> Result<Bx> {
    let h = r.bytes(at, 8)?;
    let size32 = BE.u32(&h, 0);
    let typ = [h[4], h[5], h[6], h[7]];
    let (size, hdr) = match size32 {
        0 => (limit - at, 8), // extends to the end of its container
        1 => (r.u64(at + 8, BE)?, 16),
        n => (u64::from(n), 8),
    };
    let end = at.checked_add(size).filter(|&e| size >= hdr && e <= limit);
    let end = end.ok_or_else(|| Error::corrupt("cr3: box size out of range"))?;
    Ok(Bx {
        typ,
        body: at + hdr,
        end,
    })
}

/// Direct children of the range `[from, to)`.
fn children(r: &Reader, from: u64, to: u64) -> Result<Vec<Bx>> {
    let mut out = Vec::new();
    let mut at = from;
    while at + 8 <= to {
        if out.len() == MAX_CHILDREN {
            return Err(Error::corrupt("cr3: too many boxes"));
        }
        let b = read_box(r, at, to)?;
        at = b.end;
        out.push(b);
    }
    Ok(out)
}

fn find(r: &Reader, parent: Bx, typ: &[u8; 4]) -> Result<Bx> {
    children(r, parent.body, parent.end)?
        .into_iter()
        .find(|b| &b.typ == typ)
        .ok_or_else(|| Error::corrupt(format!("cr3: no {} box", String::from_utf8_lossy(typ))))
}

/// `n` bytes at `rel` inside a box body; a field past the box end is corrupt.
fn field(r: &Reader, b: Bx, rel: u64, n: usize) -> Result<Vec<u8>> {
    if b.body + rel + n as u64 > b.end {
        return Err(Error::corrupt("cr3: box too short"));
    }
    r.bytes(b.body + rel, n)
}

/// Walks top-level boxes only until moov: the mdat after it is huge and
/// anything trailing it is irrelevant.
fn moov(r: &Reader) -> Result<Bx> {
    let ftyp = read_box(r, 0, r.len())?;
    if &ftyp.typ != b"ftyp" || field(r, ftyp, 0, 4)? != *b"crx " {
        return Err(Error::corrupt("cr3: not a crx ftyp"));
    }
    let mut at = ftyp.end;
    for _ in 0..MAX_CHILDREN {
        let b = read_box(r, at, r.len())?;
        if &b.typ == b"moov" {
            return Ok(b);
        }
        at = b.end;
    }
    Err(Error::corrupt("cr3: no moov"))
}

pub(super) fn locate(r: &Reader) -> Result<(Span, Orientation)> {
    let moov = moov(r)?;
    let kids = children(r, moov.body, moov.end)?;
    let traks: Vec<Bx> = kids.iter().copied().filter(|b| &b.typ == b"trak").collect();
    let canon = canon_box(r, &kids)?;
    let (trak_no, orientation) = match canon {
        Some(c) => (preview_trak(r, c).unwrap_or(None), cmt1_orientation(r, c)),
        None => (None, Orientation::Normal),
    };
    // Without a CCDT record, fall back to trak 1 as rawler does: every
    // known CR3 stores PreviewBig there.
    let idx = trak_no.map_or(0, |n| n.saturating_sub(1));
    let trak = traks
        .get(idx)
        .copied()
        .ok_or_else(|| Error::corrupt("cr3: preview trak missing"))?;
    Ok((first_sample(r, trak)?, orientation))
}

fn canon_box(r: &Reader, kids: &[Bx]) -> Result<Option<Bx>> {
    for b in kids.iter().filter(|b| &b.typ == b"uuid") {
        if b.end >= b.body + 16 && field(r, *b, 0, 16)? == CANON_UUID {
            return Ok(Some(Bx {
                typ: b.typ,
                body: b.body + 16,
                end: b.end,
            }));
        }
    }
    Ok(None)
}

/// 1-based trak number of the PreviewBig image, from CCTP/CCDT.
fn preview_trak(r: &Reader, canon: Bx) -> Result<Option<usize>> {
    let cctp = find(r, canon, b"CCTP")?;
    // CCTP body: version/flags, unknown u32, line count u32, then CCDTs.
    for ccdt in children(r, cctp.body + 12, cctp.end)? {
        if &ccdt.typ != b"CCDT" {
            continue;
        }
        let b = field(r, ccdt, 0, 16)?;
        if BE.u64(&b, 0) == PREVIEW_BIG {
            return Ok(usize::try_from(BE.u32(&b, 12)).ok());
        }
    }
    Ok(None)
}

/// CMT1 is a complete little TIFF whose IFD0 mirrors the camera's IFD0.
fn cmt1_orientation(r: &Reader, canon: Bx) -> Orientation {
    let read = || -> Result<Orientation> {
        let cmt1 = find(r, canon, b"CMT1")?;
        let (t, ifd0) = Tiff::open(r, cmt1.body)?;
        Ok(t.orientation(&t.ifd(ifd0)?))
    };
    read().unwrap_or(Orientation::Normal)
}

fn first_sample(r: &Reader, trak: Bx) -> Result<Span> {
    let mdia = find(r, trak, b"mdia")?;
    let minf = find(r, mdia, b"minf")?;
    let stbl = find(r, minf, b"stbl")?;
    let kids = children(r, stbl.body, stbl.end)?;
    let get = |t: &[u8; 4]| kids.iter().find(|b| &b.typ == t).copied();
    let stsz = get(b"stsz").ok_or_else(|| Error::corrupt("cr3: no stsz"))?;
    // stsz body: version/flags, sample_size, sample_count, [sizes..].
    let head = field(r, stsz, 0, 12)?;
    let (fixed, count) = (BE.u32(&head, 4), BE.u32(&head, 8));
    if count == 0 {
        return Err(Error::corrupt("cr3: preview trak has no samples"));
    }
    let len = if fixed != 0 {
        fixed
    } else {
        BE.u32(&field(r, stsz, 12, 4)?, 0)
    };
    // co64/stco body: version/flags, entry_count, offsets.
    let off = if let Some(co64) = get(b"co64") {
        BE.u64(&field(r, co64, 8, 8)?, 0)
    } else if let Some(stco) = get(b"stco") {
        u64::from(BE.u32(&field(r, stco, 8, 4)?, 0))
    } else {
        return Err(Error::corrupt("cr3: no chunk offsets"));
    };
    r.span(off, u64::from(len))
}
