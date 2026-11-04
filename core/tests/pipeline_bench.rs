//! 24 MP hot-path timings and the resize filter comparison. Run with
//! `cargo test -p photosee-core --release --test pipeline_bench -- --ignored --nocapture`.
#![allow(
    clippy::unwrap_used,
    reason = "bench helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "pipeline_util.rs"]
mod util;

use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use photosee_core::decode::std_formats::decode_image;
use photosee_core::encode::encode;
use photosee_core::pipeline::{Rendered, render};
use photosee_core::resize::resize;
use photosee_core::testfs::MemFs;
// `decode` is std_formats' (WIC on Windows); the pipeline calls the same one.
use photosee_core::decode::std_formats::decode;
use photosee_core::{Format, Rgba8, Size};
use std::path::Path;
use std::time::Instant;
use util::*;

const RUNS: usize = 5;

/// Gradient plus deterministic noise: photo-like entropy for the JPEG
/// coder, smooth areas for the resizer.
fn photo_like(w: u32, h: u32) -> Rgba8 {
    let mut seed = 0x9E37_79B9u32;
    rgba(w, h, |x, y| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let n = (seed & 31) as u8;
        let gx = (x * 220 / w) as u8;
        let gy = (y * 220 / h) as u8;
        [gx + n, gy + n, (gx / 2 + gy / 2) + n, 255]
    })
}

fn median_ms(mut f: impl FnMut()) -> f64 {
    let mut t: Vec<f64> = (0..RUNS)
        .map(|_| {
            let s = Instant::now();
            f();
            s.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    t.sort_by(f64::total_cmp);
    t[RUNS / 2]
}

type DecodeFn = fn(&Vec<u8>, Size) -> Rgba8;

/// decode / resize / encode / render for one source size, for the path in
/// use (WIC on Windows: the decode is already reduced) and for the `image`
/// path (full-size decode), which is what every other platform runs.
fn split(label: &str, w: u32, h: u32) {
    let src = jpeg(&photo_like(w, h), 92);
    println!("{label}: {w}x{h} JPEG q92, {} bytes", src.len());
    let box_ = Size::new(2560, 1440);
    let target = photosee_core::resize::fit(Size::new(w, h), box_, false);
    let paths: [(&str, DecodeFn); 2] = [
        ("in use", |s, t| decode(s, Format::Jpeg, Some(t)).unwrap()),
        ("image ", |s, _| decode_image(s, Format::Jpeg).unwrap()),
    ];
    for (name, dec_fn) in paths {
        let mut img = None;
        let dec = median_ms(|| img = Some(dec_fn(&src, target)));
        let img = img.unwrap();
        let mut small = None;
        let res = median_ms(|| small = Some(resize(&img, target).unwrap()));
        let small = small.unwrap();
        let enc = median_ms(|| drop(encode(&small, 85).unwrap()));
        println!(
            "  {name} -> {}x{} (decoded {}x{}): decode {dec:.1} ms, resize {res:.1} ms,              encode {enc:.1} ms",
            target.w, target.h, img.size.w, img.size.h
        );
    }
    let fs = MemFs::new();
    fs.file("/b/x.jpg", &src);
    let total = median_ms(|| {
        let r = render(&fs, Path::new("/b/x.jpg"), box_, &[]).unwrap();
        assert!(matches!(r, Rendered::Transcoded { .. }));
    });
    println!("  render end to end (median of {RUNS}): {total:.1} ms");
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn bench_24mp_jpeg_to_2560x1440() {
    split("24MP", 6000, 4000);
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn bench_12mp_jpeg_to_2560x1440() {
    split("12MP", 4000, 3000);
}

fn fir(img: &Rgba8, to: Size, f: FilterType) -> Vec<u8> {
    let src = ImageRef::new(img.size.w, img.size.h, &img.data, PixelType::U8x4).unwrap();
    let mut dst = Image::new(to.w, to.h, PixelType::U8x4);
    let opts = ResizeOptions::new()
        .resize_alg(ResizeAlg::Convolution(f))
        .use_alpha(false);
    Resizer::new().resize(&src, &mut dst, &opts).unwrap();
    dst.into_vec()
}

/// Std-dev of the red channel over the middle of the image.
fn stddev(px: &[u8], w: u32, h: u32) -> f64 {
    let vals: Vec<f64> = (h / 4..3 * h / 4)
        .flat_map(|y| (w / 4..3 * w / 4).map(move |x| (y * w + x) as usize))
        .map(|i| f64::from(px[i * 4]))
        .collect();
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64).sqrt()
}

/// Horizontal sine grating of `period` source pixels, full contrast.
fn grating(w: u32, h: u32, period: f64) -> Rgba8 {
    rgba(w, h, |x, _| {
        let v = 127.5 + 127.5 * (std::f64::consts::TAU * f64::from(x) / period).sin();
        [v as u8, v as u8, v as u8, 255]
    })
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn bench_resize_filters() {
    let (sw, sh) = (6000, 4000);
    let to = Size::new(2560, 1707);
    let photo = photo_like(sw, sh);
    // 3 px source period = 0.78 cycles/px after the 0.427x scale, beyond
    // Nyquist: any surviving contrast is aliasing. 24 px stays in band
    // (10 px at the output): surviving contrast there is sharpness.
    let alias = grating(sw, sh, 3.0);
    let pass = grating(sw, sh, 24.0);
    let filters = [
        ("Bilinear", FilterType::Bilinear),
        ("CatmullRom", FilterType::CatmullRom),
        ("Mitchell", FilterType::Mitchell),
        ("Lanczos3", FilterType::Lanczos3),
    ];
    println!(
        "6000x4000 -> 2560x1707, U8x4, median of {RUNS}; stddev of a full-contrast input is 90"
    );
    for (name, f) in filters {
        let ms = median_ms(|| drop(fir(&photo, to, f)));
        let a = stddev(&fir(&alias, to, f), to.w, to.h);
        let p = stddev(&fir(&pass, to, f), to.w, to.h);
        println!("{name:>10}: {ms:6.1} ms  alias residual {a:5.2}  passband {p:5.2}");
    }
}

#[test]
#[ignore = "benchmark: run in release with --ignored --nocapture"]
fn bench_wic_decode_by_scale() {
    let src = jpeg(&photo_like(6000, 4000), 92);
    for (w, h) in [(6000, 4000), (3000, 2000), (1500, 1000), (750, 500)] {
        let ms = median_ms(|| drop(decode(&src, Format::Jpeg, Some(Size::new(w, h))).unwrap()));
        println!("24MP decode with max {w}x{h}: {ms:.1} ms");
    }
}
