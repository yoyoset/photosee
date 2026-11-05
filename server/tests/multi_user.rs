//! Family accounts: each user sees only their own roots and their own
//! ui-state; no cookie means 401; wrong PINs lock the name for a minute.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_support.rs"]
mod support;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use photosee_core::io::LocalFs;
use photosee_server::accounts::{LOCKOUT, MAX_FAILS, Sessions, parse_users};
use photosee_server::paths::Policy;
use photosee_server::queue::IoQueue;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use support::*;
use tower::ServiceExt;

async fn post(app: &Router, uri: &str, body: &str, cookie: Option<&str>) -> Resp {
    let mut req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let resp = app
        .clone()
        .oneshot(req.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let (parts, body) = resp.into_parts();
    let body = axum::body::to_bytes(body, 1 << 20).await.unwrap();
    Resp {
        status: parts.status,
        headers: parts.headers,
        body: body.to_vec(),
    }
}

struct Family {
    app: Router,
    _dirs: Vec<tempfile::TempDir>,
}

/// dad: `comics` (his own) + `shared`; mum: `books` + `shared`.
fn family() -> Family {
    let (dad, mum, shared, web, data) = (tempdir(), tempdir(), tempdir(), tempdir(), tempdir());
    std::fs::write(dad.path().join("d.bmp"), bmp(2, 2)).unwrap();
    std::fs::write(mum.path().join("m.bmp"), bmp(2, 2)).unwrap();
    std::fs::write(web.path().join("index.html"), "<p>ui</p>").unwrap();
    let s = |p: &Path| p.to_str().unwrap().replace('\\', "\\\\");
    let file = format!(
        r#"{{"users":[
          {{"name":"dad","pin":"1234","roots":[{{"name":"comics","dir":"{}"}},{{"name":"shared","dir":"{}"}}]}},
          {{"name":"mum","pin":"5678","roots":[{{"name":"books","dir":"{}"}},{{"name":"shared","dir":"{}"}}]}}]}}"#,
        s(dad.path()),
        s(shared.path()),
        s(mum.path()),
        s(shared.path()),
    );
    let users = parse_users(file.as_bytes(), Path::is_dir).unwrap();
    let base = data.path().to_path_buf();
    let make = |u: &photosee_server::accounts::UserSpec| {
        let (mut st, _tmp) = state_with(
            Arc::new(LocalFs),
            Policy::Roots(u.roots.clone()),
            IoQueue::new(2),
        );
        st.settings_path = base.join("users").join(&u.name).join("settings.json");
        st.thumbs = Arc::new(photosee_core::thumbcache::ThumbCache::new(
            base.join("thumbs"),
            1 << 20,
        ));
        st
    };
    let app = photosee_server::multi::router(
        &users,
        base.join("sessions.json"),
        vec![],
        web.path().to_path_buf(),
        make,
    );
    Family {
        app,
        _dirs: vec![dad, mum, shared, web, data],
    }
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

