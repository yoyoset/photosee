//! Bounds-checked, budgeted range reads with a tiny block cache.
//!
//! Why a cache: TIFF/BMFF parsing issues many 2..12 byte reads that sit close
//! together. On SMB every `read_at` is a network round trip, so we fetch a
//! block and serve neighbours from memory. Why a budget: a hostile file can
//! chain many small structures; the budget turns that into `Corrupt` instead
//! of megabytes of IO.

use crate::error::{Error, Result};
use crate::io::{ReadAt, read_exact_at};
use std::cell::{Cell, RefCell};

/// Bytes fetched per cache miss (clamped to the file end).
const BLOCK: usize = 16 * 1024;
/// Cached blocks; IFD0, a sub-structure and the JPEG head fit at once.
const SLOTS: usize = 4;
/// Largest single request. The biggest legitimate one is a JPEG APP1
/// segment (< 64 KiB); IFDs are capped at 1024 entries (12 KiB).
const MAX_REQUEST: usize = 128 * 1024;
/// Total bytes we are willing to fetch for headers of one file.
const BUDGET: u64 = 1024 * 1024;

/// A validated byte range inside the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Span {
    pub off: u64,
    pub len: u64,
}

pub(super) struct Reader<'a> {
    src: &'a dyn ReadAt,
    len: u64,
    blocks: RefCell<Vec<(u64, Vec<u8>)>>,
    spent: Cell<u64>,
}

impl<'a> Reader<'a> {
    pub fn new(src: &'a dyn ReadAt) -> Result<Self> {
        Ok(Self {
            src,
            len: src.len()?,
            blocks: RefCell::new(Vec::new()),
            spent: Cell::new(0),
        })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    /// A non-empty range that lies entirely inside the file.
    pub fn span(&self, off: u64, len: u64) -> Result<Span> {
        let end = off
            .checked_add(len)
            .ok_or_else(|| Error::corrupt("raw: range overflows"))?;
        if len == 0 || end > self.len {
            return Err(Error::corrupt(format!(
                "raw: range {off}+{len} outside file of {} bytes",
                self.len
            )));
        }
        Ok(Span { off, len })
    }

    /// Exactly `n` bytes at `off`. Fails before allocating if the range is
    /// outside the file, so a hostile length never allocates past `len`.
    pub fn bytes(&self, off: u64, n: usize) -> Result<Vec<u8>> {
        if n == 0 {
            return Ok(Vec::new());
        }
        if n > MAX_REQUEST {
            return Err(Error::corrupt(format!("raw: header read of {n} bytes")));
        }
        self.span(off, n as u64)?;
        if let Some(hit) = self.cached(off, n) {
            return Ok(hit);
        }
        // `span` proved off+n <= len, so `want >= n` and fits in usize.
        let want =
            usize::try_from(self.len - off).map_or(BLOCK.max(n), |rest| rest.min(BLOCK).max(n));
        let block = self.fetch(off, want)?;
        let out = block[..n].to_vec();
        if want <= BLOCK {
            let mut blocks = self.blocks.borrow_mut();
            if blocks.len() == SLOTS {
                blocks.remove(0);
            }
            blocks.push((off, block));
        }
        Ok(out)
    }

    fn cached(&self, off: u64, n: usize) -> Option<Vec<u8>> {
        let blocks = self.blocks.borrow();
        blocks.iter().find_map(|(start, data)| {
            let rel = usize::try_from(off.checked_sub(*start)?).ok()?;
            data.get(rel..rel.checked_add(n)?).map(<[u8]>::to_vec)
        })
    }

    fn fetch(&self, off: u64, n: usize) -> Result<Vec<u8>> {
        let spent = self.spent.get() + n as u64;
        if spent > BUDGET {
            return Err(Error::corrupt("raw: header structures exceed read budget"));
        }
        self.spent.set(spent);
        read_exact_at(self.src, off, n)
    }

    pub fn u16(&self, off: u64, e: Endian) -> Result<u16> {
        let b = self.bytes(off, 2)?;
        Ok(e.u16(&b, 0))
    }

    pub fn u32(&self, off: u64, e: Endian) -> Result<u32> {
        let b = self.bytes(off, 4)?;
        Ok(e.u32(&b, 0))
    }

    pub fn u64(&self, off: u64, e: Endian) -> Result<u64> {
        let b = self.bytes(off, 8)?;
        Ok(e.u64(&b, 0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Endian {
    Little,
    Big,
}

impl Endian {
    /// Reads at `at`; bytes past the slice end read as zero, so callers
    /// that sized the slice themselves never panic on a logic slip.
    fn arr<const N: usize>(b: &[u8], at: usize) -> [u8; N] {
        let mut a = [0u8; N];
        for (i, v) in a.iter_mut().enumerate() {
            *v = at
                .checked_add(i)
                .and_then(|j| b.get(j))
                .copied()
                .unwrap_or(0);
        }
        a
    }

    pub fn u16(self, b: &[u8], at: usize) -> u16 {
        let a = Self::arr::<2>(b, at);
        match self {
            Self::Little => u16::from_le_bytes(a),
            Self::Big => u16::from_be_bytes(a),
        }
    }

    pub fn u32(self, b: &[u8], at: usize) -> u32 {
        let a = Self::arr::<4>(b, at);
        match self {
            Self::Little => u32::from_le_bytes(a),
            Self::Big => u32::from_be_bytes(a),
        }
    }

    pub fn u64(self, b: &[u8], at: usize) -> u64 {
        let a = Self::arr::<8>(b, at);
        match self {
            Self::Little => u64::from_le_bytes(a),
            Self::Big => u64::from_be_bytes(a),
        }
    }
}
