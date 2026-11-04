//! PSD merged (composite) image. PSB and ZIP-compressed PSD are non-goals and
//! return Unsupported. Package D.
//!
//! This does NOT use zune-psd 0.5.1, which the brief names. Measured on that
//! version (see QUESTIONS.md): uncompressed 8-bit RGB/RGBA comes back as
//! garbage with `Ok` (the channel loop stops at `pixel_count / channels`),
//! 16-bit samples are byte-swapped on little-endian, 16-bit RLE is "Not
//! implemented", 5+ channels (RGB + alpha channels) are refused, and the 4th
//! channel is always treated as alpha. The merged-image section is simple
//! enough (header, three skipped sections, planar raw or PackBits data) that
//! reading it directly is shorter than working around those.
//!
//! Known gaps: the embedded ICC profile (image resource 1039) is ignored, so
//! Display P3 / AdobeRGB files look slightly desaturated; layers are never
//! composited (only the stored merged image is shown); 1-bit, 32-bit, Lab,
//! CMYK, indexed, duotone and multichannel files are Unsupported.
//!
//! Alpha: the merged data's extra channel is transparency only when the
//! layer-info count is negative (spec: "first alpha channel contains the
//! transparency data for the merged result"); otherwise extra channels are
//! saved selections and are ignored. Photoshop stores that transparency
//! pre-blended with white, which is undone below.

use crate::error::{Error, ErrorCode, Result};
use crate::io::{ReadAt, read_exact_at};
use crate::types::{Format, ImageInfo, Orientation, Rgba8, Size};

use super::std_formats::MAX_PIXELS;

const HEADER_LEN: usize = 26;
/// The format's own limit for PSD (PSB goes to 300000).
const MAX_DIM: u32 = 30_000;

struct Header {
    channels: usize,
    w: u32,
    h: u32,
    depth: u16,
    mode: u16,
}

fn be16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}
fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn parse_header(src: &dyn ReadAt) -> Result<Header> {
    let b = read_exact_at(src, 0, HEADER_LEN)?;
    if &b[..4] != b"8BPS" {
        return Err(Error::corrupt("psd: bad signature"));
    }
    match be16(&b[4..6]) {
        1 => {}
        2 => return Err(Error::unsupported("psd: PSB (version 2) files")),
        v => return Err(Error::corrupt(format!("psd: unknown version {v}"))),
    }
    let hdr = Header {
        channels: usize::from(be16(&b[12..14])),
        h: be32(&b[14..18]),
        w: be32(&b[18..22]),
        depth: be16(&b[22..24]),
        mode: be16(&b[24..26]),
    };
    if !(1..=56).contains(&hdr.channels)
        || !(1..=MAX_DIM).contains(&hdr.w)
        || !(1..=MAX_DIM).contains(&hdr.h)
    {
        return Err(Error::corrupt("psd: channel count or size out of range"));
    }
    check_supported(&hdr)?;
    Ok(hdr)
}

/// Color channels the composite needs for a supported mode.
fn color_channels(mode: u16) -> usize {
    if mode == 1 { 1 } else { 3 }
}

fn check_supported(h: &Header) -> Result<()> {
    let name = match h.mode {
        1 | 3 => "",
        0 => "Bitmap",
        2 => "Indexed",
        4 => "CMYK",
        7 => "Multichannel",
        8 => "Duotone",
        9 => "Lab",
        m => return Err(Error::corrupt(format!("psd: unknown color mode {m}"))),
    };
    if !name.is_empty() {
        return Err(Error::unsupported(format!("psd: {name} color mode")));
    }
    if h.depth != 8 && h.depth != 16 {
        return Err(Error::unsupported(format!("psd: {}-bit depth", h.depth)));
    }
    if h.channels < color_channels(h.mode) {
        return Err(Error::corrupt(
            "psd: fewer channels than the color mode needs",
        ));
    }
    if Size::new(h.w, h.h).pixels() > MAX_PIXELS {
        return Err(Error::new(
            ErrorCode::TooLarge,
            "psd: exceeds the pixel cap",
        ));
    }
    Ok(())
}

pub fn info(src: &dyn ReadAt) -> Result<ImageInfo> {
    let h = parse_header(src)?;
    Ok(ImageInfo {
        format: Format::Psd,
        size: Size::new(h.w, h.h),
        orientation: Orientation::Normal,
        animated: false,
    })
}

struct Layout {
    /// Offset of the 2-byte compression field.
    data_off: u64,
    alpha: bool,
}

/// Walks the three length-prefixed sections after the header.
fn locate_image_data(src: &dyn ReadAt) -> Result<Layout> {
    let file_len = src.len()?;
    let mut off = HEADER_LEN as u64;
    let mut alpha = false;
    for section in 0..3 {
        let len = u64::from(be32(&read_exact_at(src, off, 4)?));
        if section == 2 && len >= 6 {
            // [layer info length: 4][layer count: i16]; negative = transparency.
            let b = read_exact_at(src, off + 4, 6)?;
            alpha = be32(&b[..4]) >= 2 && (be16(&b[4..6]) as i16) < 0;
        }
        off += 4 + len;
        if off + 2 > file_len {
            return Err(Error::corrupt("psd: sections run past the end of the file"));
        }
    }
    Ok(Layout {
        data_off: off,
        alpha,
    })
}

