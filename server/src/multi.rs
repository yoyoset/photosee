//! Multi-user router (accounts.rs explains the why). Public without a
//! session: the UI files, `GET /users` (names for the login screen),
//! `POST /login`, `POST /logout`. Every other `/api/v1/*` call goes to the
//! caller's own router, picked by the `ps_user` cookie, or answers 401.

use crate::accounts::{Sessions, UserSpec, new_token};
use crate::error::error_response;
use crate::guard::{add_security_headers, host_allowed};
use crate::{AppState, routes};
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use photosee_core::{Error, ErrorCode};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceExt;

pub const COOKIE: &str = "ps_user";
/// "Logged in until Switch user": a year, renewed by every login.
const COOKIE_MAX_AGE_S: u32 = 365 * 24 * 3600;

pub struct Multi {
    names: Vec<String>,
    routers: HashMap<String, Router>,
    sessions: Sessions,
    hosts: Vec<String>,
}

/// `make` builds one user's state from the shared parts (main.rs owns them).
pub fn router(
    users: &[UserSpec],
    sessions_file: PathBuf,
    hosts: Vec<String>,
    web_dir: PathBuf,
    make: impl Fn(&UserSpec) -> AppState,
) -> Router {
    let m = Multi {
        names: users.iter().map(|u| u.name.clone()).collect(),
        routers: users
            .iter()
            .map(|u| (u.name.clone(), routes::router(make(u), None)))
            .collect(),
        sessions: Sessions::new(users, sessions_file),
        hosts,
    };
    let m = Arc::new(m);
    Router::new()
        .route("/api/v1/users", get(list_users))
        .route("/api/v1/login", post(login))
        .route("/api/v1/logout", post(logout))
        .route("/api/v1/{*rest}", any(dispatch))
        .fallback_service(routes::ui_service(web_dir))
        .layer(axum::middleware::from_fn_with_state(m.clone(), check_host))
        .with_state(m)
}

/// The per-user routers check Host too, but the public routes and the UI
/// files live only here.
async fn check_host(State(m): State<Arc<Multi>>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host_allowed(&m.hosts, host) {
        return error_response(&Error::new(ErrorCode::Forbidden, "Host not allowed"));
    }
    let mut resp = next.run(req).await;
    add_security_headers(resp.headers_mut());
    resp
}

fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v)
}

async fn dispatch(State(m): State<Arc<Multi>>, mut req: Request) -> Response {
    let user = cookie(req.headers()).and_then(|t| m.sessions.user_of(t));
    let Some(router) = user.as_ref().and_then(|u| m.routers.get(&u.0)) else {
        return error_response(&Error::new(ErrorCode::Unauthorized, "login required"));
    };
    if let Some(u) = user {
        req.extensions_mut().insert(u); // /session reports who is logged in
    }
    match router.clone().oneshot(req).await {
        Ok(r) => r,
        Err(never) => match never {},
    }
}

async fn list_users(State(m): State<Arc<Multi>>) -> Json<Vec<String>> {
    Json(m.names.clone())
}

#[derive(serde::Deserialize)]
struct LoginBody {
    name: String,
    pin: String,
}

fn set_cookie(resp: &mut Response, value: &str, max_age: u32) {
    let c = format!("{COOKIE}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}");
    if let Ok(v) = HeaderValue::from_str(&c) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
}

async fn login(State(m): State<Arc<Multi>>, body: Bytes) -> Response {
    let Ok(b) = serde_json::from_slice::<LoginBody>(&body) else {
        return error_response(&Error::new(ErrorCode::Unauthorized, "expected {name, pin}"));
    };
    let token = match new_token() {
        Ok(t) => t,
        Err(e) => return error_response(&e),
    };
    match m.sessions.login(&b.name, &b.pin, Instant::now(), token) {
        Ok(token) => {
            let mut resp = Json(serde_json::json!({ "user": b.name })).into_response();
            set_cookie(&mut resp, &token, COOKIE_MAX_AGE_S);
            resp
        }
        Err(e) => error_response(&e),
    }
}

async fn logout(State(m): State<Arc<Multi>>, headers: HeaderMap) -> Response {
    if let Some(t) = cookie(&headers) {
        m.sessions.logout(t);
    }
    let mut resp = Json(serde_json::json!({})).into_response();
    set_cookie(&mut resp, "", 0);
    resp
}
