//! Tiny ZIP writer for the archive_* tests (included with `#[path]`). Also
//! compiled as its own empty test target, hence the allow. CRCs are left at
//! zero: the reader does not check them (decoders validate the images).
#![allow(dead_code)]

pub struct Member<'a> {
    pub name: &'a str,
    pub data: &'a [u8],
    pub deflate: bool,
}

pub fn stored<'a>(name: &'a str, data: &'a [u8]) -> Member<'a> {
    Member {
        name,
        data,
        deflate: false,
    }
}

pub fn deflated<'a>(name: &'a str, data: &'a [u8]) -> Member<'a> {
    Member {
        name,
        data,
        deflate: true,
    }
}

fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Local header + data for one member; returns its central record.
fn write_member(out: &mut Vec<u8>, m: &Member<'_>) -> Vec<u8> {
    let packed = if m.deflate {
        miniz_oxide::deflate::compress_to_vec(m.data, 6)
    } else {
        m.data.to_vec()
    };
    let method: u16 = if m.deflate { 8 } else { 0 };
    let offset = out.len() as u32;
    let name = m.name.as_bytes();
    let fields = |v: &mut Vec<u8>| {
        put16(v, 20); // version needed
        put16(v, 0); // flags
        put16(v, method);
        put32(v, 0); // dos time + date
        put32(v, 0); // crc
        put32(v, packed.len() as u32);
        put32(v, m.data.len() as u32);
        put16(v, name.len() as u16);
    };
    put32(out, 0x0403_4b50);
    fields(out);
    // A local extra field the central directory does not have: real
    // archivers do this, and the reader must use the local length.
    put16(out, 4);
    out.extend_from_slice(name);
    out.extend_from_slice(&[0xAA, 0xBB, 0, 0]);
    out.extend_from_slice(&packed);
    let mut c = Vec::new();
    put32(&mut c, 0x0201_4b50);
    put16(&mut c, 20); // version made by
    fields(&mut c);
    put16(&mut c, 0); // extra
    put16(&mut c, 0); // comment
    put16(&mut c, 0); // disk
    put16(&mut c, 0); // internal attrs
    put32(&mut c, 0); // external attrs
    put32(&mut c, offset);
    c.extend_from_slice(name);
    c
}

pub fn zip(members: &[Member<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    let cds: Vec<Vec<u8>> = members.iter().map(|m| write_member(&mut out, m)).collect();
    let cd_offset = out.len() as u32;
    for c in &cds {
        out.extend_from_slice(c);
    }
    let cd_size = out.len() as u32 - cd_offset;
    put32(&mut out, 0x0605_4b50);
    put16(&mut out, 0);
    put16(&mut out, 0);
    put16(&mut out, members.len() as u16);
    put16(&mut out, members.len() as u16);
    put32(&mut out, cd_size);
    put32(&mut out, cd_offset);
    let comment = b"archive comment";
    put16(&mut out, comment.len() as u16);
    out.extend_from_slice(comment);
    out
}

/// A small real PNG so decode and thumbnail paths can run end to end.
pub fn png(w: u32, h: u32) -> Vec<u8> {
    use image::ImageEncoder;
    let px: Vec<u8> = (0..w * h)
        .flat_map(|i| [(i % 256) as u8, 80, 160])
        .collect();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&px, w, h, image::ExtendedColorType::Rgb8)
        .expect("png encode");
    out
}