pub fn decode(src: &dyn ReadAt, max: Option<Size>) -> Result<Rgba8> {
    let _ = max; // no reduced decode; the caller resizes
    let h = parse_header(src)?;
    let layout = locate_image_data(src)?;
    let comp = be16(&read_exact_at(src, layout.data_off, 2)?);
    let base = layout.data_off + 2;
    let color = color_channels(h.mode);
    let used = if layout.alpha && h.channels > color {
        color + 1
    } else {
        color
    };
    let planes = match comp {
        0 => read_raw_planes(src, &h, base, used)?,
        1 => read_rle_planes(src, &h, base, used)?,
        2 | 3 => return Err(Error::unsupported("psd: ZIP-compressed image data")),
        c => return Err(Error::corrupt(format!("psd: unknown compression {c}"))),
    };
    let mut rgba = vec![255u8; (h.w as usize) * (h.h as usize) * 4];
    for (c, plane) in planes.iter().enumerate() {
        // The extra channel is alpha: it belongs in slot 3, not slot `color`.
        let slot = if c < color { c } else { 3 };
        scatter(&mut rgba, plane, slot, h.depth == 16);
    }
    if color == 1 {
        for px in rgba.chunks_exact_mut(4) {
            (px[1], px[2]) = (px[0], px[0]);
        }
    }
    if used > color {
        remove_white_matte(&mut rgba);
    }
    Ok(Rgba8 {
        size: Size::new(h.w, h.h),
        data: rgba,
    })
}

fn plane_bytes(h: &Header) -> usize {
    (h.w as usize) * (h.h as usize) * usize::from(h.depth / 8)
}

fn read_raw_planes(src: &dyn ReadAt, h: &Header, base: u64, used: usize) -> Result<Vec<Vec<u8>>> {
    let size = plane_bytes(h);
    // Check before allocating: a hostile header can claim 800 MB of planes.
    if base + (used * size) as u64 > src.len()? {
        return Err(Error::corrupt("psd: image data is truncated"));
    }
    (0..used)
        .map(|c| read_exact_at(src, base + (c * size) as u64, size))
        .collect()
}

/// Layout: `channels * height` u16 row lengths, then every row's PackBits data.
fn read_rle_planes(src: &dyn ReadAt, h: &Header, base: u64, used: usize) -> Result<Vec<Vec<u8>>> {
    let rows = h.h as usize;
    let table = read_exact_at(src, base, h.channels * rows * 2)?;
    let lens: Vec<usize> = table
        .chunks_exact(2)
        .map(|b| usize::from(be16(b)))
        .collect();
    let row_bytes = (h.w as usize) * usize::from(h.depth / 8);
    // Sane encoders expand a row by at most 1 byte per 128; allow the
    // sloppiest legal one (a header byte per literal byte) and no more.
    let max_row = 2 * row_bytes + 2;
    if lens.iter().any(|&l| l > max_row) {
        return Err(Error::corrupt("psd: RLE row length out of range"));
    }
    let mut off = base + table.len() as u64;
    let mut planes = Vec::with_capacity(used);
    for c in 0..used {
        let chan = &lens[c * rows..(c + 1) * rows];
        let total: usize = chan.iter().sum();
        if off + total as u64 > src.len()? {
            return Err(Error::corrupt("psd: image data is truncated"));
        }
        let packed = read_exact_at(src, off, total)?;
        off += packed.len() as u64;
        let mut plane = Vec::with_capacity(row_bytes * rows);
        let mut pos = 0;
        for &len in chan {
            unpack_row(&packed[pos..pos + len], row_bytes, &mut plane)?;
            pos += len;
        }
        planes.push(plane);
    }
    Ok(planes)
}

/// PackBits: n in 0..=127 copies n+1 literals; 129..=255 repeats the next
/// byte 257-n times; 128 is a no-op. A row must expand to exactly `want`.
fn unpack_row(row: &[u8], want: usize, out: &mut Vec<u8>) -> Result<()> {
    let start = out.len();
    let mut i = 0;
    while i < row.len() && out.len() - start < want {
        let n = usize::from(row[i]);
        i += 1;
        if n < 128 {
            let lit = row
                .get(i..i + n + 1)
                .ok_or_else(|| Error::corrupt("psd: RLE literal past row end"))?;
            out.extend_from_slice(lit);
            i += n + 1;
        } else if n > 128 {
            let b = *row
                .get(i)
                .ok_or_else(|| Error::corrupt("psd: RLE run past row end"))?;
            out.resize(out.len() + 257 - n, b);
            i += 1;
        }
    }
    if out.len() - start != want {
        return Err(Error::corrupt("psd: RLE row has the wrong length"));
    }
    Ok(())
}

/// Writes one planar channel into slot `slot` of the interleaved RGBA buffer,
/// reducing 16-bit samples (big-endian) to 8-bit with rounding.
fn scatter(rgba: &mut [u8], plane: &[u8], slot: usize, wide: bool) {
    if wide {
        for (px, s) in rgba.chunks_exact_mut(4).zip(plane.chunks_exact(2)) {
            let v = u32::from(be16(s));
            px[slot] = ((v * 255 + 32767) / 65535) as u8;
        }
    } else {
        for (px, &s) in rgba.chunks_exact_mut(4).zip(plane) {
            px[slot] = s;
        }
    }
}

/// c' = c*a + 255*(1-a)  =>  c = 255 - (255-c')*255/alpha. Verified against a
/// PNG export of the same Photoshop file (max difference 1 per sample).
fn remove_white_matte(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = u32::from(px[3]);
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                let lost = (255 - u32::from(*c)) * 255;
                *c = 255u32.saturating_sub((lost + a / 2) / a) as u8;
            }
        }
    }
}
