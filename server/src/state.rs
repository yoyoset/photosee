//! Shared server state. Contract file: orchestrator only.

use crate::{cache::ListingCache, paths::Policy, queue::IoQueue};
use photosee_core::io::FsBackend;
use photosee_core::thumbcache::ThumbCache;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub fs: Arc<dyn FsBackend>,
    pub policy: Arc<Policy>,
    pub listings: Arc<ListingCache>,
    pub queue: Arc<IoQueue>,
    pub thumbs: Arc<ThumbCache>,
    pub settings_path: PathBuf,
    /// Session token. Desktop: random per launch, set as an HttpOnly cookie
    /// by `/` and required on every `/api` call. Docker: derived from
    /// PHOTOSEE_PASSWORD, or `None` for an open LAN server.
    pub token: Option<String>,
    /// Accepted `Host` header values (DNS-rebinding defence).
    pub allowed_hosts: Vec<String>,
    /// What `/api/v1/session` reports as the starting point. Mutable for
    /// resident mode: a second launch sets the new path and reloads the
    /// already-warm webview, which then boots from it.
    pub start: Arc<RwLock<Option<String>>>,
}
