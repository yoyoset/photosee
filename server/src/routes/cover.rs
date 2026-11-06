//! GET /cover?p=DIR[&s=256]: the thumbnail of a folder's first image, for the
//! folder tiles of the grid. On a phone every comic folder was the same grey
//! icon (phone review, 2026-11-06). A folder without images of its own (a
//! series whose volumes are subfolders or .cbz files) shows its first
//! subfolder's cover, at most `DEPTH` levels down. None: 404 `not_found`, and
//! the tile keeps its icon.
//!
//! Same answer as /thumb for that image, ETag included; first = natural
//! order, as /list sorts.

use super::http::{Q, Reply, header_str, queued};
use crate::AppState;
use crate::queue::Priority;
use axum::extract::State;
use axum::http::{HeaderMap, header};
use photosee_core::{EntryKind, Error, ErrorCode, Result};

/// Folder levels to look down: series / volume / chapter.
const DEPTH: usize = 3;

pub async fn get(State(state): State<AppState>, q: Q, headers: HeaderMap) -> Reply {
    let p = q.need("p")?.to_owned();
    let edge = super::thumb::edge_of(&q)?;
    let inm = header_str(&headers, header::IF_NONE_MATCH).map(str::to_owned);
    let out = queued(&state.clone(), Priority::Thumb, move || {
        let first = first_image(&state, &p)?;
        super::thumb::make(&state, &first, edge, inm.as_deref())
    })
    .await?;
    Ok(super::thumb::respond(out))
}

/// Logical path of the first image in `logical` or, failing that, in its
/// first subfolder, and so on.
fn first_image(state: &AppState, logical: &str) -> Result<String> {
    let mut dir = state.policy.resolve(logical)?;
    for _ in 0..=DEPTH {
        let listing = super::list::cached(state, &dir)?;
        let img = listing
            .images
            .iter()
            .find(|e| matches!(e.kind, EntryKind::Image(_)));
        if let Some(e) = img {
            return state.policy.to_logical(&dir.join(&e.name));
        }
        match listing.dirs.first() {
            Some(d) => dir = dir.join(&d.name),
            None => break,
        }
    }
    Err(Error::new(
        ErrorCode::NotFound,
        "no image to use as a cover",
    ))
}
