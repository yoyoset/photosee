//! The golden files of /contracts, served from the docs/api.md fixture tree.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_jpeg.rs"]
mod jpeg;
#[path = "api_support.rs"]
mod support;

use axum::http::StatusCode;
use serde_json::{Value, json};
use std::sync::Arc;
use support::*;

fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[tokio::test]
async fn session_matches_golden() {
    let env = golden_env();
    let r = get(&env.app, "/api/v1/session").await;
    assert_eq!(r.status, StatusCode::OK);
    let mut want = golden("session.json");
    want["platform"] = json!(platform());
    // The golden holds a placeholder: the real one changes every release.
    want["version"] = json!(env!("CARGO_PKG_VERSION"));
    assert_eq!(r.json(), want);
}

#[tokio::test]
async fn list_matches_golden() {
    let env = golden_env();
    let r = get(&env.app, "/api/v1/list?p=/photos").await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json(), golden("list.json"));
}

#[tokio::test]
async fn list_of_a_subfolder_has_a_parent() {
    let env = golden_env();
    let r = get(&env.app, "/api/v1/list?p=/photos/2023").await;
    let body = r.json();
    assert_eq!(body["parent"], json!("/photos"));
    assert_eq!(body["images"][0]["path"], json!("/photos/2023/b.png"));
    assert_eq!(body["images"][0]["fmt"], json!("png"));
    // A trailing slash is the same folder, spelled out in the reply once.
    let again = get(&env.app, "/api/v1/list?p=/photos/2023/").await;
    assert_eq!(again.json(), body);
}

#[tokio::test]
async fn list_errors_use_the_error_body() {
    let env = golden_env();
    let missing = get(&env.app, "/api/v1/list?p=/photos/nope").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json()["code"], json!("not_found"));
    let file = get(&env.app, "/api/v1/list?p=/photos/a2.jpg").await;
    assert_eq!(file.json()["code"], json!("unsupported"));
    let none = get(&env.app, "/api/v1/list").await;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
    assert_eq!(none.json()["code"], json!("unsupported"));
    let escape = get(&env.app, "/api/v1/list?p=/photos/../etc").await;
    assert_eq!(escape.status, StatusCode::FORBIDDEN);
    assert_eq!(escape.json()["code"], json!("forbidden"));
}

#[tokio::test]
async fn nav_matches_every_golden_case() {
    let env = golden_env();
    let golden = golden("nav.json");
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for c in cases {
        let uri = format!(
            "/api/v1/nav?p={}&root={}&d={}",
            c["p"].as_str().unwrap(),
            c["root"].as_str().unwrap(),
            c["d"].as_str().unwrap()
        );
        let r = get(&env.app, &uri).await;
        assert_eq!(r.status, StatusCode::OK, "{uri}");
        assert_eq!(r.json(), json!({ "dir": c["dir"] }), "{uri}");
    }
}

#[tokio::test]
async fn nav_rejects_bad_input() {
    let env = golden_env();
    let sideways = get(&env.app, "/api/v1/nav?p=/photos&root=/photos&d=up").await;
    assert_eq!(sideways.status, StatusCode::BAD_REQUEST);
    let no_root = get(&env.app, "/api/v1/nav?p=/photos&d=next").await;
    assert_eq!(no_root.status, StatusCode::BAD_REQUEST);
    let outside = get(&env.app, "/api/v1/nav?p=/photos&root=/other&d=next").await;
    assert_eq!(outside.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn config_defaults_match_golden() {
    let env = golden_env();
    let r = get(&env.app, "/api/v1/config").await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json(), golden("config.json"));
}

async fn put(env: &Env, body: &str) -> Resp {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/config")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let resp = env.app.clone().oneshot(req).await.unwrap();
    let (parts, body) = resp.into_parts();
    Resp {
        status: parts.status,
        headers: parts.headers,
        body: axum::body::to_bytes(body, 1 << 20).await.unwrap().to_vec(),
    }
}

#[tokio::test]
async fn config_put_normalizes_and_persists() {
    let env = golden_env();
    let mut sent = golden("config.json");
    sent["interval_s"] = json!(999);
    sent["wheel"] = json!("zoom");
    let r = put(&env, &sent.to_string()).await;
    assert_eq!(r.status, StatusCode::OK);
    let mut want = sent.clone();
    want["interval_s"] = json!(60);
    assert_eq!(r.json(), want);
    assert_eq!(get(&env.app, "/api/v1/config").await.json(), want);
}

#[tokio::test]
async fn config_put_rejects_garbage() {
    let env = golden_env();
    for body in [
        "{not json",
        r#"{"wheel":"sideways"}"#,
        r#"{"interval_s":-1}"#,
    ] {
        let r = put(&env, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(r.json()["code"], json!("unsupported"), "{body}");
    }
    assert_eq!(
        get(&env.app, "/api/v1/config").await.json(),
        golden("config.json")
    );
}

#[tokio::test]
async fn meta_has_the_golden_shape_and_display_size() {
    let fs = golden_tree();
    // Stored 6000x4000 with orientation 6 (rotate 90): shown as 4000x6000.
    let bytes = jpeg::skeleton(6000, 4000, 6, 100);
    fs.file("/photos/cam.jpg", &bytes);
    let env = env_with(Arc::new(fs), photosee_server::queue::IoQueue::new(4));
    let r = get(&env.app, "/api/v1/meta?p=/photos/cam.jpg").await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "{:?}",
        String::from_utf8_lossy(&r.body)
    );
    let got = r.json();
    let mut want = golden("meta.json");
    // The golden file describes a 5 MB photo; the fixture is smaller.
    want["size"] = json!(bytes.len());
    want["mtime_ms"] = json!(0);
    assert_eq!(got, want);
    assert!(got["width"].is_u64() && got["exif"]["f_number"].is_f64());
}

#[tokio::test]
async fn meta_without_exif_gives_nulls() {
    let fs = golden_tree();
    fs.file("/photos/plain.bmp", &bmp(3, 2));
    let env = env_with(Arc::new(fs), photosee_server::queue::IoQueue::new(4));
    let got = get(&env.app, "/api/v1/meta?p=/photos/plain.bmp")
        .await
        .json();
    assert_eq!(
        (got["width"].clone(), got["height"].clone()),
        (json!(3), json!(2))
    );
    assert_eq!(got["fmt"], json!("bmp"));
    assert_eq!(got["exif"]["camera"], Value::Null);
}

#[tokio::test]
async fn unknown_api_paths_are_json_404s() {
    let env = golden_env();
    let r = get(&env.app, "/api/v1/nope").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.json()["code"], json!("not_found"));
}
