//! Real-camera check of the RAW preview locator. Runs only when
//! PHOTOSEE_PRIVATE_FIXTURES points at a folder with a `raw/` subfolder;
//! those files are private and never committed.
//!
//! For every file: the located range lies inside the file, decodes with
//! `image`, matches the SOF size `info` reports, and matches the preview
//! size rawler (the oracle) reports. Header IO is counted per file.

use photosee_core::Format;
use photosee_core::decode::raw::{info, locate_preview};
use photosee_core::io::{ReadAt, read_exact_at};
use rawler::decoders::RawDecodeParams;
use rawler::rawsource::RawSource;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// "A few hundred KB" from the brief, as a hard ceiling per file.
const MAX_HEADER_BYTES: u64 = 512 * 1024;

struct Counting<'a> {
    inner: &'a dyn ReadAt,
    bytes: AtomicU64,
}

impl ReadAt for Counting<'_> {
    fn len(&self) -> photosee_core::Result<u64> {
        self.inner.len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> photosee_core::Result<usize> {
        let n = self.inner.read_at(offset, buf)?;
        self.bytes.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

fn rawler_preview_size(path: &Path) -> Option<(u32, u32)> {
    let src = RawSource::new(path).ok()?;
    let dec = rawler::get_decoder(&src).ok()?;
    let img = dec
        .preview_image(&src, &RawDecodeParams::default())
        .ok()??;
    Some((img.width(), img.height()))
}

fn check_file(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let Some(Format::Raw(kind)) = photosee_core::format::from_extension(&name) else {
        return None;
    };
    let file = std::fs::File::open(path).expect("open sample");
    let src = Counting {
        inner: &file,
        bytes: AtomicU64::new(0),
    };
    // `info` runs the locator and the SOF scan, i.e. everything a request needs.
    let inf = info(&src, kind).unwrap_or_else(|e| panic!("{name}: info: {e}"));
    let header_bytes = src.bytes.load(Ordering::Relaxed);
    let loc = locate_preview(&file, kind).unwrap_or_else(|e| panic!("{name}: locate: {e}"));
    let len = file.len().expect("len");
    assert!(
        loc.offset + loc.len <= len,
        "{name}: range ends past the file"
    );
    let jpeg = read_exact_at(&file, loc.offset, usize::try_from(loc.len).expect("len"))
        .expect("read range");
    let img = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
        .unwrap_or_else(|e| panic!("{name}: range is not a decodable JPEG: {e}"));
    let got = (img.width(), img.height());
    assert_eq!(got, (inf.size.w, inf.size.h), "{name}: SOF size vs decoded");
    let oracle = rawler_preview_size(path);
    if let Some(want) = oracle {
        assert_eq!(got, want, "{name}: our preview vs rawler's");
    }
    assert!(
        header_bytes <= MAX_HEADER_BYTES,
        "{name}: {header_bytes} header bytes"
    );
    Some(format!(
        "{name} ({kind:?}): {}x{} {:?}, range {}+{}, header bytes read {header_bytes}, rawler {}",
        got.0,
        got.1,
        loc.orientation,
        loc.offset,
        loc.len,
        oracle.map_or("n/a".into(), |(w, h)| format!("{w}x{h}")),
    ))
}

#[test]
fn private_raw_samples() {
    let Some(root) = std::env::var_os("PHOTOSEE_PRIVATE_FIXTURES") else {
        println!("PHOTOSEE_PRIVATE_FIXTURES unset: skipping real RAW samples");
        return;
    };
    let dir = Path::new(&root).join("raw");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!("{} missing: skipping real RAW samples", dir.display());
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    let lines: Vec<String> = paths.iter().filter_map(|p| check_file(p)).collect();
    for l in &lines {
        println!("{l}");
    }
    println!("{} RAW samples checked", lines.len());
}
