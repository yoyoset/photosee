//! Writes the spike's test JPEGs into the given directory.
//!   noise-*.jpg  RGB noise: compresses badly, so modest dimensions give the
//!                ≈3 MB / ≈30 MB files the transport test asks for.
//!   photo-*.jpg  smooth gradients + light grain: decode cost of a real photo
//!                (24MP for cold start / prefetch, screen size for /img?w=&h=).
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use image::ExtendedColorType;
use image::codecs::jpeg::JpegEncoder;

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn noise(w: u32, h: u32, seed: u64) -> Vec<u8> {
    let mut rng = XorShift(seed);
    let mut buf = vec![0u8; (w * h * 3) as usize];
    for chunk in buf.chunks_mut(8) {
        let v = rng.next().to_le_bytes();
        chunk.copy_from_slice(&v[..chunk.len()]);
    }
    buf
}

fn photo(w: u32, h: u32, seed: u64) -> Vec<u8> {
    let mut rng = XorShift(seed);
    let mut buf = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let base = [
                0.5 + 0.4 * (fx * 7.0 + fy * 3.0).sin(),
                0.5 + 0.4 * (fy * 5.0 - fx * 2.0).cos(),
                0.5 + 0.4 * ((fx + fy) * 11.0).sin(),
            ];
            let grain = rng.next();
            for (c, b) in base.iter().enumerate() {
                let g = ((grain >> (c * 8)) & 0x0f) as f32 - 7.5;
                buf.push((b * 255.0 + g).clamp(0.0, 255.0) as u8);
            }
        }
    }
    buf
}

fn write(dir: &Path, name: &str, w: u32, h: u32, px: &[u8]) -> std::io::Result<()> {
    let path = dir.join(format!("{name}.jpg"));
    let mut enc = JpegEncoder::new_with_quality(BufWriter::new(File::create(&path)?), 90);
    enc.encode(px, w, h, ExtendedColorType::Rgb8).map_err(std::io::Error::other)?;
    let len = std::fs::metadata(&path)?.len();
    println!("{name}: {w}x{h} {:.2} MB", len as f64 / 1e6);
    Ok(())
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "samples".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;
    write(dir, "noise-3mb", 1460, 1100, &noise(1460, 1100, 1))?;
    write(dir, "noise-30mb", 4900, 3270, &noise(4900, 3270, 2))?;
    write(dir, "photo-24mp", 6000, 4000, &photo(6000, 4000, 3))?;
    write(dir, "photo-screen", 2560, 1440, &photo(2560, 1440, 4))?;
    Ok(())
}
