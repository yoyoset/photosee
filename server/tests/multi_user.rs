//! Family accounts: first-run setup makes the admin; each user sees only
//! the folders they mounted; no cookie means 401.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "fixtures_multi/mod.rs"]
mod family;
#[path = "api_support.rs"]
mod support;

use axum::http::StatusCode;
use family::*;
use serde_json::{Value, json};
use support::get;

#[tokio::test]
async fn without_a_session_only_the_login_screen_answers() {
    let f = family(Some(DAD_AND_MUM));
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
    for uri in [
        "/api/v1/list?p=%2Fdad",
        "/api/v1/volumes",
        "/api/v1/admin/users",
    ] {
        assert_eq!(
            get(&f.app, uri).await.status,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
    }
    let wrong = json!({"name": "dad", "pin": "0000"});
    let bad = call(&f.app, "POST", "/api/v1/login", wrong, "").await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert!(bad.header("set-cookie").is_empty());
}

#[tokio::test]
async fn each_user_sees_only_the_folders_they_mounted() {
    let f = family(Some(DAD_AND_MUM));
    let dad = login(&f.app, "dad", "1234").await;
    let mum = login(&f.app, "mum", "5678").await;
    assert_eq!(roots(&f.app, &dad).await, ["/dad", "/shared"]);
    assert_eq!(roots(&f.app, &mum).await, ["/mum"]);
    let s = call(&f.app, "GET", "/api/v1/session", Value::Null, &dad)
        .await
        .json();
    assert_eq!(
        (s["user"].clone(), s["admin"].clone()),
        (json!("dad"), json!(true))
    );
    let list = async |c: &str, p: &str| {
        let uri = format!("/api/v1/list?p=%2F{p}");
        call(&f.app, "GET", &uri, Value::Null, c).await.status
    };
    assert_eq!(list(&dad, "dad").await, StatusCode::OK);
    assert_eq!(list(&mum, "dad").await, StatusCode::NOT_FOUND); // not hers: no such root
    assert_eq!(
        list(&mum, "mum%2F..%2F..%2Fdad").await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn each_user_has_their_own_ui_state() {
    let f = family(Some(DAD_AND_MUM));
    let dad = login(&f.app, "dad", "1234").await;
    let mum = login(&f.app, "mum", "5678").await;
    let state = json!({"version": 1, "last": "/dad"});
    let put = call(&f.app, "PUT", "/api/v1/ui-state", state, &dad).await;
    assert!(put.status.is_success());
    let last = async |c: &str| {
        call(&f.app, "GET", "/api/v1/ui-state", Value::Null, c)
            .await
            .json()["last"]
            .clone()
    };
    assert_eq!(last(&dad).await, json!("/dad"));
    assert_eq!(last(&mum).await, json!(null));
}

#[tokio::test]
async fn logout_ends_the_session() {
    let f = family(Some(DAD_AND_MUM));
    let dad = login(&f.app, "dad", "1234").await;
    let out = call(&f.app, "POST", "/api/v1/logout", Value::Null, &dad).await;
    assert!(out.header("set-cookie").contains("Max-Age=0"));
    let s = call(&f.app, "GET", "/api/v1/session", Value::Null, &dad).await;
    assert_eq!(s.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn first_run_creates_the_admin_once_with_no_folders() {
    let f = family(None);
    assert_eq!(get(&f.app, "/api/v1/users").await.json(), json!([]));
    let first = json!({"name": "dad", "pin": "1234"});
    let r = call(&f.app, "POST", "/api/v1/setup", first, "").await;
    assert_eq!(r.status, StatusCode::OK);
    let cookie = r.header("set-cookie").split(';').next().unwrap().to_owned();
    assert!(roots(&f.app, &cookie).await.is_empty()); // nobody starts with whole disks
    let late = json!({"name": "evil", "pin": "9999"});
    let again = call(&f.app, "POST", "/api/v1/setup", late, "").await;
    assert_eq!(again.status, StatusCode::FORBIDDEN);
    let saved = std::fs::read_to_string(f.data.join("users.json")).unwrap();
    assert!(
        saved.contains("\"admin\": true") && !saved.contains("evil"),
        "{saved}"
    );
}
