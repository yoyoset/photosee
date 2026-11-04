//! 200 decodes (good, truncated, garbage) must not grow the process handle
//! count or private memory: every COM object and the COM init itself are
//! released on every path. Own test binary so no parallel test skews counts.
//!
//! The brief suggests `GetProcessHandleCount`, but the workspace does not
//! enable the windows crate's `Win32_System_Threading` feature, so the
//! counts come from PowerShell's Get-Process for this PID instead.
#![cfg(windows)]

#[path = "heic_enc.rs"]
mod heic_enc;

use photosee_core::decode::heic_wic::{decode, info};
use photosee_core::io::ReadAt;

/// (handles, private bytes) of this process.
fn counts() -> Option<(u64, u64)> {
    let pid = std::process::id();
    let script =
        format!("$p = Get-Process -Id {pid}; \"$($p.HandleCount) $($p.PrivateMemorySize64)\"");
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut it = text.split_whitespace().map(|v| v.parse::<u64>().ok());
    Some((it.next()??, it.next()??))
}

fn round(good: &dyn ReadAt, cut: &dyn ReadAt, junk: &dyn ReadAt) {
    assert!(decode(good, None).is_ok());
    assert!(info(good).is_ok());
    assert!(decode(cut, None).is_err());
    assert!(decode(junk, None).is_err());
}

#[test]
fn two_hundred_decodes_do_not_leak() {
    let Ok(good) = heic_enc::encode_heic(64, 48, &heic_enc::quadrant_bgra(64, 48)) else {
        return println!("skipped: no WIC HEIF encoder");
    };
    let cut = good[..good.len() - 100].to_vec();
    let mut junk = good[..32].to_vec();
    junk.extend_from_slice(&[0xAB; 600]);
    // Warm up: the first call loads the extension and pins one decoder.
    for _ in 0..10 {
        round(&good, &cut, &junk);
    }
    let Some((h0, m0)) = counts() else {
        return println!("skipped: could not read handle count via PowerShell");
    };
    for _ in 0..200 {
        round(&good, &cut, &junk);
    }
    let (h1, m1) = counts().expect("counts after the loop");
    let (dh, dm) = (h1 as i64 - h0 as i64, (m1 as i64 - m0 as i64) / 1024);
    println!("200 rounds (800 calls): handles {h0} -> {h1} ({dh:+}), private {dm:+} KiB");
    // Measured: a bare WIC CopyPixels on a HEIF frame (no PhotoSee code)
    // leaks ~1 handle per call inside the HEIF/HEVC extension (300 calls:
    // +307; frame without CopyPixels: +0). So 200 good decodes may cost ~200
    // handles; info, truncated and garbage calls must cost none. A leak of
    // our own (one COM object per call) would add 200-800 more.
    assert!(dh < 260, "handle count grew by {dh}");
    assert!(dm < 16 * 1024, "private memory grew by {dm} KiB");
}
