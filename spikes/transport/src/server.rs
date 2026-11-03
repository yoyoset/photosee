//! Embedded axum server: pages, transport (a), the cold-start image, results.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Query, State as AxState};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::OnceCell;
use tokio_util::io::ReaderStream;

use crate::clock::{Timeline, epoch_ms};

pub struct State {
    pub bench: bool,
    pub samples: PathBuf,
    pub first_path: Option<PathBuf>,
    /// Bytes of the CLI image, read in parallel with WebView2 init.
    pub first: OnceCell<Option<Bytes>>,
    pub timeline: Timeline,
    pub created_epoch: f64,
    pub launched_at: Option<f64>,
    pub out: PathBuf,
    pub hold: Duration,
    pub app: OnceLock<tauri::AppHandle>,
}

pub type Shared = Arc<State>;

/// Maps a sample name to a path inside the samples dir, refusing traversal.
pub fn sample_path(samples: &Path, name: &str) -> Option<PathBuf> {
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    ok.then(|| samples.join(format!("{name}.jpg")))
}

pub async fn first_bytes(st: &State) -> Option<Bytes> {
    st.first
        .get_or_init(|| async {
            let p = st.first_path.as_ref()?;
            let b = tokio::fs::read(p).await.ok()?;
            st.timeline.mark("first_file_read");
            Some(Bytes::from(b))
        })
        .await
        .clone()
}

pub fn router(st: Shared) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/bench.js", get(|| async { js(include_str!("../static/bench.js")) }))
        .route("/prefetch.js", get(|| async { js(include_str!("../static/prefetch.js")) }))
        .route("/file", get(file))
        .route("/first", get(first))
        .route("/result", post(result))
        .with_state(st)
}

fn no_store(mut r: Response, ctype: &'static str) -> Response {
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(ctype));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

fn js(src: &'static str) -> Response {
    no_store(src.into_response(), "text/javascript")
}

async fn index(AxState(st): AxState<Shared>) -> Response {
    let page = if st.bench { include_str!("../static/bench.html") } else { include_str!("../static/cold.html") };
    no_store(page.into_response(), "text/html; charset=utf-8")
}

/// Transport (a): streaming `tokio::fs` read.
async fn file(AxState(st): AxState<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let Some(path) = q.get("n").and_then(|n| sample_path(&st.samples, n)) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(f) = tokio::fs::File::open(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
    // Pitfall: ReaderStream's default 4 KiB chunks mean ~7500 chunks for a
    // 30 MB file; 256 KiB keeps per-chunk overhead out of the measurement.
    let body = Body::from_stream(ReaderStream::with_capacity(f, 256 * 1024));
    let mut r = no_store(body.into_response(), "image/jpeg");
    r.headers_mut().insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    r
}

async fn first(AxState(st): AxState<Shared>) -> Response {
    st.timeline.mark("first_requested");
    match first_bytes(&st).await {
        Some(b) => no_store(b.into_response(), "image/jpeg"),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The page posts its numbers here; Rust adds its timeline and writes the file.
async fn result(AxState(st): AxState<Shared>, Json(page): Json<serde_json::Value>) -> StatusCode {
    st.timeline.mark("result_received");
    let doc = serde_json::json!({
        "created_epoch": st.created_epoch,
        "launched_at": st.launched_at,
        "rust_marks_epoch": st.timeline.to_json(),
        "page": page,
        "written_epoch": epoch_ms(),
    });
    let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    if tokio::fs::write(&st.out, text).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    let st2 = st.clone();
    tokio::spawn(async move {
        tokio::time::sleep(st2.hold).await;
        if let Some(app) = st2.app.get() {
            app.exit(0);
        }
    });
    StatusCode::NO_CONTENT
}
