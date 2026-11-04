//! Plumbing shared by the endpoints: query parsing that answers with the API
//! error body, the failure type, ETags and conditional requests.

use crate::AppState;
use crate::error::{ApiError, error_response};
use crate::queue::Priority;
use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use photosee_core::{Entry, EntryKind, Error, Format, Result};
use std::collections::HashMap;

/// A failed request, already shaped as the API's JSON error body.
/// Boxed: a `Response` is large and every handler returns this in `Result`.
pub struct Fail(pub Box<Response>);

impl From<Error> for Fail {
    fn from(e: Error) -> Self {
        Self(Box::new(ApiError(e).into_response()))
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> Response {
        *self.0
    }
}

pub type Reply = std::result::Result<Response, Fail>;

/// The API has no dedicated "bad request" code. Callers get the
/// `unsupported` body (as docs/api.md's callers expect) under status 400,
/// so a malformed request is distinguishable from an unsupported format.
pub fn bad_request(why: impl Into<String>) -> Fail {
    with_status(&Error::unsupported(why), StatusCode::BAD_REQUEST)
}

/// The JSON error body for `e` under a different HTTP status.
pub fn with_status(e: &Error, status: StatusCode) -> Fail {
    let mut resp = error_response(e);
    *resp.status_mut() = status;
    Fail(Box::new(resp))
}

/// Decoded query string. Own extractor so a bad query gets the API's error
/// body instead of axum's plain-text rejection.
pub struct Q(HashMap<String, String>);

impl<S: Send + Sync> FromRequestParts<S> for Q {
    type Rejection = Fail;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> std::result::Result<Self, Fail> {
        Query::<HashMap<String, String>>::try_from_uri(&parts.uri)
            .map(|Query(m)| Self(m))
            .map_err(|e| bad_request(format!("bad query string: {e}")))
    }
}

impl Q {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// A required parameter.
    pub fn need(&self, key: &str) -> std::result::Result<&str, Fail> {
        self.get(key)
            .ok_or_else(|| bad_request(format!("missing query parameter `{key}`")))
    }

    /// An optional unsigned integer; present but not a number is a 400.
    pub fn number(&self, key: &str) -> std::result::Result<Option<u32>, Fail> {
        self.get(key)
            .map(|v| {
                v.parse::<u32>()
                    .map_err(|_| bad_request(format!("`{key}` must be a non-negative integer")))
            })
            .transpose()
    }
}

/// Runs one filesystem job at `prio` through the shared queue.
pub async fn queued<T, F>(state: &AppState, prio: Priority, job: F) -> std::result::Result<T, Fail>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    Ok(state.queue.run_try(prio, job).await?)
}

/// `/img`, `/thumb` and `/meta` take files; `stat` also succeeds on folders.
pub fn require_file(entry: &Entry) -> Result<()> {
    match entry.kind {
        EntryKind::Image(_) => Ok(()),
        EntryKind::Dir => Err(Error::unsupported("path is a directory, not an image")),
    }
}

pub fn require_dir(entry: &Entry) -> Result<()> {
    match entry.kind {
        EntryKind::Dir => Ok(()),
        EntryKind::Image(_) => Err(Error::unsupported("path is a file, not a directory")),
    }
}

/// 64-bit FNV-1a, rendered as a quoted strong ETag. A stable hash of the
/// inputs, not std's `DefaultHasher`, so ETags survive a toolchain bump and
/// browsers keep their caches.
pub fn etag(logical: &str, entry: &Entry, w: u32, h: u32) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(logical.as_bytes());
    // Fixed-width fields after a separator keep "ab"+1 distinct from "a"+b1.
    feed(&[0xFF]);
    feed(&entry.size.to_le_bytes());
    feed(&entry.mtime_ms.to_le_bytes());
    feed(&w.to_le_bytes());
    feed(&h.to_le_bytes());
    format!("\"{hash:016x}\"")
}

/// `If-None-Match` against our strong ETag: `*`, or any listed tag equal to
/// it, weak prefix ignored (RFC 9110 weak comparison for GET).
pub fn matches_etag(if_none_match: Option<&str>, etag: &str) -> bool {
    if_none_match.is_some_and(|list| {
        list.split(',')
            .map(str::trim)
            .any(|t| t == "*" || t.strip_prefix("W/").unwrap_or(t) == etag)
    })
}

pub fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// Client-side caching shared by `/img` and `/thumb`: the bytes behind one
/// URL change only when the file does, and the ETag covers that.
pub fn cache_headers(resp: &mut Response, etag: &str) {
    let h = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(etag) {
        h.insert(header::ETAG, v);
    }
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=60"),
    );
}

pub fn not_modified(etag: &str) -> Response {
    let mut resp = StatusCode::NOT_MODIFIED.into_response();
    cache_headers(&mut resp, etag);
    resp
}

/// Formats the client can decode natively, from its `Accept` header. Only
/// explicit media types count: `image/*` says nothing about WebP or AVIF.
/// `q=0` means "not acceptable".
pub fn native_formats(accept: Option<&str>) -> Vec<Format> {
    const KNOWN: [(&str, Format); 4] = [
        ("image/webp", Format::Webp),
        ("image/avif", Format::Avif),
        ("image/jxl", Format::Jxl),
        ("image/heic", Format::Heic),
    ];
    let mut out = Vec::new();
    for item in accept.unwrap_or_default().split(',') {
        let mut parts = item.split(';');
        let mime = parts.next().unwrap_or_default().trim();
        let refused = parts.any(|p| {
            p.trim()
                .strip_prefix("q=")
                .is_some_and(|q| q.trim().parse::<f32>().is_ok_and(|q| q <= 0.0))
        });
        if refused {
            continue;
        }
        if let Some((_, f)) = KNOWN.iter().find(|(m, _)| mime.eq_ignore_ascii_case(m)) {
            out.push(*f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_lists_only_named_types() {
        let got = native_formats(Some("image/avif,image/webp;q=0.8,image/*,*/*;q=0.5"));
        assert_eq!(got, [Format::Avif, Format::Webp]);
        assert!(native_formats(Some("image/webp;q=0")).is_empty());
        assert!(native_formats(None).is_empty());
    }

    #[test]
    fn if_none_match_forms() {
        let t = "\"abc\"";
        assert!(matches_etag(Some("\"abc\""), t));
        assert!(matches_etag(Some("W/\"abc\""), t));
        assert!(matches_etag(Some("\"x\", \"abc\""), t));
        assert!(matches_etag(Some("*"), t));
        assert!(!matches_etag(Some("\"abd\""), t));
        assert!(!matches_etag(None, t));
    }
}
