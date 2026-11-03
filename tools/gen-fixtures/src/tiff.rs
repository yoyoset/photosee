//! Hand-written little-endian TIFF that claims 100000x100000 pixels but carries
//! only 16 bytes of strip data. It exists to test the decoder's pixel-count
//! limit: a reader that trusts the header would try to allocate 30 GB.

const TYPE_SHORT: u16 = 3;
const TYPE_LONG: u16 = 4;
const ENTRY_COUNT: u16 = 9;
const HEADER_LEN: u32 = 8;
const IFD_LEN: u32 = 2 + ENTRY_COUNT as u32 * 12 + 4;
const CLAIMED_SIDE: u32 = 100_000;

fn entry(out: &mut Vec<u8>, tag: u16, ty: u16, value: u32) {
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    // SHORT values are left-justified in the 4-byte field; writing the u32
    // little-endian yields exactly that for values below 65536.
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn huge_header() -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN as usize + IFD_LEN as usize + 16);
    out.extend_from_slice(b"II*\0");
    out.extend_from_slice(&HEADER_LEN.to_le_bytes());
    out.extend_from_slice(&ENTRY_COUNT.to_le_bytes());
    // Tags must be in ascending order per the TIFF spec.
    entry(&mut out, 256, TYPE_LONG, CLAIMED_SIDE); // ImageWidth
    entry(&mut out, 257, TYPE_LONG, CLAIMED_SIDE); // ImageLength
    entry(&mut out, 258, TYPE_SHORT, 8); // BitsPerSample
    entry(&mut out, 259, TYPE_SHORT, 1); // Compression: none
    entry(&mut out, 262, TYPE_SHORT, 2); // Photometric: RGB
    entry(&mut out, 273, TYPE_LONG, HEADER_LEN + IFD_LEN); // StripOffsets
    entry(&mut out, 277, TYPE_SHORT, 3); // SamplesPerPixel
    entry(&mut out, 278, TYPE_SHORT, 1); // RowsPerStrip
    entry(&mut out, 279, TYPE_LONG, 16); // StripByteCounts
    out.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    out.extend_from_slice(&[0u8; 16]);
    out
}

/// Test-side parser: (width, length, strip offset) from IFD0.
#[cfg(test)]
pub fn parse_dims(tiff: &[u8]) -> Option<(u32, u32, u32)> {
    if tiff.get(..4)? != b"II*\0" {
        return None;
    }
    let ifd = u32::from_le_bytes(tiff.get(4..8)?.try_into().ok()?) as usize;
    let count = u16::from_le_bytes(tiff.get(ifd..ifd + 2)?.try_into().ok()?) as usize;
    let value = |want: u16| {
        (0..count).find_map(|i| {
            let e = tiff.get(ifd + 2 + i * 12..ifd + 14 + i * 12)?;
            (u16::from_le_bytes([e[0], e[1]]) == want)
                .then(|| u32::from_le_bytes([e[8], e[9], e[10], e[11]]))
        })
    };
    Some((value(256)?, value(257)?, value(273)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_claims_100000_square() {
        let t = huge_header();
        let (w, h, off) = parse_dims(&t).unwrap();
        assert_eq!((w, h), (100_000, 100_000));
        assert_eq!(off, 122);
    }

    #[test]
    fn strip_data_is_exactly_16_zero_bytes_at_the_offset() {
        let t = huge_header();
        let (_, _, off) = parse_dims(&t).unwrap();
        assert_eq!(t.len(), off as usize + 16);
        assert!(t[off as usize..].iter().all(|&b| b == 0));
    }
}
