//! `/api/v1/*` handlers. Package G. Every response shape is pinned by the
//! golden files in /contracts and docs/api.md. One file per endpoint under
//! `routes/`; shared plumbing (query parsing, ETags, error mapping) is in
//! `routes/http.rs`.
//!
//! All filesystem work runs inside one queued job per request: resolve the
//! path, stat, decide, read. The job is the unit of priority, of the 5 s
//! timeout and of cancellation, so a request never holds a slot between
//! two steps and never queues twice.

mod config;
mod cover;
mod http;
mod img;
mod list;
mod meta;
mod nav;
mod session;
mod thumb;
mod ui_state;

use crate::AppState;
use crate::error::ApiError;
use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{Next, from_fn};
use axum::response::Response;
use axum::routing::get;
use photosee_core::{Error, ErrorCode};
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};

async fn unknown_endpoint() -> ApiError {
    ApiError(Error::new(ErrorCode::NotFound, "no such endpoint"))
}

fn api() -> Router<AppState> {
    Router::new()
        .route("/session", get(session::get))
        .route("/list", get(list::get))
        .route("/img", get(img::get))
        .route("/thumb", get(thumb::get))
        .route("/cover", get(cover::get))
        .route("/meta", get(meta::get))
        .route("/nav", get(nav::get))
        .route("/config", get(config::get).put(config::put))
        .route("/ui-state", get(ui_state::get).put(ui_state::put))
        // Without this, a mistyped /api/v1 path would fall through to the
        // static UI and answer 200 with index.html.
        .fallback(unknown_endpoint)
}

/// Vite names bundle files by content hash, so `/assets/*` never changes
/// under a name; index.html changes on every deploy and names the new
/// bundle, so it must be revalidated (ETag, 304) every time. Without any
/// Cache-Control the browser guessed a lifetime from Last-Modified, and a
/// phone kept the old UI after the Docker image was redeployed (2026-11-05).
async fn ui_cache(req: Request, next: Next) -> Response {
    let hashed = req.uri().path().starts_with("/assets/");
    let mut resp = next.run(req).await;
    let st = resp.status();
    if st.is_success() || st == StatusCode::NOT_MODIFIED {
        let v = if hashed {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        resp.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(v));
    }
    resp
}

/// The web UI as static files. index.html answers every unknown path with
/// 200: the UI is a single page and may be opened at any address.
pub fn ui_service(dir: PathBuf) -> Router {
    let index = ServeFile::new(dir.join("index.html"));
    Router::new()
        .fallback_service(ServeDir::new(dir).fallback(index))
        .layer(from_fn(ui_cache))
}

pub fn router(state: AppState, web_dir: Option<PathBuf>) -> Router {
    let mut app = Router::new().nest("/api/v1", api());
    if let Some(dir) = web_dir {
        // index.html answers every unknown path with 200: the UI is a
        // single page and may be opened at any address.
        app = app.fallback_service(ui_service(dir));
    }
    // Added last so the guard wraps the API, the static files and the
    // fallback alike.
    app.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::guard::check,
    ))
    .with_state(state)
}
