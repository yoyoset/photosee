//! Minimal ZIP reader for comic archives (.cbz): central directory, stored
//! and deflated members. Every byte is hostile (AGENTS.md rule 4): offsets
//! are bounds-checked against the archive length and allocations are capped
//! before they happen.
//!
//! Measured on the user's library (2026-11-05, 32 .cbz on $LAN_HOST): both
//! stored (method 0, the 400–900 MB colour volumes) and deflated (method 8)
//! archives are common; none used ZIP64 or non-ASCII names. ZIP64 is
//! refused with `unsupported` rather than guessed at.

use crate::error::{Error, Result};
use crate::io::{ReadAt, read_exact_at};

const EOCD_SIG: u32 = 0x0605_4b50;
const CDIR_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const EOCD_LEN: u64 = 22;
const LOCAL_LEN: u64 = 30;
const CDIR_LEN: usize = 46;
/// A 300-page volume has a ~30 KB directory; this is generous headroom.
const MAX_CDIR_BYTES: u64 = 64 << 20;
/// A page is a few MB; anything this large is a zip bomb or not a comic.
pub const MAX_MEMBER_BYTES: u64 = 256 << 20;

pub const STORED: u16 = 0;
pub const DEFLATED: u16 = 8;

/// One member as the central directory describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Raw name bytes decoded as UTF-8 (lossy; see `decode_name`).
    pub name: String,
    pub method: u16,
    pub encrypted: bool,
    pub comp_size: u64,
    pub size: u64,
    pub local_offset: u64,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Every member in central-directory order.
pub fn members(src: &dyn ReadAt) -> Result<Vec<Member>> {
    let len = src.len()?;
    let (cd_offset, cd_size, count) = find_eocd(src, len)?;
    let cd = read_exact_at(
        src,
        cd_offset,
        usize::try_from(cd_size).unwrap_or(usize::MAX),
    )?;
    let mut out = Vec::with_capacity(usize::from(count));
    let mut at = 0usize;
    for _ in 0..count {
        let (m, next) = parse_cdir_record(&cd, at)?;
        out.push(m);
        at = next;
    }
    Ok(out)
}

/// Finds the End Of Central Directory record by scanning backwards over the
/// optional comment (at most 65535 bytes).
fn find_eocd(src: &dyn ReadAt, len: u64) -> Result<(u64, u64, u16)> {
    if len < EOCD_LEN {
        return Err(Error::corrupt("zip: shorter than an end record"));
    }
    let tail_len = len.min(EOCD_LEN + 0xFFFF);
    let tail_start = len - tail_len;
    // tail_len <= 65557, so the cast cannot truncate.
    let tail = read_exact_at(src, tail_start, tail_len as usize)?;
    let pos = (0..=tail.len() - EOCD_LEN as usize)
        .rev()
        .find(|&i| u32_at(&tail, i) == EOCD_SIG)
        .ok_or_else(|| Error::corrupt("zip: no end of central directory"))?;
    let e = &tail[pos..];
    let count = u16_at(e, 10);
    let cd_size = u32_at(e, 12);
    let cd_offset = u32_at(e, 16);
    if count == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF {
        return Err(Error::unsupported("zip64 archives are not supported"));
    }
    if u16_at(e, 4) != 0 || u16_at(e, 6) != 0 {
        return Err(Error::unsupported(
            "multi-part zip archives are not supported",
        ));
    }
    let (cd_offset, cd_size) = (u64::from(cd_offset), u64::from(cd_size));
    let eocd_at = tail_start + pos as u64;
    if cd_size > MAX_CDIR_BYTES || cd_offset + cd_size > eocd_at {
        return Err(Error::corrupt("zip: central directory out of bounds"));
    }
    Ok((cd_offset, cd_size, count))
}

fn parse_cdir_record(cd: &[u8], at: usize) -> Result<(Member, usize)> {
    let short = || Error::corrupt("zip: central directory truncated");
    let fixed = cd.get(at..at + CDIR_LEN).ok_or_else(short)?;
    if u32_at(fixed, 0) != CDIR_SIG {
        return Err(Error::corrupt("zip: bad central directory signature"));
    }
    let name_len = usize::from(u16_at(fixed, 28));
    let extra_len = usize::from(u16_at(fixed, 30));
    let comment_len = usize::from(u16_at(fixed, 32));
    let name_start = at + CDIR_LEN;
    let raw_name = cd
        .get(name_start..name_start + name_len)
        .ok_or_else(short)?;
    let next = name_start + name_len + extra_len + comment_len;
    if next > cd.len() {
        return Err(short());
    }
    let (comp, size, local) = (u32_at(fixed, 20), u32_at(fixed, 24), u32_at(fixed, 42));
    if [comp, size, local].contains(&0xFFFF_FFFF) {
        return Err(Error::unsupported("zip64 archives are not supported"));
    }
    let member = Member {
        name: decode_name(raw_name),
        method: u16_at(fixed, 10),
        encrypted: u16_at(fixed, 8) & 1 != 0,
        comp_size: u64::from(comp),
        size: u64::from(size),
        local_offset: u64::from(local),
    };
    Ok((member, next))
}

/// Names are UTF-8 when general-purpose flag bit 11 is set, and in practice
/// also when it is not (every sample on 13 was ASCII without the flag).
/// Legacy code-page names (GBK, Shift-JIS) come out lossy: there is no
/// decoder for them without a new dependency. Known limitation, see
/// docs/non-goals.md.
fn decode_name(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}

/// Bytes of one member, inflated if needed. Stored members of any size are
/// returned as a window onto the archive by the caller instead; this reads
/// into memory and therefore enforces `MAX_MEMBER_BYTES`.
pub fn read_member(src: &dyn ReadAt, m: &Member) -> Result<Vec<u8>> {
    if m.encrypted {
        return Err(Error::unsupported("encrypted zip member"));
    }
    if m.size > MAX_MEMBER_BYTES || m.comp_size > MAX_MEMBER_BYTES {
        return Err(Error::new(
            crate::error::ErrorCode::TooLarge,
            format!("zip member is {} bytes", m.size),
        ));
    }
    let start = data_start(src, m)?;
    // Both sizes were capped above, so they fit in usize.
    let packed = read_exact_at(src, start, m.comp_size as usize)?;
    match m.method {
        STORED => Ok(packed),
        DEFLATED => miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, m.size as usize)
            .map_err(|e| Error::corrupt(format!("zip: inflate failed: {e:?}"))),
        other => Err(Error::unsupported(format!(
            "zip compression method {other}"
        ))),
    }
}

/// Offset of a member's data. The local header repeats the name and has its
/// own extra field, whose length may differ from the central one.
pub fn data_start(src: &dyn ReadAt, m: &Member) -> Result<u64> {
    let h = read_exact_at(src, m.local_offset, LOCAL_LEN as usize)?;
    if u32_at(&h, 0) != LOCAL_SIG {
        return Err(Error::corrupt("zip: bad local header signature"));
    }
    let start = m.local_offset + LOCAL_LEN + u64::from(u16_at(&h, 26)) + u64::from(u16_at(&h, 28));
    if start + m.comp_size > src.len()? {
        return Err(Error::corrupt("zip: member runs past end of archive"));
    }
    Ok(start)
}
