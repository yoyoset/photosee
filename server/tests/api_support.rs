//! Shared fixtures for the api_* tests (included with `#[path]`). Also
//! compiled as its own empty test target, hence the allows.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]
#![allow(dead_code, reason = "each api_* test uses a different subset")]

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use photosee_core::io::FsBackend;
use photosee_core::testfs::MemFs;
use photosee_core::thumbcache::ThumbCache;
use photosee_server::AppState;
use photosee_server::cache::ListingCache;
use photosee_server::paths::{Policy, Root};
use photosee_server::queue::IoQueue;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

pub struct Resp {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Resp {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("not JSON ({e}): {:?}", String::from_utf8_lossy(&self.body)))
    }

    pub fn header(&self, name: &str) -> &str {
        self.headers.get(name).map_or("", |v| v.to_str().unwrap())
    }
}

pub async fn send(app: &Router, uri: &str, headers: &[(&str, &str)]) -> Resp {
    let mut req = Request::builder().uri(uri);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (parts, body) = resp.into_parts();
    let body = axum::body::to_bytes(body, 64 << 20).await.unwrap();
    Resp {
        status: parts.status,
        headers: parts.headers,
        body: body.to_vec(),
    }
}

pub async fn get(app: &Router, uri: &str) -> Resp {
    send(app, uri, &[]).await
}

pub fn golden(name: &str) -> serde_json::Value {
    let path = format!("{}/../contracts/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub struct Env {
    pub state: AppState,
    pub app: Router,
    /// Holds the settings file and the thumbnail cache.
    pub tmp: TempDir,
}

pub fn state_with(fs: Arc<dyn FsBackend>, policy: Policy, queue: IoQueue) -> (AppState, TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState {
        fs,
        policy: Arc::new(policy),
        listings: Arc::new(ListingCache::default()),
        queue: Arc::new(queue),
        thumbs: Arc::new(ThumbCache::new(tmp.path().join("thumbs"), 1 << 20)),
        settings_path: tmp.path().join("settings.json"),
        token: None,
        allowed_hosts: vec![],
        start: Default::default(),
    };
    (state, tmp)
}

pub fn photos_policy() -> Policy {
    Policy::Roots(vec![Root {
        name: "photos".into(),
        dir: PathBuf::from("/photos"),
    }])
}

pub fn env_with(fs: Arc<dyn FsBackend>, queue: IoQueue) -> Env {
    let (state, tmp) = state_with(fs, photos_policy(), queue);
    let app = photosee_server::router(state.clone(), None);
    Env { state, app, tmp }
}

/// The fixture tree of docs/api.md §Golden-file fixture tree.
pub fn golden_tree() -> MemFs {
    let fs = MemFs::new();
    fs.file("/photos/a10.jpg", b"x")
        .file("/photos/a2.jpg", b"x")
        .file("/photos/.hidden.jpg", b"x")
        .file("/photos/notes.txt", b"x")
        .file("/photos/2023/b.png", b"x")
        .dir("/photos/2024")
        .file("/photos/2025/c.jpg", b"x");
    fs
}

pub fn golden_env() -> Env {
    env_with(Arc::new(golden_tree()), IoQueue::new(4))
}

/// 24-bit BMP, `w`x`h`, a gradient. BMP is native to every client, so it
/// shows both passthrough (small request) and transcode (large image).
pub fn bmp(w: u32, h: u32) -> Vec<u8> {
    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut b = b"BM".to_vec();
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&w.to_le_bytes());
    b.extend_from_slice(&h.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]); // no compression, sizes and palette unused
    for y in 0..h {
        for x in 0..w {
            b.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, 128]);
        }
        b.resize(b.len() + (row - w * 3) as usize, 0);
    }
    b
}
