//! Measurement hook. Active only when `PHOTOSEE_PERF_OUT` names a file at
//! launch; then Rust marks and page reports are appended to it as JSON lines
//! with Unix-epoch-millisecond timestamps, so a driver script can compare
//! them with its own clock. Without the variable nothing is written and the
//! `/api/v1/host/perf` route does not exist.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const PERF_JS: &str = include_str!("perf.js");

pub fn epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

pub fn out_path() -> Option<PathBuf> {
    std::env::var_os("PHOTOSEE_PERF_OUT").map(PathBuf::from)
}

/// One line; failures are ignored on purpose (a measurement aid must never
/// break the app).
pub fn append(path: &std::path::Path, line: &serde_json::Value) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

pub fn mark(name: &str) {
    if let Some(p) = out_path() {
        append(&p, &serde_json::json!({ "kind": name, "t": epoch_ms() }));
    }
}
