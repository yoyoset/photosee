//! `/api/v1/*` handlers. Package G. Every response shape is pinned by the
//! golden files in /contracts and docs/api.md.

use crate::AppState;
use axum::Router;
use std::path::PathBuf;

pub fn router(state: AppState, web_dir: Option<PathBuf>) -> Router {
    let _ = web_dir;
    Router::new()
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::guard::check,
        ))
        .with_state(state)
}
