//! Top-level places for the browser's tree: Pictures, Desktop, then drives.
//!
//! No disk is touched. GetLogicalDrives and GetDriveTypeW read the volume
//! table, and SHGetKnownFolderPath reads the registry. A disconnected mapped
//! drive therefore cannot stall start-up, which `Path::exists` on `Z:\` can,
//! for seconds.

use photosee_server::state::Place;

#[cfg(windows)]
pub fn places() -> Vec<Place> {
    let mut out = known_folders();
    out.extend(drives());
    out
}

#[cfg(not(windows))]
pub fn places() -> Vec<Place> {
    Vec::new() // M3: macOS/Linux places
}

#[cfg(windows)]
fn known_folders() -> Vec<Place> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        FOLDERID_Desktop, FOLDERID_Pictures, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };
    let mut out = Vec::new();
    for (id, name) in [
        (&FOLDERID_Pictures, "Pictures"),
        (&FOLDERID_Desktop, "Desktop"),
    ] {
        // SAFETY: a valid KNOWNFOLDERID and no token; the returned buffer is
        // owned by us and freed with CoTaskMemFree right after copying it.
        let path = unsafe {
            let Ok(p) = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) else {
                continue;
            };
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        };
        if let Some(path) = path {
            // `name` is a stable key; the UI shows its localized label.
            out.push(Place {
                name: name.into(),
                path,
                kind: "folder",
            });
        }
    }
    out
}

#[cfg(windows)]
fn drives() -> Vec<Place> {
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    use windows::core::HSTRING;
    const DRIVE_REMOTE: u32 = 4;
    const DRIVE_CDROM: u32 = 5;
    // SAFETY: no arguments; returns a bitmask of present drive letters.
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for (i, letter) in ('A'..='Z').enumerate() {
        if mask & (1 << i) == 0 {
            continue;
        }
        let root = format!("{letter}:\\");
        // SAFETY: a valid NUL-terminated root path that lives for the call.
        let kind = match unsafe { GetDriveTypeW(&HSTRING::from(root.as_str())) } {
            DRIVE_REMOTE => "network",
            // Optical drives spin up when listed and rarely hold photos.
            DRIVE_CDROM => continue,
            _ => "drive",
        };
        out.push(Place {
            name: format!("{letter}:"),
            path: root,
            kind,
        });
    }
    out
}
