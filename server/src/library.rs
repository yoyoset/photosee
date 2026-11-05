//! "My folders": every logged-in user mounts and unmounts their own folders.
//!
//! - `GET /volumes?p=DIR`: the folder picker. Sub-folders only, over the
//!   volumes (PHOTOSEE_ROOTS); `p` absent or `/` lists the volumes.
//! - `POST /library/add {path}`: mount a picker path.
//! - `POST /library/remove {root}`: unmount by the root's name (the
//!   `name` of a `/session` root).
//!
//! The picker shows folder names only, no images, and paths in it are the
//! volume paths ("/disk1/Comics"), never container paths.

use crate::multi::{Multi, body, reply};
use crate::paths::Root;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use photosee_core::{Error, ErrorCode};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub fn routes() -> Router<Arc<Multi>> {
    Router::new()
        .route("/api/v1/volumes", get(volumes))
        .route("/api/v1/library/add", post(add))
        .route("/api/v1/library/remove", post(remove))
}

#[derive(Serialize)]
struct Folder {
    name: String,
    path: String,
}

fn subfolders(m: &Multi, dir: &str) -> Result<Value, Error> {
    let crate::paths::Policy::Roots(vols) = &m.volumes else {
        return Err(Error::new(ErrorCode::NotFound, "no volumes"));
    };
    if dir.is_empty() || dir == "/" {
        let dirs: Vec<Folder> = vols
            .iter()
            .map(|v| Folder {
                name: v.name.clone(),
                path: format!("/{}", v.name),
            })
            .collect();
        return Ok(json!({ "dir": "/", "parent": null, "dirs": dirs }));
    }
    let phys = m.volumes.resolve(dir)?;
    let mut dirs: Vec<Folder> = Vec::new();
    for e in std::fs::read_dir(&phys)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // `is_dir` follows symlinks; resolve() re-checks them on use.
        if name.starts_with('.') || name == "lost+found" || !e.path().is_dir() {
            continue;
        }
        dirs.push(Folder {
            path: m.volumes.to_logical(&e.path())?,
            name,
        });
    }
    dirs.sort_by_key(|f| f.name.to_lowercase());
    let parent = if vols.iter().any(|v| v.dir == phys) {
        "/".to_owned()
    } else {
        phys.parent()
            .map_or_else(|| Ok("/".to_owned()), |p| m.volumes.to_logical(p))?
    };
    Ok(json!({ "dir": m.volumes.to_logical(&phys)?, "parent": parent, "dirs": dirs }))
}

async fn volumes(
    State(m): State<Arc<Multi>>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if let Err(e) = m.me(&headers) {
        return reply(Err(e));
    }
    let dir = q.get("p").cloned().unwrap_or_default();
    let r = tokio::task::spawn_blocking(move || subfolders(&m, &dir)).await;
    reply(r.unwrap_or_else(|e| Err(Error::new(ErrorCode::Io, e.to_string()))))
}

#[derive(serde::Deserialize)]
struct AddBody {
    path: String,
}

async fn add(State(m): State<Arc<Multi>>, headers: HeaderMap, b: Bytes) -> Response {
    let run = || -> Result<Value, Error> {
        let me = m.me(&headers)?;
        let b: AddBody = body(&b)?;
        let phys = m.volumes.resolve(&b.path)?;
        if !phys.is_dir() {
            return Err(Error::new(ErrorCode::NotFound, "not a folder"));
        }
        m.update(|users| {
            let u = users
                .iter_mut()
                .find(|u| u.name == me.name)
                .ok_or_else(gone)?;
            if !u.dirs.contains(&phys) {
                u.dirs.push(phys);
            }
            Ok(())
        })?;
        Ok(json!({}))
    };
    reply(run())
}

#[derive(serde::Deserialize)]
struct RemoveBody {
    root: String,
}

async fn remove(State(m): State<Arc<Multi>>, headers: HeaderMap, b: Bytes) -> Response {
    let run = || -> Result<Value, Error> {
        let me = m.me(&headers)?;
        let b: RemoveBody = body(&b)?;
        // The same naming as the user's router, so a root name maps back to
        // exactly the folder /session showed under it.
        let roots: Vec<Root> = crate::accounts::roots_of(&me.dirs, Path::is_dir);
        let dir = roots
            .into_iter()
            .find(|r| r.name == b.root)
            .map(|r| r.dir)
            .ok_or_else(|| Error::new(ErrorCode::NotFound, "no such folder in your list"))?;
        m.update(|users| {
            let u = users
                .iter_mut()
                .find(|u| u.name == me.name)
                .ok_or_else(gone)?;
            u.dirs.retain(|d| *d != dir);
            Ok(())
        })?;
        Ok(json!({}))
    };
    reply(run())
}

fn gone() -> Error {
    Error::new(ErrorCode::Unauthorized, "this account was deleted")
}
