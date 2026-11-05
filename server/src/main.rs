//! Standalone server (Docker / LAN). Configuration via environment:
//!   PHOTOSEE_ROOTS     `name=dir;name2=dir2`, read-only roots, one open
//!                      library for everyone on the LAN
//!   PHOTOSEE_USERS     path of users.json: family accounts (name + PIN),
//!                      each mounting their own folders from PHOTOSEE_ROOTS,
//!                      which then become the volumes (accounts.rs, multi.rs)
//!   PHOTOSEE_PORT      listen port (default 8080)
//!   PHOTOSEE_HOSTS     `host:port,host2:port`, accepted Host headers
//!                      (DNS-rebinding defence); empty = any
//!   PHOTOSEE_DATA      settings and thumbnail cache dir (default ./data)
//!   PHOTOSEE_WEB       built web UI dir (default ./web)
//!
//! PHOTOSEE_PASSWORD (one shared password) is refused at start-up instead of
//! being silently ignored: accounts replaced it. The server only ever reads.

use photosee_core::archive::ArchiveFs;
use photosee_core::io::LocalFs;
use photosee_core::thumbcache::ThumbCache;
use photosee_server::AppState;
use photosee_server::cache::ListingCache;
use photosee_server::multi::{self, MakeState, Multi};
use photosee_server::paths::{Policy, Root};
use photosee_server::queue::IoQueue;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Same figure as the desktop shell; a NAS gains nothing from more.
const IO_SLOTS: usize = 4;
/// Comic pages make many thumbnails (300 per volume); 1 GiB holds tens of
/// thousands at ~30 KB each.
const THUMB_BUDGET: u64 = 1 << 30;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn parse_roots(spec: &str) -> Result<Vec<Root>, String> {
    let mut roots = Vec::new();
    for part in spec.split(';').filter(|p| !p.trim().is_empty()) {
        let (name, dir) = part
            .split_once('=')
            .ok_or_else(|| format!("PHOTOSEE_ROOTS: `{part}` is not name=dir"))?;
        let (name, dir) = (name.trim(), PathBuf::from(dir.trim()));
        if name.is_empty() || name.contains(['/', '\\']) {
            return Err(format!("PHOTOSEE_ROOTS: bad root name `{name}`"));
        }
        if !dir.is_dir() {
            return Err(format!(
                "PHOTOSEE_ROOTS: {} is not a directory",
                dir.display()
            ));
        }
        roots.push(Root {
            name: name.to_owned(),
            dir,
        });
    }
    if roots.is_empty() {
        return Err("PHOTOSEE_ROOTS is required, e.g. books=/data/books".into());
    }
    Ok(roots)
}

fn hosts() -> Vec<String> {
    env("PHOTOSEE_HOSTS").map_or_else(Vec::new, |h| {
        h.split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

/// One library's state. `own` holds its settings and ui-state; the file
/// system, IO queue and thumbnail cache are shared by every user.
fn build_state(roots: Vec<Root>, own: &Path, shared: &AppState) -> AppState {
    AppState {
        policy: Arc::new(Policy::Roots(roots)),
        listings: Arc::new(ListingCache::default()),
        settings_path: own.join("settings.json"),
        ..shared.clone()
    }
}

fn shared_state(data: &Path) -> AppState {
    AppState {
        fs: Arc::new(ArchiveFs::new(LocalFs)),
        policy: Arc::new(Policy::Roots(Vec::new())),
        listings: Arc::new(ListingCache::default()),
        queue: Arc::new(IoQueue::new(IO_SLOTS)),
        thumbs: Arc::new(ThumbCache::new(data.join("thumbs"), THUMB_BUDGET)),
        settings_path: data.join("settings.json"),
        token: None,
        allowed_hosts: hosts(),
        start: Arc::new(RwLock::new(None)),
        places: Vec::new(),
    }
}

/// PHOTOSEE_USERS set: accounts, and PHOTOSEE_ROOTS are the volumes each
/// user picks their own folders from. Else PHOTOSEE_ROOTS is the open library.
fn app(data: &Path, web: PathBuf) -> Result<(axum::Router, String), String> {
    let shared = shared_state(data);
    let roots = parse_roots(&env("PHOTOSEE_ROOTS").unwrap_or_default())?;
    let what = roots
        .iter()
        .map(|r| format!("{}={}", r.name, r.dir.display()))
        .collect::<Vec<_>>()
        .join(", ");
    let Some(file) = env("PHOTOSEE_USERS") else {
        let app = photosee_server::router(build_state(roots, data, &shared), Some(web));
        return Ok((app, format!("open library {what}")));
    };
    let base = data.to_path_buf();
    let hosts = shared.allowed_hosts.clone();
    let make: MakeState =
        Box::new(move |name, roots| build_state(roots, &base.join("users").join(name), &shared));
    let m = Multi::new(
        PathBuf::from(file),
        data.join("sessions.json"),
        roots,
        hosts,
        make,
    )?;
    Ok((multi::router(m, web), format!("accounts, volumes {what}")))
}

async fn run() -> Result<(), String> {
    if env("PHOTOSEE_PASSWORD").is_some() {
        return Err(
            "PHOTOSEE_PASSWORD is not supported; use PHOTOSEE_USERS (accounts with PINs)".into(),
        );
    }
    let port: u16 = env("PHOTOSEE_PORT")
        .map_or(Ok(8080), |p| p.parse())
        .map_err(|e| format!("PHOTOSEE_PORT: {e}"))?;
    let data = PathBuf::from(env("PHOTOSEE_DATA").unwrap_or_else(|| "data".into()));
    let web = PathBuf::from(env("PHOTOSEE_WEB").unwrap_or_else(|| "web".into()));
    if !web.join("index.html").is_file() {
        return Err(format!("web UI not found: {}/index.html", web.display()));
    }
    std::fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;
    let (app, what) = app(&data, web)?;
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|e| format!("cannot listen on port {port}: {e}"))?;
    eprintln!("photosee-server: listening on :{port}, {what}");
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("photosee-server: {e}");
        std::process::exit(2);
    }
}
