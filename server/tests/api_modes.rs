//! Desktop policy on a real folder, the static UI, and the guard on both.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_support.rs"]
mod support;

use axum::http::StatusCode;
use photosee_core::io::LocalFs;
use photosee_server::paths::Policy;
use photosee_server::queue::IoQueue;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use support::*;

fn desktop(web: Option<&Path>, token: Option<&str>) -> (Env, tempfile::TempDir) {
    let photos = tempfile::tempdir().unwrap();
    std::fs::create_dir(photos.path().join("sub")).unwrap();
    std::fs::write(photos.path().join("a.bmp"), bmp(4, 4)).unwrap();
    std::fs::write(photos.path().join("sub").join("b.bmp"), bmp(2, 2)).unwrap();
    let (mut state, tmp) = state_with(Arc::new(LocalFs), Policy::AnyLocal, IoQueue::new(4));
    state.token = token.map(str::to_owned);
    let app = photosee_server::router(state.clone(), web.map(Path::to_path_buf));
    (Env { state, app, tmp }, photos)
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

fn uri(path: &str, p: &Path) -> String {
    // Windows paths carry `\`, `:` and spaces; the UI percent-encodes them.
    let enc: String = s(p)
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("{path}?p={enc}")
}

#[tokio::test]
async fn desktop_session_without_a_start() {
    let (env, _photos) = desktop(None, None);
    let got = get(&env.app, "/api/v1/session").await.json();
    assert_eq!(got["mode"], json!("desktop"));
    assert_eq!(got["roots"], json!([]));
    assert_eq!(got["start"], json!(null));
}

#[tokio::test]
async fn desktop_session_start_file_and_folder() {
    let (env, photos) = desktop(None, None);
    let file = photos.path().join("a.bmp");
    let mut state = env.state.clone();
    state.start = Some(s(&file));
    let got = get(
        &photosee_server::router(state.clone(), None),
        "/api/v1/session",
    )
    .await;
    assert_eq!(
        got.json()["start"],
        json!({ "dir": s(photos.path()), "file": s(&file) })
    );
    state.start = Some(s(photos.path()));
    let got = get(
        &photosee_server::router(state.clone(), None),
        "/api/v1/session",
    )
    .await;
    assert_eq!(
        got.json()["start"],
        json!({ "dir": s(photos.path()), "file": null })
    );
    state.start = Some(s(&photos.path().join("gone.bmp")));
    let got = get(&photosee_server::router(state, None), "/api/v1/session").await;
    assert_eq!(got.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn desktop_list_paths_are_native_and_parent_is_set() {
    let (env, photos) = desktop(None, None);
    let r = get(&env.app, &uri("/api/v1/list", photos.path())).await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "{:?}",
        String::from_utf8_lossy(&r.body)
    );
    let body = r.json();
    let a = photos.path().join("a.bmp");
    assert_eq!(body["images"][0]["path"], json!(s(&a)));
    assert_eq!(body["images"][0]["fmt"], json!("bmp"));
    assert_eq!(
        body["dirs"][0]["path"],
        json!(s(&photos.path().join("sub")))
    );
    assert_eq!(body["parent"], json!(s(photos.path().parent().unwrap())));
    let image = get(&env.app, &uri("/api/v1/img", &a)).await;
    assert_eq!(image.status, StatusCode::OK);
    assert_eq!(image.body, bmp(4, 4));
}

#[tokio::test]
async fn listings_are_cached_until_the_directory_changes() {
    let (env, photos) = desktop(None, None);
    let list = uri("/api/v1/list", photos.path());
    let first = get(&env.app, &list).await.json();
    assert_eq!(env.state.listings.len(), 1);
    assert_eq!(get(&env.app, &list).await.json(), first);
    // A new file changes the folder's mtime (allow for coarse clocks).
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(photos.path().join("c.bmp"), bmp(2, 2)).unwrap();
    let after = get(&env.app, &list).await.json();
    assert_eq!(after["images"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn static_files_come_from_web_dir_with_index_fallback() {
    let web = tempfile::tempdir().unwrap();
    std::fs::write(web.path().join("index.html"), "<p>ui</p>").unwrap();
    std::fs::write(web.path().join("app.js"), "let x;").unwrap();
    let (env, _photos) = desktop(Some(web.path()), None);
    let js = get(&env.app, "/app.js").await;
    assert_eq!(
        (js.status, js.body.as_slice()),
        (StatusCode::OK, b"let x;".as_slice())
    );
    let root = get(&env.app, "/").await;
    assert_eq!(root.body, b"<p>ui</p>");
    let deep = get(&env.app, "/some/where").await;
    assert_eq!(
        (deep.status, deep.body.as_slice()),
        (StatusCode::OK, b"<p>ui</p>".as_slice())
    );
    // The API is not swallowed by the fallback, and the guard's headers
    // reach static files too.
    assert_eq!(
        get(&env.app, "/api/v1/zzz").await.status,
        StatusCode::NOT_FOUND
    );
    assert!(!js.header("content-security-policy").is_empty());
}

#[tokio::test]
async fn the_guard_covers_static_files_and_the_api() {
    let web = tempfile::tempdir().unwrap();
    std::fs::write(web.path().join("index.html"), "<p>ui</p>").unwrap();
    let (env, _photos) = desktop(Some(web.path()), Some("tok123"));
    for uri in [
        "/",
        "/index.html",
        "/nothing",
        "/api/v1/session",
        "/api/v1/config",
    ] {
        let r = get(&env.app, uri).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(r.json()["code"], json!("forbidden"), "{uri}");
    }
    let cookie = [("cookie", "ps_token=tok123")];
    assert_eq!(send(&env.app, "/", &cookie).await.status, StatusCode::OK);
    assert_eq!(
        send(&env.app, "/api/v1/session", &cookie).await.status,
        StatusCode::OK
    );
    let boot = get(&env.app, "/?t=tok123").await;
    assert_eq!(boot.status, StatusCode::SEE_OTHER);
}
