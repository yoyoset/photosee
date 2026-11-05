//! Standalone server (Docker / LAN). Configuration via environment:
//!   PHOTOSEE_ROOTS     `name=dir;name2=dir2`, read-only roots (required)
//!   PHOTOSEE_PORT      listen port (default 8080)
//!   PHOTOSEE_HOSTS     `host:port,host2:port`, accepted Host headers
//!                      (DNS-rebinding defence); empty = any
//!   PHOTOSEE_DATA      settings and thumbnail cache dir (default ./data)
//!   PHOTOSEE_WEB       built web UI dir (default ./web)
//!
//! There is no login yet: the web UI has no password screen, so
//! PHOTOSEE_PASSWORD is refused at start-up instead of being silently
//! ignored. The server is for a trusted LAN and only ever reads.

use photosee_core::archive::ArchiveFs;
use photosee_core::io::LocalFs;
use photosee_core::thumbcache::ThumbCache;
use photosee_server::AppState;
use photosee_server::cache::ListingCache;
use photosee_server::paths::{Policy, Root};
use photosee_server::queue::IoQueue;
use std::path::PathBuf;
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

fn build_state(roots: Vec<Root>, data: &std::path::Path) -> AppState {
    let hosts = env("PHOTOSEE_HOSTS").map_or_else(Vec::new, |h| {
        h.split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    });
    AppState {
        fs: Arc::new(ArchiveFs::new(LocalFs)),
        policy: Arc::new(Policy::Roots(roots)),
        listings: Arc::new(ListingCache::default()),
        queue: Arc::new(IoQueue::new(IO_SLOTS)),
        thumbs: Arc::new(ThumbCache::new(data.join("thumbs"), THUMB_BUDGET)),
        settings_path: data.join("settings.json"),
        token: None,
        allowed_hosts: hosts,
        start: Arc::new(RwLock::new(None)),
        places: Vec::new(),
    }
}

async fn run() -> Result<(), String> {
    if env("PHOTOSEE_PASSWORD").is_some() {
        return Err("PHOTOSEE_PASSWORD is not supported yet (no login screen)".into());
    }
    let roots = parse_roots(&env("PHOTOSEE_ROOTS").unwrap_or_default())?;
    let port: u16 = env("PHOTOSEE_PORT")
        .map_or(Ok(8080), |p| p.parse())
        .map_err(|e| format!("PHOTOSEE_PORT: {e}"))?;
    let data = PathBuf::from(env("PHOTOSEE_DATA").unwrap_or_else(|| "data".into()));
    let web = PathBuf::from(env("PHOTOSEE_WEB").unwrap_or_else(|| "web".into()));
    if !web.join("index.html").is_file() {
        return Err(format!("web UI not found: {}/index.html", web.display()));
    }
    std::fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;
    let names: Vec<String> = roots
        .iter()
        .map(|r| format!("{}={}", r.name, r.dir.display()))
        .collect();
    let app = photosee_server::router(build_state(roots, &data), Some(web));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|e| format!("cannot listen on port {port}: {e}"))?;
    eprintln!(
        "photosee-server: listening on :{port}, roots {}",
        names.join(", ")
    );
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("photosee-server: {e}");
        std::process::exit(2);
    }
}
