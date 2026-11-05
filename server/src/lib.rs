//! PhotoSee HTTP server: the single API used by the desktop shell (embedded,
//! loopback only) and the Docker build. Contract: docs/api.md.

pub mod accounts;
pub mod admin;
pub mod cache;
pub mod error;
pub mod guard;
pub mod library;
pub mod multi;
pub mod paths;
pub mod queue;
pub mod range;
pub mod routes;
pub mod sessions;
pub mod state;

pub use state::AppState;

/// Full router: `/api/v1/*` plus the web UI as static files from `web_dir`.
pub fn router(state: AppState, web_dir: Option<std::path::PathBuf>) -> axum::Router {
    routes::router(state, web_dir)
}
