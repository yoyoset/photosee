//! Transport (b): Tauri asynchronous custom protocol, `http://ps.localhost/`
//! on Windows.
//!
//! Inherent difference from axum: the responder takes one complete body, so
//! the whole file is read into memory before the first byte reaches WebView2
//! (wry then wraps it in an IStream). There is no streaming option here.
use std::collections::HashMap;

use tauri::http::{Request, Response, StatusCode, header};

use crate::server::{Shared, sample_path};

pub const SCHEME: &str = "ps";

fn query(uri: &tauri::http::Uri) -> HashMap<String, String> {
    uri.query()
        .unwrap_or("")
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut r = Response::new(Vec::new());
    *r.status_mut() = code;
    r
}

pub async fn serve(st: Shared, req: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let q = query(req.uri());
    let Some(path) = q.get("n").and_then(|n| sample_path(&st.samples, n)) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return status(StatusCode::NOT_FOUND);
    };
    let mut r = Response::new(bytes);
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("image/jpeg"));
    h.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    // The page lives on 127.0.0.1, so fetch() of this origin needs CORS; <img>
    // does not, but the transfer-only measurement uses fetch().
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, header::HeaderValue::from_static("*"));
    r
}
