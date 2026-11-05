//! Fixture for the multi-user tests: one volume with dad/, mum/, shared/.
//! Included by multi_user.rs and multi_admin.rs next to api_support.rs.
#![allow(dead_code, reason = "each test binary uses a different subset")]

use crate::support::{Resp, bmp, state_with};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use photosee_core::io::LocalFs;
use photosee_server::multi::{self, MakeState, Multi};
use photosee_server::paths::{Policy, Root};
use photosee_server::queue::IoQueue;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

pub async fn call(app: &Router, method: &str, uri: &str, body: Value, cookie: &str) -> Resp {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("cookie", cookie);
    let body = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let (parts, body) = resp.into_parts();
    let body = axum::body::to_bytes(body, 1 << 20).await.unwrap();
    Resp {
        status: parts.status,
        headers: parts.headers,
        body: body.to_vec(),
    }
}

pub struct Family {
    pub app: Router,
    pub vol: PathBuf,
    pub data: PathBuf,
    _dirs: Vec<tempfile::TempDir>,
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn make_state(base: PathBuf) -> MakeState {
    Box::new(move |name, roots| {
        let (mut st, _tmp) = state_with(Arc::new(LocalFs), Policy::Roots(roots), IoQueue::new(2));
        st.settings_path = base.join("users").join(name).join("settings.json");
        st.thumbs = Arc::new(photosee_core::thumbcache::ThumbCache::new(
            base.join("thumbs"),
            1 << 20,
        ));
        st
    })
}

/// `users` is users.json (None: first run); `{vol}` stands for the volume.
pub fn family(users: Option<&str>) -> Family {
    let (vol, web, data) = (tempdir(), tempdir(), tempdir());
    for d in ["dad", "mum", "shared"] {
        std::fs::create_dir(vol.path().join(d)).unwrap();
        std::fs::write(vol.path().join(d).join(format!("{d}.bmp")), bmp(2, 2)).unwrap();
    }
    std::fs::write(web.path().join("index.html"), "<p>ui</p>").unwrap();
    let file = data.path().join("users.json");
    if let Some(u) = users {
        let v = vol.path().to_str().unwrap().replace('\\', "\\\\");
        std::fs::write(&file, u.replace("{vol}", &v)).unwrap();
    }
    let volumes = vec![Root {
        name: "disk".into(),
        dir: vol.path().to_path_buf(),
    }];
    let make = make_state(data.path().to_path_buf());
    let m = Multi::new(
        file,
        data.path().join("sessions.json"),
        volumes,
        vec![],
        make,
    )
    .unwrap();
    Family {
        app: multi::router(m, web.path().to_path_buf()),
        vol: vol.path().to_path_buf(),
        data: data.path().to_path_buf(),
        _dirs: vec![vol, web, data],
    }
}

pub const DAD_AND_MUM: &str = r#"{"users":[
  {"name":"dad","pin":"1234","admin":true,"dirs":["{vol}/dad","{vol}/shared"]},
  {"name":"mum","pin":"5678","dirs":["{vol}/mum"]}]}"#;

pub async fn login(app: &Router, name: &str, pin: &str) -> String {
    let body = serde_json::json!({ "name": name, "pin": pin });
    let r = call(app, "POST", "/api/v1/login", body, "").await;
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

pub async fn roots(app: &Router, cookie: &str) -> Vec<String> {
    let s = call(app, "GET", "/api/v1/session", Value::Null, cookie)
        .await
        .json();
    s["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_owned())
        .collect()
}

/// POST with a JSON body; just the status.
pub async fn post(app: &Router, uri: &str, body: Value, cookie: &str) -> StatusCode {
    call(app, "POST", uri, body, cookie).await.status
}

/// GET as `cookie`; the JSON body.
pub async fn get_json(app: &Router, uri: &str, cookie: &str) -> Value {
    call(app, "GET", uri, Value::Null, cookie).await.json()
}
