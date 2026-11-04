//! Host routes against a fake window: they must be guarded exactly like the
//! API, and must win over the API's `/api/v1` catch-all after the merge.

use super::*;
use axum::body::Body;
use axum::http::{Request, header};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tower::ServiceExt;

const TOKEN: &str = "tok";
const HOST: &str = "127.0.0.1:1";

#[derive(Default)]
struct Fake {
    full: AtomicBool,
    log: Mutex<Vec<&'static str>>,
}

impl WindowCtl for Fake {
    fn is_fullscreen(&self) -> bool {
        self.full.load(Ordering::SeqCst)
    }
    fn set_fullscreen(&self, on: bool) -> bool {
        self.full.store(on, Ordering::SeqCst);
        on
    }
    fn hide(&self) {
        self.log.lock().unwrap().push("hide");
    }
    fn quit(&self) {
        self.log.lock().unwrap().push("quit");
    }
    fn pick_folder(&self) -> Option<String> {
        self.log.lock().unwrap().push("pick");
        Some(r"D:\Photos".into())
    }
}

fn app(fake: Arc<Fake>) -> (Router, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let state = crate::serve::build_state(
        TOKEN.into(),
        vec![HOST.into()],
        None,
        tmp.path(),
        Vec::new(),
    );
    let api = photosee_server::router(state.clone(), None);
    (api.merge(routes(fake, state, None)), tmp)
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: bool,
    host: &str,
    body: &str,
) -> (u16, String) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, host);
    if cookie {
        req = req.header(header::COOKIE, format!("ps_token={TOKEN}"));
    }
    let resp = app
        .clone()
        .oneshot(req.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn host_routes_need_the_cookie_and_an_allowed_host() {
    let fake = Arc::new(Fake::default());
    let (app, _t) = app(fake.clone());
    for (m, p) in [
        ("GET", "fullscreen"),
        ("POST", "fullscreen"),
        ("POST", "close"),
        ("POST", "quit"),
        ("POST", "pick-folder"),
    ] {
        let uri = format!("/api/v1/host/{p}");
        assert_eq!(
            send(&app, m, &uri, false, HOST, "{\"on\":true}").await.0,
            403,
            "{p} without cookie"
        );
        assert_eq!(
            send(&app, m, &uri, true, "evil.example", "{\"on\":true}")
                .await
                .0,
            403,
            "{p} bad host"
        );
    }
    assert!(fake.log.lock().unwrap().is_empty() && !fake.is_fullscreen());
}

#[tokio::test]
async fn fullscreen_close_and_quit_reach_the_window() {
    let fake = Arc::new(Fake::default());
    let (app, _t) = app(fake.clone());
    assert_eq!(
        send(&app, "GET", "/api/v1/host/fullscreen", true, HOST, "").await,
        (200, "{\"on\":false}".into())
    );
    let set = send(
        &app,
        "POST",
        "/api/v1/host/fullscreen",
        true,
        HOST,
        "{\"on\":true}",
    )
    .await;
    assert_eq!(set, (200, "{\"on\":true}".into()));
    assert!(fake.is_fullscreen());
    assert_eq!(
        send(&app, "POST", "/api/v1/host/close", true, HOST, "")
            .await
            .0,
        204
    );
    assert_eq!(
        send(&app, "POST", "/api/v1/host/quit", true, HOST, "")
            .await
            .0,
        204
    );
    assert_eq!(*fake.log.lock().unwrap(), ["hide", "quit"]);
}

#[tokio::test]
async fn bad_bodies_and_unknown_host_paths_fail_cleanly() {
    let (app, _t) = app(Arc::new(Fake::default()));
    for body in ["", "nope", "{}", "{\"on\":1}"] {
        assert_eq!(
            send(&app, "POST", "/api/v1/host/fullscreen", true, HOST, body)
                .await
                .0,
            400,
            "{body:?}"
        );
    }
    assert_eq!(
        send(&app, "GET", "/api/v1/host/nothing", true, HOST, "")
            .await
            .0,
        404
    );
    assert_eq!(
        send(&app, "PUT", "/api/v1/host/close", true, HOST, "")
            .await
            .0,
        405
    );
    // The API itself still answers next to the host routes.
    assert_eq!(
        send(&app, "GET", "/api/v1/config", true, HOST, "").await.0,
        200
    );
}

#[tokio::test]
async fn perf_route_exists_only_when_asked_for() {
    let (app, _t) = app(Arc::new(Fake::default()));
    assert_eq!(
        send(&app, "POST", "/api/v1/host/perf", true, HOST, "{}")
            .await
            .0,
        404
    );
}

#[tokio::test]
async fn pick_folder_returns_the_picked_path() {
    let fake = Arc::new(Fake::default());
    let (app, _t) = app(fake.clone());
    let got = send(&app, "POST", "/api/v1/host/pick-folder", true, HOST, "").await;
    assert_eq!(got, (200, r#"{"path":"D:\\Photos"}"#.into()));
    assert_eq!(*fake.log.lock().unwrap(), ["pick"]);
}
