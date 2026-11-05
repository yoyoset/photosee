//! Family accounts for the standalone server (Docker, multi-user mode).
//!
//! Why: one server on the home LAN, several readers (asked for 2026-11-05).
//! Opening the page showed everyone every root and whoever read last; each
//! person must see only their own folders, recent list and last page.
//!
//! The isolation is the server's, not the UI's: every user gets their own
//! `AppState` (own `Policy::Roots`, settings, ui-state, listing cache) and a
//! request is routed into exactly one of them by its session cookie. A path
//! outside the user's roots fails in `Policy::resolve` like any other.
//! Shared between users: the file system, the IO queue and the thumbnail
//! cache (keyed by the real file, which says nothing about who asked).
//!
//! Accounts live in `users.json`, edited by hand (no admin UI, see
//! docs/non-goals.md). PINs are stored as written: a 4-12 digit PIN has at
//! most 10^12 values, so a hash would not stop someone who can read the file,
//! and the file sits in the server's data dir (owner 65534, mode 600).

use crate::paths::Root;
use crate::state::CurrentUser;
use photosee_core::{Error, ErrorCode};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Wrong PINs in a row before a user is locked, and for how long. Four
/// digits are 10 000 guesses: at 5 per minute that is about 33 hours.
pub const MAX_FAILS: u32 = 5;
pub const LOCKOUT: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
struct FileRoot {
    name: String,
    dir: PathBuf,
}

#[derive(Deserialize)]
struct FileUser {
    name: String,
    pin: String,
    roots: Vec<FileRoot>,
}

#[derive(Deserialize)]
struct UsersFile {
    users: Vec<FileUser>,
}

pub struct UserSpec {
    pub name: String,
    pub pin: String,
    pub roots: Vec<Root>,
}

/// Also the user's data folder name, so it must be a plain single segment.
fn check_name(n: &str) -> Result<(), String> {
    let ok = !n.is_empty()
        && n.chars().count() <= 32
        && !n.starts_with('.')
        && !n.contains(['/', '\\', '\0', ':']);
    if ok {
        Ok(())
    } else {
        Err(format!("users.json: bad name `{n}`"))
    }
}

fn check_pin(user: &str, p: &str) -> Result<(), String> {
    if (4..=12).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(format!("users.json: PIN of `{user}` must be 4-12 digits"))
    }
}

/// Parses and validates the file. `is_dir` is injected so tests need no
/// real folders; the server passes `Path::is_dir`.
pub fn parse_users(json: &[u8], is_dir: impl Fn(&Path) -> bool) -> Result<Vec<UserSpec>, String> {
    let file: UsersFile = serde_json::from_slice(json).map_err(|e| format!("users.json: {e}"))?;
    if file.users.is_empty() {
        return Err("users.json: no users".into());
    }
    let mut out: Vec<UserSpec> = Vec::new();
    for u in file.users {
        check_name(&u.name)?;
        check_pin(&u.name, &u.pin)?;
        if out.iter().any(|o| o.name == u.name) {
            return Err(format!("users.json: `{}` appears twice", u.name));
        }
        let mut roots: Vec<Root> = Vec::new();
        for r in u.roots {
            check_name(&r.name)?;
            if !is_dir(&r.dir) {
                return Err(format!(
                    "users.json: {} is not a directory",
                    r.dir.display()
                ));
            }
            if roots.iter().any(|o| o.name == r.name) {
                return Err(format!(
                    "users.json: `{}` has two roots named `{}`",
                    u.name, r.name
                ));
            }
            roots.push(Root {
                name: r.name,
                dir: r.dir,
            });
        }
        if roots.is_empty() {
            return Err(format!("users.json: `{}` has no roots", u.name));
        }
        out.push(UserSpec {
            name: u.name,
            pin: u.pin,
            roots,
        });
    }
    Ok(out)
}

struct Fails {
    count: u32,
    until: Option<Instant>,
}

/// Sessions (token -> user name) and the PIN check. Sessions are written to
/// `sessions.json` so a redeploy does not log the whole family out: the
/// user chose "logged in until Switch user".
pub struct Sessions {
    pins: HashMap<String, String>,
    tokens: Mutex<HashMap<String, String>>,
    fails: Mutex<HashMap<String, Fails>>,
    file: PathBuf,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A poisoned map is still a valid map: the panicking writer either
    // inserted or did not.
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Sessions {
    pub fn new(users: &[UserSpec], file: PathBuf) -> Self {
        let pins = users
            .iter()
            .map(|u| (u.name.clone(), u.pin.clone()))
            .collect();
        // Unreadable or corrupt: everyone logs in again, nothing worse.
        let mut tokens: HashMap<String, String> = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        tokens.retain(|_, name| users.iter().any(|u| &u.name == name)); // removed users are out
        Self {
            pins,
            tokens: Mutex::new(tokens),
            fails: Mutex::new(HashMap::new()),
            file,
        }
    }

    pub fn user_of(&self, token: &str) -> Option<CurrentUser> {
        lock(&self.tokens)
            .get(token)
            .map(|n| CurrentUser(n.clone()))
    }

    /// A new session token for `name`, or why not. `now` is injected for tests.
    pub fn login(
        &self,
        name: &str,
        pin: &str,
        now: Instant,
        token: String,
    ) -> Result<String, Error> {
        let wrong = || Error::new(ErrorCode::Unauthorized, "wrong name or PIN");
        let real = self.pins.get(name).ok_or_else(wrong)?;
        let mut fails = lock(&self.fails);
        let f = fails.entry(name.to_owned()).or_insert(Fails {
            count: 0,
            until: None,
        });
        if let Some(t) = f.until.filter(|t| now < *t) {
            let secs = (t - now).as_secs() + 1;
            return Err(Error::new(
                ErrorCode::Unauthorized,
                format!("too many wrong PINs; try again in {secs} s"),
            ));
        }
        if !crate::guard::ct_eq(pin, real) {
            f.count += 1;
            if f.count >= MAX_FAILS {
                f.until = Some(now + LOCKOUT);
            }
            return Err(wrong());
        }
        fails.remove(name);
        drop(fails);
        lock(&self.tokens).insert(token.clone(), name.to_owned());
        self.save();
        Ok(token)
    }

    pub fn logout(&self, token: &str) {
        if lock(&self.tokens).remove(token).is_some() {
            self.save();
        }
    }

    fn save(&self) {
        let bytes = serde_json::to_vec(&*lock(&self.tokens)).unwrap_or_default();
        // Losing this write only means a login is forgotten at the next
        // restart; the session itself works from memory.
        if let Err(e) = photosee_core::config::write_atomic(&self.file, &bytes) {
            eprintln!("photosee-server: cannot save sessions: {e}");
        }
    }
}

/// 32 bytes from the OS CSPRNG, hex. Same reasoning as the desktop token
/// (app/src/token.rs): never time-seeded.
pub fn new_token() -> Result<String, Error> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b)
        .map_err(|e| Error::new(ErrorCode::Io, format!("OS randomness unavailable: {e}")))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}
