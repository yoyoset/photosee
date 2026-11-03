//! Markdown rendering. Pure functions from measurements to text, so the report
//! layout is testable without touching a disk.

use crate::phases::{FirstRun, HeadRun};
use crate::stats::{self, Summary};
use crate::sweep::{self, KResult};

const MAX_ERRORS_SHOWN: usize = 20;

fn f1(x: f64) -> String {
    format!("{x:.1}")
}

fn summary_line(label: &str, s: Option<Summary>, unit: &str) -> String {
    match s {
        Some(s) => format!(
            "{label}: min {} / median {} / max {} {unit}\n",
            f1(s.min),
            f1(s.median),
            f1(s.max)
        ),
        None => format!("{label}: no data\n"),
    }
}

pub fn header(
    date: &str,
    dir: &str,
    files: usize,
    jpegs: usize,
    bytes: u64,
    runs: usize,
) -> String {
    format!(
        "# bench-smb report\n\n- date: {date}\n- dir: `{dir}`\n- files: {files} ({jpegs} JPEG), {:.1} MB\n\
         - os: {} {}\n- runs: {runs} (run 1 is cold on a best-effort basis)\n\n",
        bytes as f64 / 1e6,
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn list_section(times_ms: &[f64]) -> String {
    let mut s =
        String::from("## 1. List (read_dir + metadata)\n\n| run | kind | ms |\n|---|---|---|\n");
    for (i, t) in times_ms.iter().enumerate() {
        let kind = if i == 0 { "cold" } else { "warm" };
        s += &format!("| {} | {kind} | {} |\n", i + 1, f1(*t));
    }
    s += "\n";
    s += &summary_line("all runs", stats::summarize(times_ms), "ms");
    if times_ms.len() > 1 {
        s += &summary_line("warm (runs 2+)", stats::summarize(&times_ms[1..]), "ms");
    }
    s + "\n"
}

pub fn head_section(runs: &[HeadRun]) -> String {
    let mut s = String::from(
        "## 2. Head reads (first 64 KB of up to 50 JPEGs)\n\n| run | files | median ms | p90 ms |\n|---|---|---|---|\n",
    );
    let mut medians = Vec::new();
    for (i, r) in runs.iter().enumerate() {
        let (m, p) = (stats::median(&r.latencies_ms), stats::p90(&r.latencies_ms));
        medians.extend(m);
        let cell = |x: Option<f64>| x.map_or("-".to_string(), f1);
        s += &format!(
            "| {} | {} | {} | {} |\n",
            i + 1,
            r.latencies_ms.len(),
            cell(m),
            cell(p)
        );
    }
    s += "\n";
    s += &summary_line(
        "per-file median across runs",
        stats::summarize(&medians),
        "ms",
    );
    s + "\n"
}

pub fn first_section(runs: &[FirstRun]) -> String {
    let mut s = String::from(
        "## 3. First image (full read of the first JPEG)\n\n| run | ms | size KB |\n|---|---|---|\n",
    );
    for (i, r) in runs.iter().enumerate() {
        s += &format!("| {} | {} | {} |\n", i + 1, f1(r.ms), r.bytes / 1000);
    }
    let times: Vec<f64> = runs.iter().map(|r| r.ms).collect();
    s + "\n" + &summary_line("all runs", stats::summarize(&times), "ms") + "\n"
}

pub fn sweep_section(results: &[KResult], jpeg_count: usize) -> String {
    let mut s = String::from("## 4. Concurrency sweep (40 whole JPEGs per K, single pass)\n\n");
    if results.is_empty() {
        return s + "skipped: no JPEG files\n\n";
    }
    if sweep::reuses_files(jpeg_count) {
        s += &format!(
            "WARNING: only {jpeg_count} JPEGs (< 200); files are reused across K, so the OS cache inflates later K.\n\n"
        );
    }
    let rates: Vec<(usize, f64)> = results.iter().map(|r| (r.k, r.mb_per_s())).collect();
    let pick = stats::pick_k(&rates);
    s += "| K | seconds | MB/s | errors | |\n|---|---|---|---|---|\n";
    for r in results {
        let mark = if pick == Some(r.k) {
            "<- smallest K with >=90% of best MB/s"
        } else {
            ""
        };
        s += &format!(
            "| {} | {:.3} | {} | {} | {mark} |\n",
            r.k,
            r.secs,
            f1(r.mb_per_s()),
            r.errors.len()
        );
    }
    s + "\n"
}

pub fn errors_section(errors: &[String]) -> String {
    let mut s = String::from("## Errors\n\n");
    if errors.is_empty() {
        return s + "none\n";
    }
    for e in errors.iter().take(MAX_ERRORS_SHOWN) {
        s += &format!("- {e}\n");
    }
    if errors.len() > MAX_ERRORS_SHOWN {
        s += &format!("- ... and {} more\n", errors.len() - MAX_ERRORS_SHOWN);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_marks_exactly_one_k_and_warns_on_small_sets() {
        let mk = |k, bytes| KResult {
            k,
            secs: 1.0,
            bytes,
            errors: vec![],
        };
        let r = vec![mk(1, 50_000_000), mk(2, 95_000_000), mk(4, 100_000_000)];
        let s = sweep_section(&r, 10);
        assert_eq!(s.matches("<- smallest K").count(), 1);
        assert!(s.contains("| 2 | 1.000 | 95.0 | 0 | <-"));
        assert!(s.contains("WARNING"));
        assert!(!sweep_section(&r, 200).contains("WARNING"));
    }

    #[test]
    fn empty_inputs_render_without_panicking() {
        assert!(list_section(&[]).contains("no data"));
        assert!(
            head_section(&[HeadRun {
                latencies_ms: vec![],
                errors: vec![]
            }])
            .contains("| - |")
        );
        assert!(first_section(&[]).contains("no data"));
        assert!(sweep_section(&[], 0).contains("skipped"));
        assert!(errors_section(&[]).contains("none"));
    }

    #[test]
    fn errors_are_capped() {
        let e: Vec<String> = (0..25).map(|i| format!("e{i}")).collect();
        assert!(errors_section(&e).contains("... and 5 more"));
    }
}
