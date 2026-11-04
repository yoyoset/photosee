//! EXIF orientation. Package C. Invariant: orientation is applied exactly
//! once — by the webview for passthrough, by core for transcodes (and the
//! transcoded output carries no Orientation tag).

use crate::error::Result;
use crate::io::{ReadAt, read_exact_at};
use crate::types::{Format, Orientation, Rgba8, Size};
use image::ImageDecoder;

/// JPEG APP1 and PNG eXIf sit near the start; this bounds the read.
const EXIF_HEAD: u64 = 256 * 1024;
/// WebP puts EXIF after the image data, so chunks are walked by range reads.
/// An animated file can have a chunk per frame before it; give up after this.
const WEBP_MAX_CHUNKS: usize = 64;
const WEBP_MAX_EXIF: u32 = 1 << 20;

/// Orientation stored in the file's metadata (`Normal` if none). HEIC returns
/// `Normal`: its irot/imir transforms are applied by the HEIC decoder.
///
/// Damaged metadata yields `Normal`, never an error: it must not fail a
/// photo. Only IO errors from reading the head propagate.
pub fn read(src: &dyn ReadAt, fmt: Format) -> Result<Orientation> {
    let tag = match fmt {
        Format::Jpeg | Format::Png => {
            let len = src.len()?.min(EXIF_HEAD) as usize;
            let head = read_exact_at(src, 0, len)?;
            exif::Reader::new()
                .read_from_container(&mut std::io::Cursor::new(head))
                .ok()
                .and_then(|e| orientation_field(&e))
        }
        Format::Webp => webp_exif(src)?.and_then(|raw| {
            let raw = raw
                .strip_prefix(b"Exif\0\0")
                .map(<[u8]>::to_vec)
                .unwrap_or(raw);
            exif::Reader::new()
                .read_raw(raw)
                .ok()
                .and_then(|e| orientation_field(&e))
        }),
        // TIFF writers (libtiff among them) often put IFD0 after the pixel
        // data, past any head we would read; kamadak-exif needs the whole
        // file then. The TIFF decoder seeks straight to IFD0 instead.
        Format::Tiff => crate::decode::std_formats::decoder(src, Format::Tiff)
            .ok()
            .and_then(|mut d| d.orientation().ok())
            .map(|o| u16::from(o.to_exif())),
        _ => None,
    };
    Ok(tag.map_or(Orientation::Normal, Orientation::from_exif))
}

fn orientation_field(e: &exif::Exif) -> Option<u16> {
    let f = e.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    f.value.get_uint(0).and_then(|v| u16::try_from(v).ok())
}

/// Payload of the first `EXIF` RIFF chunk, reading only chunk headers.
fn webp_exif(src: &dyn ReadAt) -> Result<Option<Vec<u8>>> {
    let len = src.len()?;
    let mut pos = 12u64;
    for _ in 0..WEBP_MAX_CHUNKS {
        if pos + 8 > len {
            break;
        }
        let h = read_exact_at(src, pos, 8)?;
        let size = u32::from_le_bytes([h[4], h[5], h[6], h[7]]);
        if &h[..4] == b"EXIF" {
            if size > WEBP_MAX_EXIF || pos + 8 + u64::from(size) > len {
                return Ok(None);
            }
            return Ok(Some(read_exact_at(src, pos + 8, size as usize)?));
        }
        // Chunks are padded to an even size.
        pos += 8 + u64::from(size) + u64::from(size & 1);
    }
    Ok(None)
}

pub fn apply(img: Rgba8, o: Orientation) -> Rgba8 {
    use Orientation as O;
    let Size { w, h } = img.size;
    let (w, h) = (w as usize, h as usize);
    if o == O::Normal || img.data.len() != w * h * 4 {
        // A size/data mismatch is an internal bug; leave the pixels alone
        // rather than index out of bounds.
        return img;
    }
    let out_size = if o.swaps_axes() {
        Size::new(img.size.h, img.size.w)
    } else {
        img.size
    };
    let dw = out_size.w as usize;
    // Destination (x, y) of source (sx, sy); `dw` is the output width.
    let map = |sx: usize, sy: usize| -> (usize, usize) {
        match o {
            O::Normal => (sx, sy),
            O::FlipH => (w - 1 - sx, sy),
            O::Rotate180 => (w - 1 - sx, h - 1 - sy),
            O::FlipV => (sx, h - 1 - sy),
            O::Transpose => (sy, sx),
            O::Rotate90 => (h - 1 - sy, sx),
            O::Transverse => (h - 1 - sy, w - 1 - sx),
            O::Rotate270 => (sy, w - 1 - sx),
        }
    };
    let mut out = vec![0u8; img.data.len()];
    for (sy, row) in img.data.chunks_exact(w * 4).enumerate() {
        for (sx, px) in row.chunks_exact(4).enumerate() {
            let (dx, dy) = map(sx, sy);
            let d = (dy * dw + dx) * 4;
            out[d..d + 4].copy_from_slice(px);
        }
    }
    Rgba8 {
        size: out_size,
        data: out,
    }
}
