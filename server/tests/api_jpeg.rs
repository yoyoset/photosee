//! Hand-built JPEG skeletons for the api_* tests (included with `#[path]`).
//! They carry real markers and EXIF but no decodable scan: `/meta` and
//! passthrough read only the headers, which is what these tests exercise.
#![allow(dead_code, reason = "each api_* test uses a different subset")]

type Entry = (u16, u16, u32, Vec<u8>);

fn ascii(s: &str) -> Entry2 {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    (2, v.len() as u32, v)
}

type Entry2 = (u16, u32, Vec<u8>);

fn short(v: u16) -> Entry2 {
    (3, 1, v.to_le_bytes().to_vec())
}

fn long(v: u32) -> Entry2 {
    (4, 1, v.to_le_bytes().to_vec())
}

fn rational(num: u32, den: u32) -> Entry2 {
    let mut v = num.to_le_bytes().to_vec();
    v.extend_from_slice(&den.to_le_bytes());
    (5, 1, v)
}

fn entry(tag: u16, (typ, count, value): Entry2) -> Entry {
    (tag, typ, count, value)
}

/// One little-endian IFD. Values over 4 bytes go to `data`, addressed from
/// `data_base`.
fn ifd(entries: &[Entry], data: &mut Vec<u8>, data_base: u32) -> Vec<u8> {
    let mut out = (entries.len() as u16).to_le_bytes().to_vec();
    for (tag, typ, count, value) in entries {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&typ.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if value.len() <= 4 {
            out.extend_from_slice(value);
            out.resize(out.len() + 4 - value.len(), 0);
        } else {
            out.extend_from_slice(&(data_base + data.len() as u32).to_le_bytes());
            data.extend_from_slice(value);
            if data.len() % 2 == 1 {
                data.push(0);
            }
        }
    }
    out.extend_from_slice(&[0; 4]); // no next IFD
    out
}

/// The EXIF of `contracts/meta.json`'s camera, with the given orientation.
fn exif_tiff(orientation: u16) -> Vec<u8> {
    let ifd_len = |n: usize| (2 + 12 * n + 4) as u32;
    let (n0, n1) = (4, 6);
    let exif_at = 8 + ifd_len(n0);
    let data_base = exif_at + ifd_len(n1);
    let mut data = Vec::new();
    let ifd0 = ifd(
        &[
            entry(0x010F, ascii("Canon")),
            entry(0x0110, ascii("Canon EOS R6")),
            entry(0x0112, short(orientation)),
            entry(0x8769, long(exif_at)),
        ],
        &mut data,
        data_base,
    );
    let ifd1 = ifd(
        &[
            entry(0x829A, rational(1, 250)),
            entry(0x829D, rational(4, 1)),
            entry(0x8827, short(400)),
            entry(0x9003, ascii("2024:05:01 13:22:10")),
            entry(0x920A, rational(50, 1)),
            entry(0xA434, ascii("RF24-105mm F4 L IS USM")),
        ],
        &mut data,
        data_base,
    );
    let mut t = b"II*\0".to_vec();
    t.extend_from_slice(&8u32.to_le_bytes());
    t.extend_from_slice(&ifd0);
    t.extend_from_slice(&ifd1);
    t.extend_from_slice(&data);
    t
}

fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut s = vec![0xFF, marker];
    s.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    s.extend_from_slice(payload);
    s
}

/// A JPEG that claims to be `w`x`h` (stored, before orientation), carries the
/// camera EXIF with `orientation`, and `padding` bytes of comment so tests
/// have a body to slice. Not decodable: there is no scan.
pub fn skeleton(w: u16, h: u16, orientation: u16, padding: usize) -> Vec<u8> {
    let mut exif = b"Exif\0\0".to_vec();
    exif.extend_from_slice(&exif_tiff(orientation));
    let mut sof = vec![8];
    sof.extend_from_slice(&h.to_be_bytes());
    sof.extend_from_slice(&w.to_be_bytes());
    sof.extend_from_slice(&[1, 1, 0x11, 0]);
    let comment: Vec<u8> = (0..padding).map(|i| (i % 251) as u8).collect();
    let mut out = vec![0xFF, 0xD8];
    out.extend(segment(0xE1, &exif));
    out.extend(segment(0xFE, &comment));
    out.extend(segment(0xC0, &sof));
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}
