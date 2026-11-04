//! /img and /thumb: delivery choice, Range, conditional requests, errors.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_jpeg.rs"]
mod jpeg;
#[path = "api_support.rs"]
mod support;

use axum::http::StatusCode;
use photosee_core::testfs::MemFs;
use photosee_server::queue::IoQueue;
use serde_json::json;
use std::sync::Arc;
use support::*;

const SMALL: &str = "/api/v1/img?p=/photos/small.jpg&w=100&h=100";

/// 100x100 JPEG skeleton with 3000 body bytes, a 400x300 BMP, and a RAW-like
/// name that is not an image.
fn env() -> (Env, Vec<u8>, Arc<MemFs>) {
    let fs = Arc::new(golden_tree());
    let small = jpeg::skeleton(100, 100, 1, 3000);
    fs.file("/photos/small.jpg", &small)
        .file("/photos/wide.bmp", &bmp(400, 300))
        .file("/photos/rot.jpg", &jpeg::skeleton(100, 50, 6, 10));
    (env_with(fs.clone(), IoQueue::new(4)), small, fs)
}

#[tokio::test]
async fn passthrough_sends_the_file_with_caching_headers() {
    let (env, bytes, _) = env();
    let r = get(&env.app, SMALL).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body, bytes);
    assert_eq!(r.header("content-type"), "image/jpeg");
    assert_eq!(r.header("x-photosee-delivery"), "passthrough");
    assert_eq!(r.header("cache-control"), "private, max-age=60");
    assert_eq!(r.header("accept-ranges"), "bytes");
    assert!(r.header("etag").starts_with('"'), "{}", r.header("etag"));
}

#[tokio::test]
async fn omitting_the_box_keeps_original_pixels() {
    let (env, bytes, _) = env();
    let r = get(&env.app, "/api/v1/img?p=/photos/small.jpg").await;
    assert_eq!(r.header("x-photosee-delivery"), "passthrough");
    assert_eq!(r.body, bytes);
}

