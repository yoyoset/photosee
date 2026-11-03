//! Guard middleware: Host allow-list, token cookie, bootstrap, headers.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use photosee_core::testfs::MemFs;
use photosee_core::thumbcache::ThumbCache;
use photosee_server::AppState;
use photosee_server::cache::ListingCache;
use photosee_server::guard;
use photosee_server::paths::Policy;
use photosee_server::queue::IoQueue;
use std::sync::Arc;
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:5173";
const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn state(token: Option<&str>, hosts: &[&str]) -> AppState {
    AppState {
        fs: Arc::new(MemFs::new()),
        policy: Arc::new(Policy::AnyLocal),
        listings: Arc::new(ListingCache::default()),
        queue: Arc::new(IoQueue::new(4)),
        thumbs: Arc::new(ThumbCache::new(std::env::temp_dir(), 0)),
        settings_path: std::env::temp_dir().join("photosee-guard-test.json"),
        token: token.map(str::to_owned),
        allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
        start: None,
    }
}

fn app(s: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { "ui" }))
        .route("/api/v1/session", get(|| async { "{}" }))
        .layer(axum::middleware::from_fn_with_state(
            s.clone(),
            guard::check,
        ))
        .with_state(s)
}

async fn send(
    app: Router,
    uri: &str,
    host: Option<&str>,
    cookie: Option<&str>,
) -> axum::response::Response {
    let mut req = Request::builder().uri(uri);
    if let Some(h) = host {
        req = req.header(header::HOST, h);
    }
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap()
}

fn desktop() -> Router {
    app(state(Some(TOKEN), &[HOST, "localhost:5173"]))
}

#[tokio::test]
async fn wrong_or_missing_host_is_forbidden() {
    let ok = format!("ps_token={TOKEN}");
    for host in [Some("evil.example:5173"), Some("127.0.0.1:9999"), None] {
        let r = send(desktop(), "/api/v1/session", host, Some(&ok)).await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "{host:?}");
    }
    let r = send(
        desktop(),
        "/api/v1/session",
        Some("LOCALHOST:5173"),
        Some(&ok),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
}

#[tokio::test]
async fn token_cookie_is_required_and_compared_exactly() {
    let wrong = format!("ps_token={}x", &TOKEN[..TOKEN.len() - 1]);
    for cookie in [
        None,
        Some("ps_token="),
        Some(wrong.as_str()),
        Some("other=1"),
    ] {
        let r = send(desktop(), "/api/v1/session", Some(HOST), cookie).await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "{cookie:?}");
        let body = axum::body::to_bytes(r.into_body(), 4096).await.unwrap();
        assert!(body.starts_with(br#"{"code":"forbidden""#));
    }
    let ok = format!("a=1; ps_token={TOKEN}; b=2");
    let r = send(desktop(), "/", Some(HOST), Some(&ok)).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.headers().get(header::CONTENT_SECURITY_POLICY).is_some());
    assert_eq!(r.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
}

#[tokio::test]
async fn bootstrap_sets_cookie_and_strips_query() {
    let r = send(desktop(), &format!("/?t={TOKEN}"), Some(HOST), None).await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    assert_eq!(r.headers()[header::LOCATION], "/");
    let c = r.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(c.starts_with(&format!("ps_token={TOKEN};")));
    assert!(c.contains("HttpOnly") && c.contains("SameSite=Strict"));
    // A wrong bootstrap token sets nothing, and the API never bootstraps.
    let r = send(desktop(), "/?t=nope", Some(HOST), None).await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let r = send(
        desktop(),
        &format!("/api/v1/session?t={TOKEN}"),
        Some(HOST),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    // Bootstrap still needs an allowed Host (rebinding page guessing a token).
    let r = send(
        desktop(),
        &format!("/?t={TOKEN}"),
        Some("evil.example"),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn open_lan_server_needs_neither() {
    let r = send(
        app(state(None, &[])),
        "/api/v1/session",
        Some("nas.lan:8080"),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
}

#[test]
fn ct_eq_matches_eq() {
    assert!(guard::ct_eq("abc", "abc"));
    assert!(!guard::ct_eq("abc", "abd"));
    assert!(!guard::ct_eq("abc", "ab"));
    assert!(guard::ct_eq("", ""));
}
