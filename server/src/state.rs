//! Shared server state. Contract file: orchestrator only.

use crate::{cache::ListingCache, paths::Policy, queue::IoQueue};
use photosee_core::io::FsBackend;
use photosee_core::thumbcache::ThumbCache;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// A top-level place the browser offers: a Docker root, a drive, or a
/// known folder such as Pictures. Supplied by whoever builds the state (the
/// desktop shell knows drives; the server only knows its roots).
#[derive(Clone, Debug, serde::Serialize)]
pub struct Place {
    pub name: String,
    pub path: String,
    /// `root` (Docker), `folder` (known folder), `drive`, `network`.
    pub kind: &'static str,
}

/// Multi-user mode: the logged-in user, put into the request extensions by
/// the dispatcher (multi.rs). Absent in single-user and desktop mode.
#[derive(Clone, Debug)]
pub struct CurrentUser(pub String);

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
    /// Desktop: drives and known folders for the browser's tree. Ignored
    /// under `Policy::Roots`, where the roots are the places.
    pub places: Vec<Place>,
}