#[tokio::test]
async fn range_requests_get_206_and_416() {
    let (env, bytes, _) = env();
    let n = bytes.len();
    let r = send(&env.app, SMALL, &[("range", "bytes=10-19")]).await;
    assert_eq!(r.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(r.body, &bytes[10..20]);
    assert_eq!(r.header("content-range"), format!("bytes 10-19/{n}"));
    assert_eq!(r.header("x-photosee-delivery"), "passthrough");

    let tail = send(&env.app, SMALL, &[("range", "bytes=-5")]).await;
    assert_eq!(tail.body, &bytes[n - 5..]);
    assert_eq!(
        tail.header("content-range"),
        format!("bytes {}-{}/{n}", n - 5, n - 1)
    );

    let past = send(&env.app, SMALL, &[("range", &format!("bytes={n}-"))]).await;
    assert_eq!(past.status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(past.header("content-range"), format!("bytes */{n}"));
    assert_eq!(past.json()["code"], json!("unsupported"));

    // Multi-range and nonsense are ignored: the whole file, status 200.
    for h in ["bytes=0-1,5-6", "lines=1-2", "bytes=9-3"] {
        let r = send(&env.app, SMALL, &[("range", h)]).await;
        assert_eq!((r.status, r.body.len()), (StatusCode::OK, n), "{h}");
    }
}

#[tokio::test]
async fn if_none_match_answers_304_without_work() {
    let (env, _, fs) = env();
    let etag = get(&env.app, SMALL).await.header("etag").to_owned();
    let r = send(&env.app, SMALL, &[("if-none-match", &etag)]).await;
    assert_eq!(r.status, StatusCode::NOT_MODIFIED);
    assert!(r.body.is_empty());
    assert_eq!(r.header("etag"), etag);
    assert_eq!(r.header("cache-control"), "private, max-age=60");
    // Another box is another representation, so another tag.
    let other = get(&env.app, "/api/v1/img?p=/photos/small.jpg&w=101&h=100").await;
    assert_ne!(other.header("etag"), etag);
    // The file changed: the old tag no longer matches, even at the same URL.
    fs.file("/photos/small.jpg", &jpeg::skeleton(100, 100, 1, 3001));
    let changed = send(&env.app, SMALL, &[("if-none-match", &etag)]).await;
    assert_eq!(changed.status, StatusCode::OK);
    // A 304 must not decode: a corrupt file with a matching tag still 304s.
    fs.file("/photos/bad.jpg", b"not a jpeg");
    let bad_uri = "/api/v1/img?p=/photos/bad.jpg&w=10&h=10";
    let broken = get(&env.app, bad_uri).await;
    assert_eq!(broken.status, StatusCode::UNPROCESSABLE_ENTITY);
    // `*` matches any tag, so this is a 304 only if nothing was decoded.
    let hit = send(&env.app, bad_uri, &[("if-none-match", "*")]).await;
    assert_eq!(hit.status, StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn large_images_are_transcoded_to_jpeg() {
    let (env, _, _) = env();
    let r = get(&env.app, "/api/v1/img?p=/photos/wide.bmp&w=100&h=75").await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("x-photosee-delivery"), "transcode");
    assert_eq!(r.header("content-type"), "image/jpeg");
    assert_eq!(&r.body[..3], &[0xFF, 0xD8, 0xFF]);
    assert!(r.body.len() < 400 * 300 * 3 / 4, "{} bytes", r.body.len());
    // Range does not apply to a generated body.
    let ranged = send(
        &env.app,
        "/api/v1/img?p=/photos/wide.bmp&w=100&h=75",
        &[("range", "bytes=0-9")],
    )
    .await;
    assert_eq!(
        (ranged.status, ranged.body.len()),
        (StatusCode::OK, r.body.len())
    );
    assert!(ranged.headers.get("content-range").is_none());
}

/// A real 1x1 lossless WebP.
const WEBP: [u8; 34] = [
    0x52, 0x49, 0x46, 0x46, 0x1A, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x4C,
    0x0D, 0x00, 0x00, 0x00, 0x2F, 0x00, 0x00, 0x00, 0x10, 0x07, 0x10, 0x11, 0x11, 0x88, 0x88, 0xFE,
    0x07, 0x00,
];

#[tokio::test]
async fn accept_header_decides_native_formats() {
    let (env, _, fs) = env();
    fs.file("/photos/p.webp", &WEBP);
    // Big request box: only the format decides passthrough vs transcode.
    let uri = "/api/v1/img?p=/photos/p.webp&w=100&h=100";
    let plain = get(&env.app, uri).await;
    assert_eq!(plain.header("x-photosee-delivery"), "transcode");
    // The stub has an alpha channel, so the transcode is a PNG.
    assert_eq!(plain.header("content-type"), "image/png");
    for accept in ["image/avif,image/webp,*/*", "IMAGE/WEBP;q=0.9"] {
        let r = send(&env.app, uri, &[("accept", accept)]).await;
        assert_eq!(r.header("x-photosee-delivery"), "passthrough", "{accept}");
        assert_eq!(r.header("content-type"), "image/webp");
        assert_eq!(r.body, WEBP);
        assert_eq!(r.header("vary"), "Accept");
    }
    // A wildcard names no format, and q=0 refuses one.
    for accept in ["image/*,*/*", "image/webp;q=0"] {
        let r = send(&env.app, uri, &[("accept", accept)]).await;
        assert_eq!(r.header("x-photosee-delivery"), "transcode", "{accept}");
    }
}

#[tokio::test]
async fn bad_requests_and_missing_files_use_the_error_body() {
    let (env, _, _) = env();
    for uri in [
        "/api/v1/img?p=/photos/small.jpg&w=100",
        "/api/v1/img?p=/photos/small.jpg&h=100",
        "/api/v1/img?p=/photos/small.jpg&w=0&h=5",
        "/api/v1/img?p=/photos/small.jpg&w=x&h=5",
        "/api/v1/img?p=/photos/small.jpg&prio=later",
        "/api/v1/img",
    ] {
        let r = get(&env.app, uri).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(r.json()["code"], json!("unsupported"), "{uri}");
    }
    let missing = get(&env.app, "/api/v1/img?p=/photos/nope.jpg&w=9&h=9").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json()["code"], json!("not_found"));
    let escape = get(&env.app, "/api/v1/img?p=/photos/../x.jpg&w=9&h=9").await;
    assert_eq!(escape.status, StatusCode::FORBIDDEN);
    assert_eq!(escape.json()["code"], json!("forbidden"));
    let dir = get(&env.app, "/api/v1/img?p=/photos/2023&w=9&h=9").await;
    assert_eq!(dir.json()["code"], json!("unsupported"));
    let huge = get(
        &env.app,
        "/api/v1/img?p=/photos/small.jpg&w=4294967295&h=4294967295",
    )
    .await;
    assert_eq!(
        huge.status,
        StatusCode::OK,
        "oversized box must not overflow"
    );
}

#[tokio::test]
async fn prefetch_priority_is_accepted() {
    let (env, bytes, _) = env();
    let r = get(&env.app, &format!("{SMALL}&prio=prefetch")).await;
    assert_eq!((r.status, r.body), (StatusCode::OK, bytes));
}

#[tokio::test]
async fn thumbs_are_cached_on_disk_and_clamped() {
    let (env, _, fs) = env();
    let uri = "/api/v1/thumb?p=/photos/wide.bmp";
    let first = get(&env.app, uri).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.header("content-type"), "image/jpeg");
    assert_eq!(first.header("cache-control"), "private, max-age=60");
    // Longest edge 256 by default: 400x300 becomes 256x192.
    assert_eq!(jpeg_size(&first.body), (256, 192));
    // Same size, garbage bytes: only the cache can still answer.
    let wide = bmp(400, 300);
    fs.file("/photos/wide.bmp", &vec![0u8; wide.len()]);
    let second = get(&env.app, uri).await;
    assert_eq!((second.status, &second.body), (StatusCode::OK, &first.body));
    let tag = first.header("etag").to_owned();
    let hit = send(&env.app, uri, &[("if-none-match", &tag)]).await;
    assert_eq!(hit.status, StatusCode::NOT_MODIFIED);
    // `s` is clamped to 64..=512 and is part of the cache key.
    let tiny = get(&env.app, &format!("{uri}&s=1")).await;
    assert_eq!(
        tiny.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "other key: no cached bytes"
    );
    assert_eq!(
        get(&env.app, &format!("{uri}&s=abc")).await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn thumb_edge_is_clamped() {
    let (env, _, _) = env();
    let small = get(&env.app, "/api/v1/thumb?p=/photos/wide.bmp&s=1").await;
    assert_eq!(jpeg_size(&small.body), (64, 48));
    let big = get(&env.app, "/api/v1/thumb?p=/photos/wide.bmp&s=9999").await;
    assert_eq!(jpeg_size(&big.body), (400, 300), "never upscaled past 512");
    let missing = get(&env.app, "/api/v1/thumb?p=/photos/none.jpg").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

/// Width and height from the first SOF marker.
fn jpeg_size(b: &[u8]) -> (u16, u16) {
    let mut i = 2;
    while i + 9 < b.len() {
        let (m, len) = (b[i + 1], u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize);
        if (0xC0..=0xC2).contains(&m) {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]);
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]);
            return (w, h);
        }
        i += 2 + len;
    }
    panic!("no SOF in {} bytes", b.len());
}
