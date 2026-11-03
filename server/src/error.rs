//! Error → HTTP response, the one body shape of docs/api.md §Errors.
//! Contract file: orchestrator only (guard and routes both answer with it).

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use photosee_core::Error;
use serde::Serialize;

#[derive(Serialize)]
struct Body<'a> {
    #[serde(flatten)]
    code: photosee_core::ErrorCode,
    message: &'a str,
}

pub fn error_response(e: &Error) -> Response {
    let status =
        StatusCode::from_u16(e.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = serde_json::to_vec(&Body {
        code: e.code,
        message: &e.message,
    })
    // Serializing an enum and a &str cannot fail.
    .unwrap_or_default();
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// Lets handlers return `Result<_, ApiError>` and use `?` on core results.
pub struct ApiError(pub Error);

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        error_response(&self.0)
    }
}
