//! Deterministic test-fixture generator for PhotoSee. See fixtures/MANIFEST.md.
//!
//!   cargo run --release              write fixtures/generated/
//!   cargo run --release -- --check   regenerate into a temp dir, compare
//!
//! Not generated: `anim.webp`. image 0.25.10 only has a lossless *static* WebP
//! encoder and adding a crate is not allowed, so the manifest lists it as missing.

mod codec;
mod exif;
mod fsio;
mod pixels;
mod plan;
mod rng;
mod tiff;

use codec::Result;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn out_dir() -> PathBuf {
    // Resolved at compile time so it works from any working directory.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/generated")
}

fn generate(out: &Path) -> Result<()> {
    let entries = plan::build()?;
    fsio::write_all(out, &entries)?;
    println!("wrote {} entries to {}", entries.len(), out.display());
    println!("skipped: anim.webp (image 0.25.10 cannot encode animated WebP)");
    Ok(())
}

/// Exit-code semantics: Ok(true) = identical, Ok(false) = differences found.
fn check(out: &Path) -> Result<bool> {
    let tmp = std::env::temp_dir().join(format!("gen-fixtures-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    fsio::write_all(&tmp, &plan::build()?)?;
    let problems = fsio::diff_trees(&tmp, out);
    // Clean up even when the comparison failed.
    let _ = std::fs::remove_dir_all(&tmp);
    let problems = problems?;
    for p in &problems {
        eprintln!("{p}");
    }
    if problems.is_empty() {
        println!("identical: {} matches a fresh generation", out.display());
    }
    Ok(problems.is_empty())
}

fn run() -> Result<bool> {
    let out = out_dir();
    match std::env::args().nth(1).as_deref() {
        None => generate(&out).map(|()| true),
        Some("--check") => check(&out),
        Some(other) => Err(format!("unknown argument {other:?}; only --check is supported").into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
