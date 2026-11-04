//! /ui-state: opaque JSON object store with a size cap.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_support.rs"]
mod support;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

async fn put(app: &axum::Router, body: Vec<u8>) -> StatusCode {
    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/v1/ui-state")
        .body(Body::from(body))
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn ui_state_round_trips_and_rejects_bad_bodies() {
    let env = support::golden_env();
    let got = support::get(&env.app, "/api/v1/ui-state").await;
    assert_eq!(
        (got.status, got.json()),
        (StatusCode::OK, serde_json::json!({}))
    );

    let state = br#"{"tree":{"visible":true,"width":260},"recent":["/photos/2023"]}"#;
    assert_eq!(put(&env.app, state.to_vec()).await, StatusCode::NO_CONTENT);
    let got = support::get(&env.app, "/api/v1/ui-state").await;
    assert_eq!(got.json()["tree"]["width"], 260);

    assert_eq!(
        put(&env.app, b"[1,2]".to_vec()).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        put(&env.app, b"not json".to_vec()).await,
        StatusCode::BAD_REQUEST
    );
    let mut big = b"{\"x\":\"".to_vec();
    big.extend(std::iter::repeat_n(b'a', 70 * 1024));
    big.extend_from_slice(b"\"}");
    assert_eq!(put(&env.app, big).await, StatusCode::BAD_REQUEST);
    // A rejected PUT leaves the stored object alone.
    let got = support::get(&env.app, "/api/v1/ui-state").await;
    assert_eq!(got.json()["recent"][0], "/photos/2023");
}
