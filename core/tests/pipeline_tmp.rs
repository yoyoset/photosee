#[path = "pipeline_util.rs"]
mod util;
use photosee_core::{Format, Size};
use std::time::Instant;
use util::*;

fn t<R>(name: &str, f: impl FnOnce() -> R) -> R {
    let s = Instant::now();
    let r = f();
    println!("{name}: {:.1} ms", s.elapsed().as_secs_f64() * 1000.0);
    r
}

#[test]
#[ignore]
fn breakdown() {
    let mut seed = 0x9E37_79B9u32;
    let img = rgba(6000, 4000, |x, y| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let n = (seed & 31) as u8;
        let gx = (x * 220 / 6000) as u8;
        let gy = (y * 220 / 4000) as u8;
        [gx + n, gy + n, gx / 2 + gy / 2 + n, 255]
    });
    let src = jpeg(&img, 90);
    let flat = jpeg(
        &rgba(6000, 4000, |x, y| {
            [(x / 30) as u8, (y / 20) as u8, 100, 255]
        }),
        90,
    );
    for (name, f) in [("noisy", &src), ("smooth", &flat)] {
        println!("--- {name} {} bytes", f.len());
        for _ in 0..2 {
            let d = t("image load_from_memory", || {
                image::load_from_memory_with_format(f, image::ImageFormat::Jpeg).unwrap()
            });
            let rgba = t("to_rgba8", || d.to_rgba8());
            drop(rgba);
            t("orient::read", || {
                photosee_core::orient::read(f, Format::Jpeg).unwrap()
            });
            t("info", || {
                photosee_core::decode::info(f, Format::Jpeg).unwrap()
            });
            let full = t("std decode", || {
                photosee_core::decode::decode(f, Format::Jpeg, None).unwrap()
            });
            t("has_alpha", || photosee_core::encode::has_alpha(&full));
            let small = t("resize 2160x1440", || {
                photosee_core::resize::resize(&full, Size::new(2160, 1440)).unwrap()
            });
            t("encode q85", || {
                photosee_core::encode::encode(&small, 85).unwrap()
            });
            t("encode q75", || {
                photosee_core::encode::encode(&small, 75).unwrap()
            });
        }
    }
}
