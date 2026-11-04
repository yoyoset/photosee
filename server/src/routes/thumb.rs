//! GET /thumb?p=FILE[&s=256]: filmstrip thumbnail.

use super::http::{Q, Reply, cache_headers, etag, header_str, matches_etag, not_modified};
use super::http::{queued, require_file};
use crate::AppState;
use crate::queue::Priority;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use photosee_core::Result;
use photosee_core::thumb::{self, DEFAULT_EDGE};
use photosee_core::thumbcache::ThumbCache;

const MIN_EDGE: u32 = 64;
const MAX_EDGE: u32 = 512;

enum Outcome {
    NotModified(String),
    Jpeg { bytes: Vec<u8>, etag: String },
}

pub async fn get(State(state): State<AppState>, q: Q, headers: HeaderMap) -> Reply {
    let p = q.need("p")?.to_owned();
    let edge = q
        .number("s")?
        .unwrap_or(DEFAULT_EDGE)
        .clamp(MIN_EDGE, MAX_EDGE);
    let inm = header_str(&headers, header::IF_NONE_MATCH).map(str::to_owned);
    let out = queued(&state.clone(), Priority::Thumb, move || {
        make(&state, &p, edge, inm.as_deref())
    })
    .await?;
    Ok(match out {
        Outcome::NotModified(tag) => not_modified(&tag),
        Outcome::Jpeg { bytes, etag } => jpeg(bytes, &etag),
    })
}

fn make(state: &AppState, logical: &str, edge: u32, inm: Option<&str>) -> Result<Outcome> {
    let path = state.policy.resolve(logical)?;
    let stat = state.fs.stat(&path)?;
    require_file(&stat)?;
    let tag = etag(logical, &stat, edge, 0);
    if matches_etag(inm, &tag) {
        return Ok(Outcome::NotModified(tag));
    }
    let key = ThumbCache::key(&path, &stat, edge);
    let bytes = match state.thumbs.get(&key) {
        Some(hit) => hit,
        None => {
            let made = thumb::make(&*state.fs, &path, edge)?;
            // A cache that cannot write only costs a regeneration later.
            let _ = state.thumbs.put(&key, &made.bytes);
            made.bytes
        }
    };
    Ok(Outcome::Jpeg { bytes, etag: tag })
}

fn jpeg(bytes: Vec<u8>, tag: &str) -> Response {
    let mut resp = bytes.into_response();
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    cache_headers(&mut resp, tag);
    resp
}
