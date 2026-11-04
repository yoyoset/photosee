//! GET /nav?p=DIR&root=ROOT&d=next|prev

use super::http::{Q, Reply, bad_request, queued};
use crate::AppState;
use crate::queue::Priority;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use photosee_core::{Result, nav};
use serde::Serialize;

#[derive(Serialize)]
struct Out {
    dir: Option<String>,
}

#[derive(Clone, Copy)]
enum Dir {
    Next,
    Prev,
}

pub async fn get(State(state): State<AppState>, q: Q) -> Reply {
    let dir = match q.need("d")? {
        "next" => Dir::Next,
        "prev" => Dir::Prev,
        other => {
            return Err(bad_request(format!(
                "`d` must be next or prev, not {other:?}"
            )));
        }
    };
    let (p, root) = (q.need("p")?.to_owned(), q.need("root")?.to_owned());
    let out = queued(&state.clone(), Priority::Current, move || {
        step(&state, &p, &root, dir)
    })
    .await?;
    Ok(Json(out).into_response())
}

fn step(state: &AppState, p: &str, root: &str, dir: Dir) -> Result<Out> {
    // Both go through the policy: `root` is client input like `p`.
    let (cur, root) = (state.policy.resolve(p)?, state.policy.resolve(root)?);
    let found = match dir {
        Dir::Next => nav::next_folder(&*state.fs, &root, &cur)?,
        Dir::Prev => nav::prev_folder(&*state.fs, &root, &cur)?,
    };
    Ok(Out {
        dir: found.map(|f| state.policy.to_logical(&f)).transpose()?,
    })
}
