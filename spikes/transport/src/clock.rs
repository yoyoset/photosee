//! Wall-clock timestamps shared by Rust, the page and the driver script.
//!
//! Everything is Unix epoch milliseconds (f64) so the three can be compared:
//! the page reports `performance.timeOrigin + performance.now()`, the driver
//! reports the same in Node, and Rust uses `SystemTime`, which on Windows is
//! `GetSystemTimePreciseAsFileTime` (100 ns resolution).
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

pub fn epoch_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// Kernel creation time of this process, as Unix epoch ms.
///
/// Pitfall: this is stamped by the kernel when the process object is created,
/// before the loader maps DLLs, so it is the honest "process start". Its
/// resolution is not documented; the driver also passes its own pre-spawn
/// timestamp (`--launched-at`) so the two can be cross-checked.
pub fn process_created_epoch_ms() -> Option<f64> {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut c, mut e, mut k, mut u) = (zero, zero, zero, zero);
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing,
    // and all four out-pointers are valid, exclusively borrowed locals.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    if ok == 0 {
        return None;
    }
    let ticks = (u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime);
    // FILETIME counts 100 ns since 1601-01-01; Unix epoch is 11644473600 s later.
    Some((ticks as f64 - 116_444_736_000_000_000.0) / 10_000.0)
}

/// Named marks recorded on the Rust side, as epoch ms.
#[derive(Default)]
pub struct Timeline {
    marks: Mutex<Vec<(String, f64)>>,
}

impl Timeline {
    pub fn mark(&self, name: &str) {
        let t = epoch_ms();
        if let Ok(mut m) = self.marks.lock() {
            m.push((name.to_owned(), t));
        }
    }

    pub fn mark_at(&self, name: &str, t: f64) {
        if let Ok(mut m) = self.marks.lock() {
            m.push((name.to_owned(), t));
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let m = self.marks.lock().map(|m| m.clone()).unwrap_or_default();
        serde_json::Value::Object(m.into_iter().map(|(k, v)| (k, v.into())).collect())
    }
}
