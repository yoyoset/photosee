//! GET /img?p=FILE[&w=W&h=H][&prio=current|prefetch]
//!
//! One queued job does everything that touches the filesystem: resolve, stat,
//! the `If-None-Match` check, and either the transcode or the byte read.
//! (The brief asked for the 304 check before queueing, but an ETag built from
//! size + mtime needs a stat, which on a NAS is itself IO. The job checks
//! right after that stat and before any decode or read, so a 304 costs one
//! stat and no pixels.)

use super::http::{Fail, Q, Reply, bad_request, cache_headers, etag, header_str};
use super::http::{matches_etag, native_formats, not_modified, queued, require_file, with_status};
use crate::AppState;
use crate::queue::Priority;
use crate::range::{RangeReq, classify};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use photosee_core::io::{ReadAt, read_exact_at};
use photosee_core::pipeline::{self, Rendered};
use photosee_core::{Delivery, Error, ErrorCode, Format, Result, Size};

/// Largest box edge honoured, and the box used when `w`/`h` are omitted.
/// 2^24 px is beyond any image the decoder accepts (200 MP) on its long side
/// for all real photos, and keeps the resize arithmetic far from overflow
/// even for hostile `w`/`h` values.
const MAX_EDGE: u32 = 1 << 24;

const DELIVERY: HeaderName = HeaderName::from_static("x-photosee-delivery");

struct Req {
    logical: String,
    /// `None` = original pixels.
    boxed: Option<(u32, u32)>,
    prio: Priority,
    native: Vec<Format>,
    range: Option<String>,
    if_none_match: Option<String>,
}

enum Outcome {
    NotModified(String),
    Unsatisfiable { total: u64 },
    Body(Body),
}

struct Body {
    etag: String,
    mime: &'static str,
    delivery: &'static str,
    bytes: Vec<u8>,
    /// `(first, last, total)` when this is a 206.
    slice: Option<(u64, u64, u64)>,
    ranges: bool,
}

pub async fn get(State(state): State<AppState>, q: Q, headers: HeaderMap) -> Reply {
    let req = parse(&q, &headers)?;
    let prio = req.prio;
    let out = queued(&state.clone(), prio, move || serve(&state, &req)).await?;
    Ok(match out {
        Outcome::NotModified(tag) => not_modified(&tag),
        Outcome::Unsatisfiable { total } => unsatisfiable(total),
        Outcome::Body(body) => respond(body),
    })
}

fn parse(q: &Q, headers: &HeaderMap) -> std::result::Result<Req, Fail> {
    let boxed = match (q.number("w")?, q.number("h")?) {
        (None, None) => None,
        (Some(w), Some(h)) if w > 0 && h > 0 => Some((w.min(MAX_EDGE), h.min(MAX_EDGE))),
        _ => {
            return Err(bad_request(
                "`w` and `h` must be given together, both above 0",
            ));
        }
    };
    let prio = match q.get("prio") {
        None | Some("current") => Priority::Current,
        Some("prefetch") => Priority::Prefetch,
        Some(other) => return Err(bad_request(format!("unknown prio {other:?}"))),
    };
    let own = |name| header_str(headers, name).map(str::to_owned);
    Ok(Req {
        logical: q.need("p")?.to_owned(),
        boxed,
        prio,
        native: native_formats(header_str(headers, header::ACCEPT)),
        range: own(header::RANGE),
        if_none_match: own(header::IF_NONE_MATCH),
    })
}

fn serve(state: &AppState, req: &Req) -> Result<Outcome> {
    let path = state.policy.resolve(&req.logical)?;
    let stat = state.fs.stat(&path)?;
    require_file(&stat)?;
    let (w, h) = req.boxed.unwrap_or((0, 0));
    let tag = etag(&req.logical, &stat, w, h);
    if matches_etag(req.if_none_match.as_deref(), &tag) {
        return Ok(Outcome::NotModified(tag));
    }
    let want = req
        .boxed
        .map_or(Size::new(MAX_EDGE, MAX_EDGE), |(w, h)| Size::new(w, h));
    match pipeline::render(&*state.fs, &path, want, &req.native)? {
        Rendered::Transcoded { encoded, .. } => Ok(Outcome::Body(Body {
            etag: tag,
            mime: encoded.mime,
            delivery: "transcode",
            bytes: encoded.bytes,
            slice: None,
            ranges: false,
        })),
        Rendered::Passthrough { delivery, mime, .. } => {
            let src = state.fs.open(&path)?;
            passthrough(&*src, delivery, mime, tag, req.range.as_deref())
        }
    }
}

/// Bytes of the file, or of a RAW's embedded preview, honouring `Range`
/// relative to what is being served.
fn passthrough(
    src: &dyn ReadAt,
    delivery: Delivery,
    mime: &'static str,
    etag: String,
    range: Option<&str>,
) -> Result<Outcome> {
    let file_len = src.len()?;
    let (base, total, label) = match delivery {
        Delivery::PassthroughRange { offset, len } => (offset, len, "range"),
        _ => (0, file_len, "passthrough"),
    };
    // The preview location comes from the file itself; never allocate on
    // its word before checking it lies inside the file.
    if base.checked_add(total).is_none_or(|end| end > file_len) {
        return Err(Error::corrupt("embedded preview lies outside the file"));
    }
    let (first, last, partial) = match range.map(|r| classify(r, total)) {
        Some(RangeReq::Bytes(a, b)) => (a, b, true),
        Some(RangeReq::Unsatisfiable) => return Ok(Outcome::Unsatisfiable { total }),
        _ => (0, total.saturating_sub(1), false),
    };
    let len = if total == 0 { 0 } else { last - first + 1 };
    let len = usize::try_from(len)
        .map_err(|_| Error::new(ErrorCode::TooLarge, "file larger than address space"))?;
    Ok(Outcome::Body(Body {
        etag,
        mime,
        delivery: label,
        bytes: read_exact_at(src, base + first, len)?,
        slice: partial.then_some((first, last, total)),
        ranges: true,
    }))
}

fn respond(body: Body) -> Response {
    let status = if body.slice.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let mut resp = (status, body.bytes).into_response();
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(body.mime));
    h.insert(DELIVERY, HeaderValue::from_static(body.delivery));
    // The same URL can answer passthrough or transcode depending on Accept.
    h.insert(header::VARY, HeaderValue::from_static("Accept"));
    if body.ranges {
        h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    }
    if let Some((first, last, total)) = body.slice
        && let Ok(v) = HeaderValue::from_str(&format!("bytes {first}-{last}/{total}"))
    {
        h.insert(header::CONTENT_RANGE, v);
    }
    cache_headers(&mut resp, &body.etag);
    resp
}

fn unsatisfiable(total: u64) -> Response {
    let e = Error::unsupported("requested range lies outside the file");
    let mut resp = *with_status(&e, StatusCode::RANGE_NOT_SATISFIABLE).0;
    if let Ok(v) = HeaderValue::from_str(&format!("bytes */{total}")) {
        resp.headers_mut().insert(header::CONTENT_RANGE, v);
    }
    resp
}
