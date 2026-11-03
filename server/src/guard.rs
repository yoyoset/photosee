//! Request guard middleware: Host allow-list and session token. Package H
//! (orchestrator; security-sensitive).

use crate::AppState;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

pub async fn check(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let _ = state;
    next.run(req).await // package H: enforce Host + token before passing
}
