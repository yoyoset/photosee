//! Hand-written EXIF APP1 segment carrying only the Orientation tag.
//!
//! Layout: FF E1, length, "Exif\0\0", then a little-endian TIFF block with one
//! IFD0 entry (tag 0x0112, SHORT, count 1) and no next IFD.

const SOI: [u8; 2] = [0xFF, 0xD8];

/// Bytes of the whole APP1 segment (marker included) for orientation `n`.
pub fn orientation_segment(n: u16) -> Vec<u8> {
    let mut tiff = Vec::with_capacity(26);
    tiff.extend_from_slice(b"II*\0");
    tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 sits right after the header
    tiff.extend_from_slice(&1u16.to_le_bytes()); // one entry
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes()); // type SHORT
    tiff.extend_from_slice(&1u32.to_le_bytes()); // count

    // A SHORT value is left-justified in the 4-byte value field.
    tiff.extend_from_slice(&n.to_le_bytes());
    tiff.extend_from_slice(&[0, 0]);
    tiff.extend_from_slice(&0u32.to_le_bytes()); // next IFD: none

    let mut seg = vec![0xFF, 0xE1];
    // Length counts itself (2) + "Exif\0\0" (6) + TIFF block, not the marker.
    let len = (2 + 6 + tiff.len()) as u16;
    seg.extend_from_slice(&len.to_be_bytes());
    seg.extend_from_slice(b"Exif\0\0");
    seg.extend_from_slice(&tiff);
    seg
}

/// Insert the orientation segment right after SOI. Returns `None` when the
/// input does not start with SOI (a generator bug, not hostile input).
pub fn insert_orientation(jpeg: &[u8], n: u16) -> Option<Vec<u8>> {
    if !jpeg.starts_with(&SOI) {
        return None;
    }
    let seg = orientation_segment(n);
    let mut out = Vec::with_capacity(jpeg.len() + seg.len());
    out.extend_from_slice(&SOI);
    out.extend_from_slice(&seg);
    out.extend_from_slice(&jpeg[2..]);
    Some(out)
}

/// Test-side parser: first APP1/Exif segment -> orientation value.
#[cfg(test)]
pub fn parse_orientation(jpeg: &[u8]) -> Option<u16> {
    let seg = jpeg.get(2..)?;
    if seg.get(..2)? != [0xFF, 0xE1] {
        return None;
    }
    let len = u16::from_be_bytes([*seg.get(2)?, *seg.get(3)?]) as usize;
    let body = seg.get(4..2 + len)?;
    let tiff = body.strip_prefix(b"Exif\0\0")?;
    if tiff.get(..4)? != b"II*\0" {
        return None;
    }
    let ifd = u32::from_le_bytes(tiff.get(4..8)?.try_into().ok()?) as usize;
    let count = u16::from_le_bytes(tiff.get(ifd..ifd + 2)?.try_into().ok()?) as usize;
    (0..count).find_map(|i| {
        let e = tiff.get(ifd + 2 + i * 12..ifd + 14 + i * 12)?;
        let tag = u16::from_le_bytes([e[0], e[1]]);
        (tag == 0x0112).then(|| u16::from_le_bytes([e[8], e[9]]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_length_field_excludes_marker() {
        let seg = orientation_segment(6);
        assert_eq!(seg.len(), 36);
        assert_eq!(u16::from_be_bytes([seg[2], seg[3]]) as usize, seg.len() - 2);
    }

    #[test]
    fn round_trips_every_orientation() {
        for n in 1..=8u16 {
            let jpeg = insert_orientation(&[0xFF, 0xD8, 0xFF, 0xD9], n).unwrap();
            assert_eq!(parse_orientation(&jpeg), Some(n));
            assert!(jpeg.ends_with(&[0xFF, 0xD9]));
        }
    }

    #[test]
    fn rejects_non_jpeg() {
        assert!(insert_orientation(b"nope", 1).is_none());
    }
}
