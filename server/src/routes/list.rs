//! GET /list?p=DIR

use super::http::{Q, Reply, queued, require_dir};
use crate::AppState;
use crate::paths::Policy;
use crate::queue::Priority;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use photosee_core::{Entry, EntryKind, Format, Listing, Result, scan};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct ImageItem {
    name: String,
    path: String,
    fmt: Format,
    size: u64,
    mtime_ms: i64,
}

#[derive(Serialize)]
struct DirItem {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct Out {
    dir: String,
    parent: Option<String>,
    images: Vec<ImageItem>,
    dirs: Vec<DirItem>,
}

pub async fn get(State(state): State<AppState>, q: Q) -> Reply {
    let p = q.need("p")?.to_owned();
    let out = queued(&state.clone(), Priority::Current, move || list(&state, &p)).await?;
    Ok(Json(out).into_response())
}

fn list(state: &AppState, logical: &str) -> Result<Out> {
    let dir = state.policy.resolve(logical)?;
    let listing = cached(state, &dir)?;
    shape(&state.policy, &dir, listing)
}

/// The scan of a resolved directory, through the listing cache. The
/// directory's own stat is the cache key: one cheap call decides whether the
/// (possibly seconds-long) scan can be skipped.
pub(super) fn cached(state: &AppState, dir: &Path) -> Result<Listing> {
    let stat = state.fs.stat(dir)?;
    require_dir(&stat)?;
    Ok(match state.listings.get(dir, stat.mtime_ms) {
        Some(hit) => hit,
        None => {
            let fresh = scan::list(&*state.fs, dir)?;
            state.listings.put(dir, stat.mtime_ms, fresh.clone());
            fresh
        }
    })
}

fn shape(policy: &Policy, dir: &Path, listing: Listing) -> Result<Out> {
    let path_of = |e: &Entry| policy.to_logical(&dir.join(&e.name));
    let images = listing
        .images
        .iter()
        .filter_map(|e| match e.kind {
            EntryKind::Image(fmt) => Some((e, fmt)),
            EntryKind::Dir => None,
        })
        .map(|(e, fmt)| {
            Ok(ImageItem {
                name: e.name.clone(),
                path: path_of(e)?,
                fmt,
                size: e.size,
                mtime_ms: e.mtime_ms,
            })
        })
        .collect::<Result<_>>()?;
    let dirs = listing
        .dirs
        .iter()
        .map(|e| {
            Ok(DirItem {
                name: e.name.clone(),
                path: path_of(e)?,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Out {
        dir: policy.to_logical(dir)?,
        parent: parent_of(policy, dir)?,
        images,
        dirs,
    })
}

/// `None` at a root: the UI must not climb above where the server lets it.
fn parent_of(policy: &Policy, dir: &Path) -> Result<Option<String>> {
    let at_root = match policy {
        Policy::AnyLocal => false,
        Policy::Roots(roots) => roots.iter().any(|r| r.dir == dir),
    };
    match dir.parent() {
        Some(p) if !at_root => policy.to_logical(p).map(Some),
        _ => Ok(None),
    }
}
