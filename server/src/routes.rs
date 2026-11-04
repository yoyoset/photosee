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
        .route("/meta", get(meta::get))
        .route("/nav", get(nav::get))
        .route("/config", get(config::get).put(config::put))
        .route("/ui-state", get(ui_state::get).put(ui_state::put))
        // Without this, a mistyped /api/v1 path would fall through to the
        // static UI and answer 200 with index.html.
        .fallback(unknown_endpoint)
}

pub fn router(state: AppState, web_dir: Option<PathBuf>) -> Router {
    let mut app = Router::new().nest("/api/v1", api());
    if let Some(dir) = web_dir {
        // index.html answers every unknown path with 200: the UI is a
        // single page and may be opened at any address.
        let index = ServeFile::new(dir.join("index.html"));
        app = app.fallback_service(ServeDir::new(dir).fallback(index));
    }
    // Added last so the guard wraps the API, the static files and the
    // fallback alike.
    app.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::guard::check,
    ))
    .with_state(state)
}