async fn login(app: &Router, name: &str, pin: &str) -> String {
    let r = post(
        app,
        "/api/v1/login",
        &format!(r#"{{"name":"{name}","pin":"{pin}"}}"#),
        None,
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&r.body)
    );
    let c = r.header("set-cookie").to_owned();
    assert!(
        c.contains("HttpOnly") && c.contains("SameSite=Strict"),
        "{c}"
    );
    c.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn without_a_session_only_the_login_screen_answers() {
    let f = family();
    assert_eq!(get(&f.app, "/").await.status, StatusCode::OK);
    assert_eq!(
        get(&f.app, "/api/v1/users").await.json(),
        json!(["dad", "mum"])
    );
    let s = get(&f.app, "/api/v1/session").await;
    assert_eq!(
        (s.status, s.json()["code"].clone()),
        (StatusCode::UNAUTHORIZED, json!("unauthorized"))
    );
    assert_eq!(
        get(&f.app, "/api/v1/list?p=%2Fshared").await.status,
        StatusCode::UNAUTHORIZED
    );
    let bad = post(
        &f.app,
        "/api/v1/login",
        r#"{"name":"dad","pin":"0000"}"#,
        None,
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert!(bad.header("set-cookie").is_empty());
}

#[tokio::test]
async fn each_user_sees_only_their_roots() {
    let f = family();
    let dad = login(&f.app, "dad", "1234").await;
    let mum = login(&f.app, "mum", "5678").await;
    let sess = send(&f.app, "/api/v1/session", &[("cookie", &dad)])
        .await
        .json();
    assert_eq!(sess["user"], json!("dad"));
    let roots: Vec<_> = sess["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].clone())
        .collect();
    assert_eq!(roots, vec![json!("/comics"), json!("/shared")]);
    assert_eq!(
        send(&f.app, "/api/v1/list?p=%2Fcomics", &[("cookie", &dad)])
            .await
            .status,
        StatusCode::OK
    );
    // mum asking for dad's root by its name: not hers, so it does not exist for her.
    let r = send(&f.app, "/api/v1/list?p=%2Fcomics", &[("cookie", &mum)]).await;
    assert_ne!(r.status, StatusCode::OK);
    assert_eq!(
        send(&f.app, "/api/v1/list?p=%2Fshared", &[("cookie", &mum)])
            .await
            .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn each_user_has_their_own_ui_state() {
    let f = family();
    let dad = login(&f.app, "dad", "1234").await;
    let mum = login(&f.app, "mum", "5678").await;
    let put = Request::builder()
        .method("PUT")
        .uri("/api/v1/ui-state")
        .header("cookie", &dad)
        .body(Body::from(r#"{"version":1,"last":"/comics"}"#))
        .unwrap();
    assert!(
        f.app
            .clone()
            .oneshot(put)
            .await
            .unwrap()
            .status()
            .is_success()
    );
    assert_eq!(
        send(&f.app, "/api/v1/ui-state", &[("cookie", &dad)])
            .await
            .json()["last"],
        json!("/comics")
    );
    assert_eq!(
        send(&f.app, "/api/v1/ui-state", &[("cookie", &mum)])
            .await
            .json()["last"],
        json!(null)
    );
}

#[tokio::test]
async fn logout_ends_the_session() {
    let f = family();
    let dad = login(&f.app, "dad", "1234").await;
    let out = post(&f.app, "/api/v1/logout", "", Some(&dad)).await;
    assert!(out.header("set-cookie").contains("Max-Age=0"));
    assert_eq!(
        send(&f.app, "/api/v1/session", &[("cookie", &dad)])
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn wrong_pins_lock_the_name_for_a_minute_and_sessions_survive_a_restart() {
    let dir = tempdir();
    let users = parse_users(
        br#"{"users":[{"name":"kid","pin":"2468","roots":[{"name":"r","dir":"/x"}]}]}"#,
        |_| true,
    )
    .unwrap();
    let file = dir.path().join("sessions.json");
    let s = Sessions::new(&users, file.clone());
    let t0 = Instant::now();
    for _ in 0..MAX_FAILS {
        assert!(s.login("kid", "0000", t0, "t".into()).is_err());
    }
    let locked = s.login("kid", "2468", t0, "t".into()).unwrap_err();
    assert!(locked.message.contains("try again"), "{}", locked.message);
    let tok = s.login("kid", "2468", t0 + LOCKOUT, "tok1".into()).unwrap();
    assert!(s.login("nobody", "2468", t0, "t".into()).is_err());
    let again = Sessions::new(&users, file.clone());
    assert_eq!(again.user_of(&tok).unwrap().0, "kid");
    again.logout(&tok);
    assert!(Sessions::new(&users, file).user_of(&tok).is_none());
}

/// Each one is wrong in exactly one way.
const BAD_FILES: &[&str] = &[
    r#"{"users":[]}"#,
    r#"{"users":[{"name":"a","pin":"12","roots":[{"name":"r","dir":"/x"}]}]}"#,
    r#"{"users":[{"name":"a","pin":"12ab","roots":[{"name":"r","dir":"/x"}]}]}"#,
    r#"{"users":[{"name":"a/b","pin":"1234","roots":[{"name":"r","dir":"/x"}]}]}"#,
    r#"{"users":[{"name":"..","pin":"1234","roots":[{"name":"r","dir":"/x"}]}]}"#,
    r#"{"users":[{"name":"a","pin":"1234","roots":[]}]}"#,
    r#"{"users":[{"name":"a","pin":"1234","roots":[{"name":"r","dir":"/x"},{"name":"r","dir":"/y"}]}]}"#,
    r#"{"users":[{"name":"a","pin":"1234","roots":[{"name":"r","dir":"/x"}]},{"name":"a","pin":"1234","roots":[{"name":"r","dir":"/x"}]}]}"#,
];

#[test]
fn users_file_is_validated() {
    let ok = |_: &Path| true;

    for b in BAD_FILES {
        assert!(parse_users(b.as_bytes(), ok).is_err(), "{b}");
    }
    let missing = r#"{"users":[{"name":"a","pin":"1234","roots":[{"name":"r","dir":"/nope"}]}]}"#;
    assert!(parse_users(missing.as_bytes(), |_| false).is_err());
}
