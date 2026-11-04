//! GET and PUT /config: the six user settings.
//!
//! The settings file is local app data, never a NAS path, so these skip the
//! IO queue: a hung share that pins every queue slot must not stop the
//! settings from loading.

use super::http::{Fail, Reply, bad_request};
use crate::AppState;
use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::response::IntoResponse;
use photosee_core::config::{self, Settings};
use photosee_core::{Error, ErrorCode};

async fn blocking<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Result<T, Fail> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|_| Fail::from(Error::new(ErrorCode::Io, "settings job failed")))
}

pub async fn get(State(state): State<AppState>) -> Reply {
    let settings = blocking(move || config::load(&state.settings_path)).await?;
    Ok(Json(settings).into_response())
}

pub async fn put(State(state): State<AppState>, body: Bytes) -> Reply {
    let parsed: Settings = serde_json::from_slice(&body)
        .map_err(|e| bad_request(format!("settings body is not valid: {e}")))?;
    let settings = parsed.normalized();
    let saved = settings.clone();
    blocking(move || config::save(&state.settings_path, &saved)).await??;
    // The normalized object, so the UI shows what was actually stored.
    Ok(Json(settings).into_response())
}
