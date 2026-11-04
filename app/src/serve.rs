//! The embedded server: state, socket, and where the web UI lives.

use axum::Router;
use photosee_core::io::LocalFs;
use photosee_core::thumbcache::ThumbCache;
use photosee_server::AppState;
use photosee_server::cache::ListingCache;
use photosee_server::paths::Policy;
use photosee_server::queue::IoQueue;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Concurrent filesystem jobs. Same figure the server tests use; a NAS gains
/// nothing from more.
const IO_SLOTS: usize = 4;
/// Disk budget for cached thumbnails.
const THUMB_BUDGET: u64 = 256 << 20;

/// Bound synchronously so the window URL is known at once; serving starts
/// on the tokio runtime in parallel with WebView2 creation (the M0 timeline
/// is why: WebView2 is ~645 ms of the ~1 s cold start).
pub fn bind() -> Result<(TcpListener, u16), String> {
    let l =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("cannot open a local port: {e}"))?;
    l.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    Ok((l, port))
}

pub fn allowed_hosts(port: u16) -> Vec<String> {
    vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")]
}

/// A file with this name next to the exe makes the copy portable ("green"):
/// every byte the app writes stays in `<exe dir>\data`, nothing in the user
/// profile, so deleting the folder removes all traces.
pub const PORTABLE_MARKER: &str = "portable";

/// The exe's folder when this is a portable copy.
pub fn portable_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    dir.join(PORTABLE_MARKER)
        .is_file()
        .then(|| dir.to_path_buf())
}

/// Settings and thumbnail cache: `<exe dir>\data` for a portable copy,
/// otherwise `%LOCALAPPDATA%\PhotoSee`.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = portable_dir() {
        return dir.join("data");
    }
    std::env::var_os("LOCALAPPDATA")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("PhotoSee")
}

pub fn build_state(
    token: String,
    hosts: Vec<String>,
    start: Option<String>,
    data: &Path,
    places: Vec<photosee_server::state::Place>,
) -> AppState {
    AppState {
        fs: Arc::new(LocalFs),
        // Not a sandbox: the user may open any file they can read. The
        // token + Host check is what keeps web pages out (guard.rs).
        policy: Arc::new(Policy::AnyLocal),
        listings: Arc::new(ListingCache::default()),
        queue: Arc::new(IoQueue::new(IO_SLOTS)),
        thumbs: Arc::new(ThumbCache::new(data.join("thumbs"), THUMB_BUDGET)),
        settings_path: data.join("settings.json"),
        token: Some(token),
        allowed_hosts: hosts,
        start: Arc::new(std::sync::RwLock::new(start)),
        places,
    }
}

/// `PHOTOSEE_WEB`, else `web/` next to the exe (the shipped layout), else
/// `web/dist` of the checkout (so `target/release/photosee.exe` works in dev).
pub fn find_web_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().unwrap_or(Path::new("."));
    let mut tried = Vec::new();
    let env = std::env::var_os("PHOTOSEE_WEB").map(PathBuf::from);
    for cand in env
        .into_iter()
        .chain([dir.join("web"), dir.join("../../web/dist")])
    {
        if cand.join("index.html").is_file() {
            return Ok(cand);
        }
        tried.push(cand.display().to_string());
    }
    Err(format!(
        "The PhotoSee web UI was not found. Looked in:\n{}",
        tried.join("\n")
    ))
}

pub fn spawn(listener: TcpListener, router: Router) {
    tauri::async_runtime::spawn(async move {
        if let Ok(l) = tokio::net::TcpListener::from_std(listener) {
            let _ = axum::serve(l, router).await;
        }
    });
}
