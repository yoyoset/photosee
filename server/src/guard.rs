//! Request guard middleware: Host allow-list and session token. Package H
//! (orchestrator; security-sensitive).
//!
//! Threat model (desktop, loopback server): other *local processes* are out
//! of scope. They run as the same user and can read the photos directly.
//! The threat is web pages in the user's browser:
//! - a DNS-rebinding page that makes `evil.example` resolve to 127.0.0.1;
//!   the Host check stops it;
//! - plain cross-site requests such as `<img src=http://127.0.0.1:port/...>`;
//!   the token cookie stops them, because a browser never has it.
//!
//! Bootstrap: the shell opens `/?t=<token>` once. The guard answers 303 to
//! the same path without the query and sets the token as an HttpOnly,
//! SameSite=Strict cookie, so the token never stays in the address bar or
//! reaches page script. After that, every request must carry the cookie.

use crate::AppState;
use crate::error::error_response;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use photosee_core::{Error, ErrorCode};

pub const COOKIE: &str = "ps_token";

/// UI and images come from the same origin; nothing else is ever loaded.
/// `blob:` lets the viewer show prefetched bytes, `data:` covers inline SVG
/// icons.
pub const CSP: &str = "default-src 'self'; img-src 'self' blob: data:; \
     connect-src 'self'; style-src 'self'; script-src 'self'; \
     object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'";

pub async fn check(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .or_else(|| req.uri().authority().map(|a| a.as_str()));
    if !host_allowed(&state.allowed_hosts, host) {
        return deny("Host not allowed");
    }
    if let Some(token) = state.token.as_deref() {
        if !req.uri().path().starts_with("/api/")
            && query_token(req.uri().query()).is_some_and(|t| ct_eq(t, token))
        {
            return bootstrap(req.uri().path(), token);
        }
        if !cookie_token(req.headers()).is_some_and(|t| ct_eq(t, token)) {
            return deny("missing or wrong session token");
        }
    }
    let mut resp = next.run(req).await;
    add_security_headers(resp.headers_mut());
    resp
}

fn deny(why: &str) -> Response {
    error_response(&Error::new(ErrorCode::Forbidden, why))
}

/// Empty list = accept any Host (a Docker server on a LAN has no fixed name;
/// it relies on the password instead). Comparison ignores ASCII case and
/// includes the port.
pub fn host_allowed(allowed: &[String], host: Option<&str>) -> bool {
    if allowed.is_empty() {
        return true;
    }
    host.is_some_and(|h| allowed.iter().any(|a| a.eq_ignore_ascii_case(h)))
}

/// Value of `ps_token` across every Cookie header (HTTP/2 may split them).
pub fn cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v)
}

fn query_token(query: Option<&str>) -> Option<&str> {
    query?
        .split('&')
        .filter_map(|p| p.split_once('='))
        .find(|(k, _)| *k == "t")
        .map(|(_, v)| v)
}

/// Constant-time in the contents, so response timing does not reveal how
/// many leading characters of a guess were right.
pub fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn bootstrap(path: &str, token: &str) -> Response {
    let cookie = format!("{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/");
    let mut resp = StatusCode::SEE_OTHER.into_response();
    let h = resp.headers_mut();
    // `path` came from a parsed URI, so it is a valid header value; the
    // token is server-generated. Both conversions failing would be a bug.
    if let (Ok(loc), Ok(c)) = (HeaderValue::from_str(path), HeaderValue::from_str(&cookie)) {
        h.insert(header::LOCATION, loc);
        h.insert(header::SET_COOKIE, c);
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    resp
}

pub fn add_security_headers(h: &mut HeaderMap) {
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}
