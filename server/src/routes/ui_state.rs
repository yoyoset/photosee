//! GET and PUT /ui-state: the browser's layout and history (tree visible and
//! width, thumbnail size, view mode, sort, recent folders, network places).
//!
//! Opaque to the server: the UI owns the schema, the server only stores one
//! JSON object next to the settings. It cannot live in localStorage, because
//! the desktop server listens on a random port, so the page's origin, and
//! with it localStorage, changes on every launch.

use super::http::{Fail, Reply, bad_request};
use crate::AppState;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use photosee_core::{Error, ErrorCode};
use std::path::PathBuf;

/// Layout plus a few dozen paths; anything bigger is a bug or an attack.
pub const MAX_BYTES: usize = 64 * 1024;

fn file(state: &AppState) -> PathBuf {
    state.settings_path.with_file_name("ui-state.json")
}

async fn blocking<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Result<T, Fail> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|_| Fail::from(Error::new(ErrorCode::Io, "ui-state job failed")))
}

/// `{}` when nothing was saved yet or the file is unreadable: losing a layout
/// must never stop the UI from starting.
pub async fn get(State(state): State<AppState>) -> Reply {
    let path = file(&state);
    let body = blocking(move || {
        std::fs::read(&path)
            .ok()
            .filter(|b| {
                b.len() <= MAX_BYTES
                    && serde_json::from_slice::<serde_json::Value>(b).is_ok_and(|v| v.is_object())
            })
            .unwrap_or_else(|| b"{}".to_vec())
    })
    .await?;
    Ok(([(header::CONTENT_TYPE, "application/json")], body).into_response())
}

pub async fn put(State(state): State<AppState>, body: Bytes) -> Reply {
    if body.len() > MAX_BYTES {
        return Err(bad_request(format!("ui-state is over {MAX_BYTES} bytes")));
    }
    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| bad_request(format!("ui-state is not JSON: {e}")))?;
    if !value.is_object() {
        return Err(bad_request("ui-state must be a JSON object"));
    }
    let path = file(&state);
    blocking(move || photosee_core::config::write_atomic(&path, &body)).await??;
    Ok(axum::http::StatusCode::NO_CONTENT.into_response())
}
