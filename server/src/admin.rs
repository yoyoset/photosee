//! Account admin, for admins only (the first account, made at setup):
//! - `GET /admin/users` → `[{name, admin, folders}]`
//! - `POST /admin/users {name, pin}`: new reader, no folders yet; they add
//!   their own.
//! - `POST /admin/users/pin {name, pin}`: reset a PIN; that user's devices
//!   log in again.
//! - `POST /admin/users/remove {name}`: delete; never yourself, so there is
//!   always an admin left.
//!
//! What an admin cannot do is look into another user's library: there is
//! no "log in as". Folders stay each user's own business.

use crate::accounts::{Account, check_name, check_pin};
use crate::multi::{Multi, NamePin, body, reply};
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use photosee_core::{Error, ErrorCode};
use serde_json::{Value, json};
use std::sync::Arc;

pub fn routes() -> Router<Arc<Multi>> {
    Router::new()
        .route("/api/v1/admin/users", get(list).post(create))
        .route("/api/v1/admin/users/pin", post(set_pin))
        .route("/api/v1/admin/users/remove", post(remove))
}

fn admin(m: &Multi, headers: &HeaderMap) -> Result<Account, Error> {
    let me = m.me(headers)?;
    if me.admin {
        Ok(me)
    } else {
        Err(Error::new(ErrorCode::Forbidden, "admins only"))
    }
}

fn no_such() -> Error {
    Error::new(ErrorCode::NotFound, "no such user")
}

async fn list(State(m): State<Arc<Multi>>, headers: HeaderMap) -> Response {
    let run = || -> Result<Value, Error> {
        admin(&m, &headers)?;
        let users: Vec<Value> = m
            .users()
            .iter()
            .map(|u| json!({ "name": u.name, "admin": u.admin, "folders": u.dirs.len() }))
            .collect();
        Ok(json!(users))
    };
    reply(run())
}

async fn create(State(m): State<Arc<Multi>>, headers: HeaderMap, b: Bytes) -> Response {
    let run = || -> Result<Value, Error> {
        admin(&m, &headers)?;
        let b: NamePin = body(&b)?;
        check_name(&b.name)?;
        check_pin(&b.pin)?;
        m.update(|users| {
            if users.iter().any(|u| u.name == b.name) {
                return Err(Error::new(ErrorCode::Forbidden, "name already taken"));
            }
            users.push(Account {
                name: b.name,
                pin: b.pin,
                admin: false,
                dirs: Vec::new(),
            });
            Ok(())
        })?;
        Ok(json!({}))
    };
    reply(run())
}

async fn set_pin(State(m): State<Arc<Multi>>, headers: HeaderMap, b: Bytes) -> Response {
    let run = || -> Result<Value, Error> {
        admin(&m, &headers)?;
        let b: NamePin = body(&b)?;
        check_pin(&b.pin)?;
        m.update(|users| {
            let u = users
                .iter_mut()
                .find(|u| u.name == b.name)
                .ok_or_else(no_such)?;
            u.pin = b.pin;
            Ok(())
        })?;
        m.sessions.drop_user(&b.name);
        Ok(json!({}))
    };
    reply(run())
}

#[derive(serde::Deserialize)]
struct NameOnly {
    name: String,
}

async fn remove(State(m): State<Arc<Multi>>, headers: HeaderMap, b: Bytes) -> Response {
    let run = || -> Result<Value, Error> {
        let me = admin(&m, &headers)?;
        let b: NameOnly = body(&b)?;
        if b.name == me.name {
            return Err(Error::new(
                ErrorCode::Forbidden,
                "you cannot delete yourself",
            ));
        }
        m.update(|users| {
            let before = users.len();
            users.retain(|u| u.name != b.name);
            if users.len() == before {
                Err(no_such())
            } else {
                Ok(())
            }
        })?;
        m.sessions.drop_user(&b.name);
        Ok(json!({}))
    };
    reply(run())
}
