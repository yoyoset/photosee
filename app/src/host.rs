//! Window control for the page, over plain HTTP (not Tauri IPC).
//!
//! Why HTTP: the UI is served from `http://127.0.0.1:<port>`. Granting that
//! origin Tauri IPC would let any script that ever runs there call every
//! command, so the window has no capability for it; these four routes are
//! the whole surface instead. They sit behind the same guard layer as the
//! API (Host allow-list + session cookie), so a web page in the user's
//! browser cannot reach them (no cookie, SameSite=Strict).
//!
//! Why fullscreen is a route and not `requestFullscreen()` (measured with
//! `app/measure.mjs fullscreen`, WebView2 156 in Tauri 2.12.5): assumed the
//! HTML Fullscreen API only fills the webview, because wry has no code for
//! WebView2's ContainsFullScreenElementChanged. Measured: with a user
//! gesture it does make the *window* fullscreen (1295x837 -> 1707x960, the
//! screen size). Without one it throws "Permissions check failed", and the
//! UI's `start_fullscreen` setting (default on) runs at boot with no gesture.
//! So the route stays; it also makes close/quit possible, which the Fullscreen
//! API never could (`window.close()` is a no-op for a window scripts did not open).

use crate::perf;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use photosee_server::AppState;
use std::path::PathBuf;
use std::sync::Arc;

pub const BRIDGE_JS: &str = include_str!("bridge.js");

/// What the routes need from the shell. A trait so the routes can be tested
/// without a window.
pub trait WindowCtl: Send + Sync + 'static {
    fn is_fullscreen(&self) -> bool;
    /// Returns the state the window ended up in.
    fn set_fullscreen(&self, on: bool) -> bool;
    /// Hide, keep the process (resident mode).
    fn hide(&self);
    fn quit(&self);
    /// The native folder picker, modal to the window. Blocks until the user
    /// picks a folder or cancels (`None`).
    fn pick_folder(&self) -> Option<String>;
}

#[derive(Clone)]
struct Ctl {
    win: Arc<dyn WindowCtl>,
    perf: Option<Arc<PathBuf>>,
}

/// Routes `/api/v1/host/*`, already wrapped in the guard. `perf_out` adds the
/// measurement route (see perf.rs); it is `None` in normal runs.
pub fn routes(win: Arc<dyn WindowCtl>, state: AppState, perf_out: Option<PathBuf>) -> Router {
    let mut r = Router::new()
        .route(
            "/api/v1/host/fullscreen",
            get(fullscreen_get).post(fullscreen_set),
        )
        .route("/api/v1/host/close", post(close))
        .route("/api/v1/host/quit", post(quit))
        .route("/api/v1/host/pick-folder", post(pick_folder));
    if perf_out.is_some() {
        r = r.route("/api/v1/host/perf", post(perf_report));
    }
    let ctl = Ctl {
        win,
        perf: perf_out.map(Arc::new),
    };
    r.with_state(ctl)
        .layer(axum::middleware::from_fn_with_state(
            state,
            photosee_server::guard::check,
        ))
}

fn state_json(on: bool) -> Response {
    Json(serde_json::json!({ "on": on })).into_response()
}

fn bad_request(why: &str) -> Response {
    let body = serde_json::json!({ "error": { "code": "bad_request", "message": why } });
    (StatusCode::BAD_REQUEST, Json(body)).into_response()
}

async fn fullscreen_get(State(c): State<Ctl>) -> Response {
    state_json(c.win.is_fullscreen())
}

async fn fullscreen_set(State(c): State<Ctl>, body: Bytes) -> Response {
    let on = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("on")?.as_bool());
    match on {
        Some(on) => state_json(c.win.set_fullscreen(on)),
        None => bad_request("body must be {\"on\": true|false}"),
    }
}

async fn close(State(c): State<Ctl>) -> Response {
    c.win.hide();
    StatusCode::NO_CONTENT.into_response()
}

async fn quit(State(c): State<Ctl>) -> Response {
    c.win.quit();
    StatusCode::NO_CONTENT.into_response()
}

async fn pick_folder(State(c): State<Ctl>) -> Response {
    // The dialog blocks its thread for as long as the user looks at it.
    let win = c.win.clone();
    let path = tokio::task::spawn_blocking(move || win.pick_folder())
        .await
        .ok()
        .flatten();
    Json(serde_json::json!({ "path": path })).into_response()
}

async fn perf_report(State(c): State<Ctl>, body: Bytes) -> Response {
    if let (Some(path), Ok(v)) = (&c.perf, serde_json::from_slice::<serde_json::Value>(&body)) {
        perf::append(path, &v);
    }
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod host_tests;
