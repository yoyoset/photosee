//! Family accounts for the standalone server (Docker, multi-user mode).
//!
//! Why: one server on the home LAN, several readers (asked for 2026-11-05).
//! Opening the page showed everyone both disks and whoever read last. Now
//! each person logs in with a name and PIN and sees only the folders they
//! added to their own list ("mounted"); nobody starts with whole disks.
//!
//! The list lives in `users.json`, a plain file the server rewrites on every
//! change (no database). An admin creates and deletes accounts; everyone
//! picks their own folders from the volumes (PHOTOSEE_ROOTS). Volumes are
//! browsable by every user while picking, so this separates libraries, it
//! does not hide folders from family members.
//!
//! PINs are stored as written: a 4-12 digit PIN has at most 10^12 values, so
//! a hash would not stop someone who can read the file, and the file sits in
//! the server's data dir (owner 65534).

use crate::paths::Root;
use photosee_core::{Error, ErrorCode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub pin: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub admin: bool,
    /// Folders this user mounted, as paths inside the container.
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct UsersFile {
    users: Vec<Account>,
}

fn bad(msg: String) -> Error {
    Error::new(ErrorCode::Forbidden, msg)
}

/// Also the user's data folder name, so it must be one plain segment.
pub fn check_name(n: &str) -> Result<(), Error> {
    let ok = !n.trim().is_empty()
        && n.trim() == n
        && n.chars().count() <= 32
        && !n.starts_with('.')
        && !n.contains(['/', '\\', '\0', ':']);
    if ok {
        Ok(())
    } else {
        Err(bad(format!("bad name `{n}`")))
    }
}

pub fn check_pin(p: &str) -> Result<(), Error> {
    if (4..=12).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(bad("PIN must be 4-12 digits".into()))
    }
}

/// Parses and validates the list. An empty list is valid: first run.
pub fn parse_users(json: &[u8]) -> Result<Vec<Account>, String> {
    let file: UsersFile = serde_json::from_slice(json).map_err(|e| format!("users.json: {e}"))?;
    let mut seen: Vec<&str> = Vec::new();
    for u in &file.users {
        let what = |e: Error| format!("users.json: {}", e.message);
        check_name(&u.name).map_err(what)?;
        check_pin(&u.pin).map_err(|e| format!("users.json: `{}`: {}", u.name, e.message))?;
        if seen.contains(&u.name.as_str()) {
            return Err(format!("users.json: `{}` appears twice", u.name));
        }
        seen.push(&u.name);
    }
    Ok(file.users)
}

/// A missing file is a first run, not an error.
pub fn load(file: &Path) -> Result<Vec<Account>, String> {
    match std::fs::read(file) {
        Ok(b) => parse_users(&b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", file.display())),
    }
}

pub fn save(file: &Path, users: &[Account]) -> Result<(), Error> {
    let body = UsersFile {
        users: users.to_vec(),
    };
    let bytes =
        serde_json::to_vec_pretty(&body).map_err(|e| Error::new(ErrorCode::Io, e.to_string()))?;
    photosee_core::config::write_atomic(file, &bytes)
}

/// The user's roots: one per mounted folder that still exists, named after
/// its last segment ("Comics", then "Comics 2" for a second one). A
/// vanished folder is skipped, not fatal: a moved folder on the NAS must not
/// keep the server (or the user's other folders) from starting.
pub fn roots_of(dirs: &[PathBuf], is_dir: impl Fn(&Path) -> bool) -> Vec<Root> {
    let mut roots: Vec<Root> = Vec::new();
    for d in dirs {
        if !is_dir(d) {
            eprintln!(
                "photosee-server: mounted folder is gone, skipped: {}",
                d.display()
            );
            continue;
        }
        let base = d
            .file_name()
            .map_or_else(|| "root".to_owned(), |n| n.to_string_lossy().into_owned());
        let mut name = base.clone();
        let mut n = 2;
        while roots.iter().any(|r| r.name == name) {
            name = format!("{base} {n}");
            n += 1;
        }
        roots.push(Root {
            name,
            dir: d.clone(),
        });
    }
    roots
}

/// 32 bytes from the OS CSPRNG, hex. Same reasoning as the desktop token
/// (app/src/token.rs): never time-seeded.
pub fn new_token() -> Result<String, Error> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b)
        .map_err(|e| Error::new(ErrorCode::Io, format!("OS randomness unavailable: {e}")))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}
