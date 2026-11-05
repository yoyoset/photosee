//! Multi-user router (accounts.rs explains the why). Public without a
//! session: the UI files, `GET /users` (names for the login screen),
//! `POST /setup` (first run only), `POST /login`, `POST /logout`. Library and
//! admin calls are in library.rs and admin.rs. Every other `/api/v1/*` call
//! goes to the caller's own router, picked by the `ps_user` cookie, or 401.
//!
//! The account list changes at run time (admin, "add folder"), so it sits
//! behind a lock, is saved to users.json on every change, and each change
//! rebuilds the per-user routers (a few users: rebuilding all is cheapest).

use crate::accounts::{self, Account, check_name, check_pin, new_token, roots_of};
use crate::error::error_response;
use crate::guard::{add_security_headers, host_allowed};
use crate::paths::{Policy, Root};
use crate::sessions::Sessions;
use crate::state::CurrentUser;
use crate::{AppState, admin, library, routes};
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use photosee_core::{Error, ErrorCode};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tower::ServiceExt;

pub const COOKIE: &str = "ps_user";
/// "Logged in until Switch user": a year, renewed by every login.
const COOKIE_MAX_AGE_S: u32 = 365 * 24 * 3600;

/// Builds one user's state from their roots and name (main.rs owns the
/// shared parts: file system, IO queue, thumbnail cache).
pub type MakeState = Box<dyn Fn(&str, Vec<Root>) -> AppState + Send + Sync>;

pub struct Multi {
    users: RwLock<Vec<Account>>,
    routers: RwLock<HashMap<String, Router>>,
    pub(crate) sessions: Sessions,
    hosts: Vec<String>,
    users_file: PathBuf,
    /// Everything mountable (PHOTOSEE_ROOTS), as one policy for the picker.
    pub(crate) volumes: Policy,
    make: MakeState,
}

fn read<T>(l: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    l.read().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Multi {
    pub fn new(
        users_file: PathBuf,
        sessions_file: PathBuf,
        volumes: Vec<Root>,
        hosts: Vec<String>,
        make: MakeState,
    ) -> Result<Self, String> {
        let users = accounts::load(&users_file)?;
        let sessions = Sessions::new(sessions_file, |n| users.iter().any(|u| u.name == n));
        let m = Self {
            users: RwLock::new(users),
            routers: RwLock::default(),
            sessions,
            hosts,
            users_file,
            volumes: Policy::Roots(volumes),
            make,
        };
        m.rebuild();
        Ok(m)
    }

    fn rebuild(&self) {
        let routers = read(&self.users)
            .iter()
            .map(|u| {
                (
                    u.name.clone(),
                    routes::router((self.make)(&u.name, roots_of(&u.dirs, Path::is_dir)), None),
                )
            })
            .collect();
        *self
            .routers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = routers;
    }

    pub(crate) fn users(&self) -> Vec<Account> {
        read(&self.users).clone()
    }

    /// Applies `f` to a copy of the list, saves it, then makes it current.
    /// A failed save changes nothing: never "saved" on screen but lost on disk.
    pub(crate) fn update(
        &self,
        f: impl FnOnce(&mut Vec<Account>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut guard = self
            .users
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut next = guard.clone();
        f(&mut next)?;
        accounts::save(&self.users_file, &next)?;
        *guard = next;
        drop(guard);
        self.rebuild();
        Ok(())
    }

    /// The account behind the request's cookie.
    pub(crate) fn me(&self, headers: &HeaderMap) -> Result<Account, Error> {
        let name = cookie(headers).and_then(|t| self.sessions.user_of(t));
        name.and_then(|n| read(&self.users).iter().find(|u| u.name == n).cloned())
            .ok_or_else(|| Error::new(ErrorCode::Unauthorized, "login required"))
    }
}

pub fn router(m: Multi, web_dir: PathBuf) -> Router {
    let m = Arc::new(m);
    Router::new()
        .route("/api/v1/users", get(list_users))
        .route("/api/v1/setup", post(setup))
        .route("/api/v1/login", post(login))
        .route("/api/v1/logout", post(logout))
        .merge(library::routes())
        .merge(admin::routes())
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
    let me = match m.me(req.headers()) {
        Ok(a) => a,
        Err(e) => return error_response(&e),
    };
    let Some(router) = read(&m.routers).get(&me.name).cloned() else {
        return error_response(&Error::new(ErrorCode::Unauthorized, "login required"));
    };
    // /session reports who is logged in and whether to offer the admin page.
    req.extensions_mut().insert(CurrentUser {
        name: me.name,
        admin: me.admin,
    });
    match router.oneshot(req).await {
        Ok(r) => r,
        Err(never) => match never {},
    }
}

/// Names for the login screen; an empty list means first run (setup).
async fn list_users(State(m): State<Arc<Multi>>) -> Json<Vec<String>> {
    Json(m.users().into_iter().map(|u| u.name).collect())
}

#[derive(serde::Deserialize)]
pub(crate) struct NamePin {
    pub name: String,
    pub pin: String,
}

pub(crate) fn body<T: serde::de::DeserializeOwned>(b: &Bytes) -> Result<T, Error> {
    serde_json::from_slice(b)
        .map_err(|e| Error::new(ErrorCode::Forbidden, format!("bad request body: {e}")))
}

pub(crate) fn reply(r: Result<serde_json::Value, Error>) -> Response {
    r.map_or_else(|e| error_response(&e), |v| Json(v).into_response())
}

fn with_cookie(name: &str, token: &str) -> Response {
    let mut resp = Json(serde_json::json!({ "user": name })).into_response();
    set_cookie(&mut resp, token, COOKIE_MAX_AGE_S);
    resp
}

fn set_cookie(resp: &mut Response, value: &str, max_age: u32) {
    let c = format!("{COOKIE}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}");
    if let Ok(v) = HeaderValue::from_str(&c) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
}

/// First run: whoever opens the page first creates the admin. Refused as
/// soon as any account exists, so it cannot be used to take over later.
async fn setup(State(m): State<Arc<Multi>>, b: Bytes) -> Response {
    let run = || -> Result<(String, String), Error> {
        let b: NamePin = body(&b)?;
        check_name(&b.name)?;
        check_pin(&b.pin)?;
        let name = b.name.clone();
        m.update(|users| {
            if !users.is_empty() {
                return Err(Error::new(ErrorCode::Forbidden, "already set up"));
            }
            users.push(Account {
                name: b.name,
                pin: b.pin,
                admin: true,
                dirs: Vec::new(),
            });
            Ok(())
        })?;
        let t = m.sessions.open(&name, new_token()?)?;
        Ok((name, t))
    };
    match run() {
        Ok((name, token)) => with_cookie(&name, &token),
        Err(e) => error_response(&e),
    }
}

async fn login(State(m): State<Arc<Multi>>, b: Bytes) -> Response {
    let run = || -> Result<(String, String), Error> {
        let b: NamePin =
            body(&b).map_err(|_| Error::new(ErrorCode::Unauthorized, "expected {name, pin}"))?;
        let real = m
            .users()
            .into_iter()
            .find(|u| u.name == b.name)
            .map(|u| u.pin);
        let t = m.sessions.login(
            &b.name,
            &b.pin,
            real.as_deref(),
            Instant::now(),
            new_token()?,
        )?;
        Ok((b.name, t))
    };
    match run() {
        Ok((name, token)) => with_cookie(&name, &token),
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
