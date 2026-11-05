//! Mounting your own folders, account admin, and the session store.
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
use photosee_server::accounts::parse_users;
use photosee_server::sessions::{LOCKOUT, MAX_FAILS, Sessions};
use serde_json::{Value, json};
use std::time::Instant;
use support::get;

const ADD: &str = "/api/v1/library/add";
const REMOVE: &str = "/api/v1/library/remove";
const USERS: &str = "/api/v1/admin/users";

#[tokio::test]
async fn the_picker_walks_the_volumes_folders_only() {
    let f = family(Some(DAD_AND_MUM));
    let mum = login(&f.app, "mum", "5678").await;
    let vols = get_json(&f.app, "/api/v1/volumes", &mum).await;
    assert_eq!(vols["dirs"], json!([{ "name": "disk", "path": "/disk" }]));
    let disk = get_json(&f.app, "/api/v1/volumes?p=%2Fdisk", &mum).await;
    let names: Vec<&str> = disk["dirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        (names, disk["parent"].clone()),
        (vec!["dad", "mum", "shared"], json!("/"))
    );
}

#[tokio::test]
async fn a_user_mounts_and_unmounts_their_own_folders() {
    let f = family(Some(DAD_AND_MUM));
    let mum = login(&f.app, "mum", "5678").await;
    assert_eq!(
        post(&f.app, ADD, json!({"path": "/disk/shared"}), &mum).await,
        StatusCode::OK
    );
    assert_eq!(roots(&f.app, &mum).await, ["/mum", "/shared"]);
    assert_eq!(
        post(&f.app, REMOVE, json!({"root": "mum"}), &mum).await,
        StatusCode::OK
    );
    assert_eq!(roots(&f.app, &mum).await, ["/shared"]);
    assert_ne!(
        post(&f.app, ADD, json!({"path": "/disk/../.."}), &mum).await,
        StatusCode::OK
    );
    assert_ne!(
        post(&f.app, REMOVE, json!({"root": "dad"}), &mum).await,
        StatusCode::OK
    );
    let saved = std::fs::read_to_string(f.data.join("users.json")).unwrap();
    assert!(
        saved.contains("shared") && !saved.contains("mum.bmp"),
        "{saved}"
    );
}

#[tokio::test]
async fn a_vanished_folder_is_skipped_not_fatal() {
    let f = family(Some(DAD_AND_MUM));
    let dad = login(&f.app, "dad", "1234").await;
    std::fs::remove_dir_all(f.vol.join("shared")).unwrap();
    // A change rebuilds the routers; the gone folder drops out, the rest stays.
    assert_eq!(
        post(&f.app, ADD, json!({"path": "/disk/mum"}), &dad).await,
        StatusCode::OK
    );
    assert_eq!(roots(&f.app, &dad).await, ["/dad", "/mum"]);
}

#[tokio::test]
async fn admins_create_accounts_others_cannot() {
    let f = family(Some(DAD_AND_MUM));
    let (dad, mum) = (
        login(&f.app, "dad", "1234").await,
        login(&f.app, "mum", "5678").await,
    );
    let kid = json!({"name": "kid", "pin": "2468"});
    assert_eq!(
        post(&f.app, USERS, kid.clone(), &mum).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(post(&f.app, USERS, kid.clone(), &dad).await, StatusCode::OK);
    assert_eq!(post(&f.app, USERS, kid, &dad).await, StatusCode::FORBIDDEN); // name taken
    let k = login(&f.app, "kid", "2468").await;
    assert!(roots(&f.app, &k).await.is_empty()); // readers pick their own folders
    let list = get_json(&f.app, USERS, &dad).await;
    assert_eq!(
        list[2],
        json!({"name": "kid", "admin": false, "folders": 0})
    );
    assert_eq!(
        call(&f.app, "GET", USERS, Value::Null, &mum).await.status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn admins_reset_pins_and_delete_accounts_but_not_themselves() {
    let f = family(Some(DAD_AND_MUM));
    let (dad, mum) = (
        login(&f.app, "dad", "1234").await,
        login(&f.app, "mum", "5678").await,
    );
    let pin = json!({"name": "mum", "pin": "1357"});
    assert_eq!(
        post(&f.app, "/api/v1/admin/users/pin", pin, &dad).await,
        StatusCode::OK
    );
    let s = call(&f.app, "GET", "/api/v1/session", Value::Null, &mum).await;
    assert_eq!(s.status, StatusCode::UNAUTHORIZED); // her devices log in again
    let mum = login(&f.app, "mum", "1357").await;
    let rm = "/api/v1/admin/users/remove";
    assert_eq!(
        post(&f.app, rm, json!({"name": "dad"}), &dad).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&f.app, rm, json!({"name": "mum"}), &dad).await,
        StatusCode::OK
    );
    let s = call(&f.app, "GET", "/api/v1/session", Value::Null, &mum).await;
    assert_eq!(s.status, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&f.app, "/api/v1/users").await.json(), json!(["dad"]));
}

#[test]
fn wrong_pins_lock_the_name_for_a_minute_and_sessions_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sessions.json");
    let s = Sessions::new(file.clone(), |_| true);
    let t0 = Instant::now();
    for _ in 0..MAX_FAILS {
        assert!(
            s.login("kid", "0000", Some("2468"), t0, "t".into())
                .is_err()
        );
    }
    let locked = s
        .login("kid", "2468", Some("2468"), t0, "t".into())
        .unwrap_err();
    assert!(locked.message.contains("try again"), "{}", locked.message);
    let tok = s
        .login("kid", "2468", Some("2468"), t0 + LOCKOUT, "tok1".into())
        .unwrap();
    assert!(s.login("nobody", "2468", None, t0, "t".into()).is_err());
    assert_eq!(
        Sessions::new(file.clone(), |_| true).user_of(&tok).unwrap(),
        "kid"
    );
    assert!(
        Sessions::new(file.clone(), |n| n != "kid")
            .user_of(&tok)
            .is_none()
    ); // deleted user
    s.logout(&tok);
    assert!(Sessions::new(file, |_| true).user_of(&tok).is_none());
}

/// Each one is wrong in exactly one way.
const BAD_FILES: &[&str] = &[
    r#"{"users":[{"name":"a","pin":"12"}]}"#,
    r#"{"users":[{"name":"a","pin":"12ab"}]}"#,
    r#"{"users":[{"name":"a/b","pin":"1234"}]}"#,
    r#"{"users":[{"name":".hidden","pin":"1234"}]}"#,
    r#"{"users":[{"name":" a","pin":"1234"}]}"#,
    r#"{"users":[{"name":"a","pin":"1234"},{"name":"a","pin":"5678"}]}"#,
    r#"{"people":[]}"#,
];

#[test]
fn users_file_is_validated() {
    for b in BAD_FILES {
        assert!(parse_users(b.as_bytes()).is_err(), "{b}");
    }
    assert!(parse_users(br#"{"users":[]}"#).unwrap().is_empty()); // first run
}
