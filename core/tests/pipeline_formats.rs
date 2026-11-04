//! std_formats info/decode per format, ICC, alpha, encode (package C).

#[path = "pipeline_util.rs"]
mod util;

use image::codecs::{bmp::BmpEncoder, gif::GifEncoder, ico::IcoEncoder};
use image::codecs::{tiff::TiffEncoder, webp::WebPEncoder};
use image::{ExtendedColorType, ImageEncoder};
use photosee_core::decode::{decode, info};
use photosee_core::{ErrorCode, Format, Orientation, Rgba8, Size};
use util::*;

fn encode_with(enc: impl ImageEncoder, img: &Rgba8) {
    enc.write_image(&img.data, img.size.w, img.size.h, ExtendedColorType::Rgba8)
        .expect("encode");
}

fn gif(frames: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let img = image::RgbaImage::from_fn(8, 6, |x, _| image::Rgba([x as u8 * 30, 0, 0, 255]));
    let frames = (0..frames).map(|_| image::Frame::new(img.clone()));
    GifEncoder::new(&mut out)
        .encode_frames(frames)
        .expect("gif");
    out
}

/// Lossless WebP's VP8L chunk (header included).
fn vp8l(img: &Rgba8) -> Vec<u8> {
    let mut out = Vec::new();
    encode_with(WebPEncoder::new_lossless(&mut out), img);
    out[12..].to_vec()
}

fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32 + 4).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);
    out
}

fn chunk(fourcc: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut c = fourcc.to_vec();
    c.extend_from_slice(&(body.len() as u32).to_le_bytes());
    c.extend_from_slice(body);
    if body.len() % 2 == 1 {
        c.push(0);
    }
    c
}

fn vp8x(flags: u8, w: u32, h: u32) -> Vec<u8> {
    let mut b = vec![flags, 0, 0, 0];
    b.extend_from_slice(&(w - 1).to_le_bytes()[..3]);
    b.extend_from_slice(&(h - 1).to_le_bytes()[..3]);
    chunk(b"VP8X", &b)
}

#[test]
fn png_with_alpha_decodes_and_reencodes_as_png() {
    let img = rgba(10, 4, |x, _| [200, 10, 10, (x * 25) as u8]);
    let out = decode(&png(&img, None), Format::Png, None).unwrap();
    assert_eq!(out, img);
    let enc = photosee_core::encode::encode(&out, 85).unwrap();
    assert_eq!(enc.mime, "image/png");
    assert_eq!(load(&enc.bytes).into_raw(), img.data);
}

#[test]
fn opaque_encodes_as_jpeg_without_exif() {
    let enc = photosee_core::encode::encode(&quadrants(32, 16), 85).unwrap();
    assert_eq!(enc.mime, "image/jpeg");
    assert!(enc.bytes.starts_with(&[0xFF, 0xD8]));
    assert!(!has_exif(&enc.bytes));
}

#[test]
fn gif_animation_is_detected_from_structure() {
    let one = info(&gif(1), Format::Gif).unwrap();
    assert_eq!((one.size, one.animated), (Size::new(8, 6), false));
    let two = info(&gif(2), Format::Gif).unwrap();
    assert!(two.animated);
    assert_eq!(
        decode(&gif(3), Format::Gif, None).unwrap().size,
        Size::new(8, 6)
    );
}

#[test]
fn webp_simple_extended_animated_and_exif() {
    let img = quadrants(8, 6);
    let simple = riff(&[vp8l(&img)]);
    let i = info(&simple, Format::Webp).unwrap();
    assert_eq!(
        (i.size, i.animated, i.orientation),
        (Size::new(8, 6), false, Orientation::Normal)
    );
    assert_eq!(decode(&simple, Format::Webp, None).unwrap(), img);

    // VP8X with an EXIF chunk after the image data (the spec's order).
    let tagged = riff(&[vp8x(0x08, 8, 6), vp8l(&img), chunk(b"EXIF", &exif_tiff(6))]);
    let i = info(&tagged, Format::Webp).unwrap();
    assert_eq!(
        (i.size, i.orientation),
        (Size::new(8, 6), Orientation::Rotate90)
    );
    assert_eq!(
        decode(&tagged, Format::Webp, None).unwrap().size,
        Size::new(6, 8)
    );

    let mut anmf = vec![0u8; 6];
    anmf.extend_from_slice(&7u32.to_le_bytes()[..3]); // width - 1
    anmf.extend_from_slice(&5u32.to_le_bytes()[..3]); // height - 1
    anmf.extend_from_slice(&[100, 0, 0, 0]); // duration, flags
    anmf.extend_from_slice(&vp8l(&img));
    let frames = [chunk(b"ANMF", &anmf), chunk(b"ANMF", &anmf)];
    let anim = chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]);
    let animated = riff(&[vp8x(0x02, 8, 6), anim, frames[0].clone(), frames[1].clone()]);
    let i = info(&animated, Format::Webp).unwrap();
    assert_eq!((i.size, i.animated), (Size::new(8, 6), true));
    assert_eq!(
        decode(&animated, Format::Webp, None).unwrap().size,
        Size::new(8, 6)
    );
}

