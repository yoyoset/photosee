//! GET /meta?p=FILE: facts for fitting and the info overlay, no pixels.

use super::http::{Q, Reply, queued, require_file};
use crate::AppState;
use crate::queue::Priority;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use photosee_core::{ExifSummary, Format, Orientation, Result, meta, pipeline};
use serde::Serialize;

#[derive(Serialize)]
struct Out {
    fmt: Format,
    /// Display size: after orientation, because that is what is on screen.
    width: u32,
    height: u32,
    orientation: Orientation,
    animated: bool,
    size: u64,
    mtime_ms: i64,
    exif: ExifSummary,
}

pub async fn get(State(state): State<AppState>, q: Q) -> Reply {
    let p = q.need("p")?.to_owned();
    let out = queued(&state.clone(), Priority::Current, move || load(&state, &p)).await?;
    Ok(Json(out).into_response())
}

fn load(state: &AppState, logical: &str) -> Result<Out> {
    let path = state.policy.resolve(logical)?;
    let stat = state.fs.stat(&path)?;
    require_file(&stat)?;
    let info = pipeline::info(&*state.fs, &path)?;
    let exif = meta::exif_summary(&*state.fs, &path)?;
    let shown = info.display_size();
    Ok(Out {
        fmt: info.format,
        width: shown.w,
        height: shown.h,
        orientation: info.orientation,
        animated: info.animated,
        size: stat.size,
        mtime_ms: stat.mtime_ms,
        exif,
    })
}
