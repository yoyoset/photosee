//! Orchestration: runs the phases in order and assembles the report. Failures
//! of individual phases become report lines; nothing here aborts the run.

use crate::{phases, report, scan, sweep, timefmt};
use std::path::Path;

/// Phase 1 repeated `runs` times. The first successful listing is kept (its
/// file set drives the later phases); a failure to open the directory stops
/// the loop, since later runs would fail identically.
fn list_phase(
    dir: &Path,
    runs: usize,
    errors: &mut Vec<String>,
) -> (Option<scan::Listing>, Vec<f64>) {
    let (mut first, mut times) = (None, Vec::new());
    for _ in 0..runs {
        match scan::list_once(dir) {
            Ok(l) => {
                times.push(phases::ms(l.elapsed));
                if first.is_none() {
                    errors.extend(l.errors.iter().cloned());
                    first = Some(l);
                }
            }
            Err(e) => {
                errors.push(e);
                break;
            }
        }
    }
    (first, times)
}

/// Phases 2-4, all of which need JPEGs. Returns their report sections.
fn jpeg_phases(l: &scan::Listing, runs: usize, errors: &mut Vec<String>) -> String {
    let (mut heads, mut firsts) = (Vec::new(), Vec::new());
    for _ in 0..runs {
        let h = phases::head_reads(&l.jpegs);
        errors.extend(h.errors.iter().cloned());
        heads.push(h);
        match phases::first_image(&l.jpegs) {
            Ok(f) => firsts.push(f),
            Err(e) => {
                errors.push(e);
                break;
            }
        }
    }
    let results = if l.jpegs.is_empty() {
        Vec::new()
    } else {
        sweep::sweep(&l.jpegs)
    };
    errors.extend(results.iter().flat_map(|r| r.errors.iter().cloned()));
    report::head_section(&heads)
        + &report::first_section(&firsts)
        + &report::sweep_section(&results, l.jpegs.len())
}

/// Returns the Markdown report and whether the directory could be listed at all
/// (the caller maps `false` to a non-zero exit code).
pub fn run(dir: &Path, runs: usize) -> (String, bool) {
    let mut errors = Vec::new();
    let (listing, times) = list_phase(dir, runs, &mut errors);
    let date = timefmt::now_utc();
    let shown = dir.display().to_string();
    let Some(l) = listing else {
        let head = report::header(&date, &shown, 0, 0, 0, runs);
        return (head + &report::errors_section(&errors), false);
    };
    let mut out = report::header(&date, &shown, l.files, l.jpegs.len(), l.bytes, runs);
    out += &report::list_section(&times);
    out += &jpeg_phases(&l, runs, &mut errors);
    (out + &report::errors_section(&errors), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn missing_dir_reports_error_and_fails_softly() {
        let (text, ok) = run(Path::new("Z:/nope/photosee-bench-missing"), 2);
        assert!(!ok);
        assert!(text.contains("## Errors") && text.contains("cannot list"));
    }

    #[test]
    fn smoke_run_on_generated_folder() {
        let d = TempDir::new("smoke");
        for i in 0..60 {
            d.write(&format!("img{i:03}.jpg"), 20_000 + i * 10);
        }
        d.write("notes.txt", 10);
        let (text, ok) = run(d.path(), 2);
        assert!(ok);
        for needle in [
            "## 1. List",
            "## 2. Head",
            "## 3. First",
            "## 4. Concurrency",
            "files: 61 (60 JPEG)",
        ] {
            assert!(text.contains(needle), "missing {needle}\n{text}");
        }
        assert!(text.contains("WARNING"), "60 < 200 JPEGs must warn");
        assert!(text.contains("<- smallest K"));
        assert_eq!(
            std::fs::read_dir(d.path()).unwrap().count(),
            61,
            "tool must not write into DIR"
        );
    }

    #[test]
    fn folder_without_jpegs_still_reports() {
        let d = TempDir::new("nojpeg");
        d.write("a.png", 10);
        let (text, ok) = run(d.path(), 1);
        assert!(ok);
        assert!(text.contains("no JPEG files found") && text.contains("skipped"));
    }
}