#[test]
fn bmp_and_ico_decode() {
    let img = quadrants(16, 8);
    let mut bmp = Vec::new();
    encode_with(BmpEncoder::new(&mut bmp), &img);
    assert_eq!(info(&bmp, Format::Bmp).unwrap().size, Size::new(16, 8));
    assert_eq!(decode(&bmp, Format::Bmp, None).unwrap(), img);
    let mut ico = Vec::new();
    encode_with(IcoEncoder::new(&mut ico), &img);
    assert_eq!(info(&ico, Format::Ico).unwrap().size, Size::new(16, 8));
    assert_eq!(decode(&ico, Format::Ico, None).unwrap(), img);
}

#[test]
fn tiff_16bit_cmyk_and_orientation_after_pixels() {
    let px: Vec<u16> = vec![0xFFFF, 0x8080, 0, 0, 0xFFFF, 0x4040];
    let bytes: Vec<u8> = px.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let mut t16 = std::io::Cursor::new(Vec::new());
    TiffEncoder::new(&mut t16)
        .write_image(&bytes, 2, 1, ExtendedColorType::Rgb16)
        .expect("tiff16");
    let out = decode(&t16.into_inner(), Format::Tiff, None).unwrap();
    assert_eq!(out.data, vec![255, 128, 0, 255, 0, 255, 64, 255]);

    // CMYK (0, 255, 255, 0) is red.
    let cmyk = tiff(&[0, 255, 255, 0], &tiff_entries(1, 1, vec![8; 4], 5, 4));
    assert_eq!(
        decode(&cmyk, Format::Tiff, None).unwrap().data,
        RED.to_vec()
    );

    // IFD0 after the pixel data (as libtiff writes it), Orientation=6.
    let mut entries = tiff_entries(2, 1, vec![8], 1, 2);
    entries.insert(6, (274, 3, vec![6]));
    let rotated = tiff(&[10, 200], &entries);
    let i = info(&rotated, Format::Tiff).unwrap();
    assert_eq!(i.orientation, Orientation::Rotate90);
    let out = decode(&rotated, Format::Tiff, None).unwrap();
    assert_eq!(out.size, Size::new(1, 2));
    assert_eq!(out.data, vec![10, 10, 10, 255, 200, 200, 200, 255]);
}

#[test]
fn png_exif_orientation_is_applied() {
    let p = png_with_chunk(&png(&quadrants(8, 4), None), b"eXIf", &exif_tiff(3));
    assert_eq!(
        info(&p, Format::Png).unwrap().orientation,
        Orientation::Rotate180
    );
    let out = decode(&p, Format::Png, None).unwrap();
    assert_eq!(out.data[..4], WHITE); // bottom-right came to the top-left
}

#[test]
fn avif_is_unsupported() {
    let mut f = 24u32.to_be_bytes().to_vec();
    f.extend_from_slice(b"ftypavif\0\0\0\0mif1miaf");
    assert_eq!(photosee_core::format::sniff(&f, None), Some(Format::Avif));
    assert_eq!(
        info(&f, Format::Avif).unwrap_err().code,
        ErrorCode::Unsupported
    );
    assert_eq!(
        decode(&f, Format::Avif, None).unwrap_err().code,
        ErrorCode::Unsupported
    );
}

#[test]
fn icc_p3_is_converted_and_srgb_or_garbage_left_alone() {
    let orange = rgba(4, 4, |_, _| [200, 100, 50, 255]);
    let p3 = moxcms::ColorProfile::new_display_p3()
        .encode()
        .expect("p3 icc");
    let srgb = moxcms::ColorProfile::new_srgb().encode().expect("srgb icc");
    let out = decode(&png(&orange, Some(p3)), Format::Png, None).unwrap();
    let px = &out.data[..4];
    // P3 orange is more saturated than sRGB can show: red up, blue down.
    assert!(px[0] > 210 && px[2] < 45 && px[3] == 255, "{px:?}");
    for icc in [srgb, b"definitely not an ICC profile".to_vec(), vec![]] {
        let out = decode(&png(&orange, Some(icc)), Format::Png, None).unwrap();
        assert_eq!(out, orange);
    }
}

#[test]
fn resize_keeps_transparent_colour_out_of_edges() {
    // Invisible bright green next to opaque blue: premultiplied resizing
    // must not leak green into the edge.
    let img = rgba(64, 64, |x, _| if x < 32 { [0, 255, 0, 0] } else { BLUE });
    let out = photosee_core::resize::resize(&img, Size::new(16, 16)).unwrap();
    for p in out.data.chunks_exact(4).filter(|p| p[3] > 0) {
        assert!(p[1] < 8, "green leaked: {p:?}");
    }
    let enc = photosee_core::encode::encode(&out, 85).unwrap();
    assert_eq!(enc.mime, "image/png");
}
