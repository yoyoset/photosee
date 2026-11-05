//! GET /session: start-up facts.

use super::http::{Reply, queued, require_dir};
use crate::AppState;
use crate::paths::Policy;
use crate::queue::Priority;
use crate::state::{CurrentUser, Place};
use axum::Extension;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use photosee_core::{API_VERSION, EntryKind, Error, ErrorCode, Result};
use serde::Serialize;

const PLATFORM: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "android") {
    "android"
} else if cfg!(target_os = "ios") {
    "ios"
} else {
    "linux"
};

#[derive(Serialize)]
struct Start {
    dir: String,
    file: Option<String>,
}

#[derive(Serialize)]
struct Session {
    api_version: u32,
    mode: &'static str,
    platform: &'static str,
    start: Option<Start>,
    /// Top-level places for the browser (docs/api.md §session).
    roots: Vec<Place>,
    /// Multi-user server: who is logged in (shows "Switch user"). Else null.
    user: Option<String>,
}

pub async fn get(State(state): State<AppState>, user: Option<Extension<CurrentUser>>) -> Reply {
    // A poisoned lock only means a writer panicked; the value is still a path.
    let start_path = state
        .start
        .read()
        .map_or_else(|e| e.into_inner().clone(), |g| g.clone());
    let start = match start_path {
        // The only IO here: telling a file from a folder. On a NAS that is
        // a round trip, so it goes through the queue like everything else.
        Some(p) => {
            let st = state.clone();
            Some(queued(&state, Priority::Current, move || start_of(&st, &p)).await?)
        }
        None => None,
    };
    let (mode, roots) = match &*state.policy {
        Policy::AnyLocal => ("desktop", state.places.clone()),
        Policy::Roots(roots) => (
            "server",
            roots
                .iter()
                .map(|r| Place {
                    name: r.name.clone(),
                    path: format!("/{}", r.name),
                    kind: "root",
                })
                .collect(),
        ),
    };
    Ok(Json(Session {
        api_version: API_VERSION,
        mode,
        platform: PLATFORM,
        start,
        roots,
        user: user.map(|Extension(u)| u.0),
    })
    .into_response())
}

/// A file opens its folder with the file selected; a folder opens as is.
/// `file` is the file's logical path (what `/list` calls `path`), so the UI
/// can find it in the listing without splitting any path itself.
fn start_of(state: &AppState, logical: &str) -> Result<Start> {
    let phys = state.policy.resolve(logical)?;
    let entry = state.fs.stat(&phys)?;
    if matches!(entry.kind, EntryKind::Dir) {
        require_dir(&entry)?;
        return Ok(Start {
            dir: state.policy.to_logical(&phys)?,
            file: None,
        });
    }
    let parent = phys
        .parent()
        .ok_or_else(|| Error::new(ErrorCode::NotFound, "start file has no folder"))?;
    Ok(Start {
        dir: state.policy.to_logical(parent)?,
        file: Some(state.policy.to_logical(&phys)?),
    })
}
