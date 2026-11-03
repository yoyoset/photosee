//! Error bodies match the golden contract files exactly.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

use photosee_core::error::{Codec, Error, ErrorCode};
use photosee_server::error::error_response;

async fn body(e: Error) -> (u16, serde_json::Value) {
    let r = error_response(&e);
    let status = r.status().as_u16();
    let bytes = axum::body::to_bytes(r.into_body(), 4096).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn golden(name: &str) -> serde_json::Value {
    let path = format!("{}/../contracts/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[tokio::test]
async fn error_bodies_match_golden_files() {
    let want = golden("error-missing-codec.json");
    let msg = want["message"].as_str().unwrap().to_owned();
    let (status, got) = body(Error::new(ErrorCode::MissingCodec(Codec::HevcExt), msg)).await;
    assert_eq!((status, got), (415, want));

    let want = golden("error-not-found.json");
    let msg = want["message"].as_str().unwrap().to_owned();
    let (status, got) = body(Error::new(ErrorCode::NotFound, msg)).await;
    assert_eq!((status, got), (404, want));
}
