//! /cover: a folder tile's picture is its first image's thumbnail, looking
//! into first subfolders when the folder has no images of its own.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_support.rs"]
mod support;

use axum::http::StatusCode;
use photosee_core::testfs::MemFs;
use photosee_server::queue::IoQueue;
use std::sync::Arc;
use support::*;

fn env() -> Env {
    let fs = MemFs::new();
    fs.file("/photos/series/v1/p10.bmp", &bmp(40, 30))
        .file("/photos/series/v1/p2.bmp", &bmp(30, 40))
        .file("/photos/series/v2/p1.bmp", &bmp(50, 50))
        .file("/photos/mixed/own.bmp", &bmp(20, 10))
        .file("/photos/mixed/sub/other.bmp", &bmp(10, 20))
        .file("/photos/deep/a/b/c/d/x.bmp", &bmp(10, 10))
        .file("/photos/notes/readme.txt", b"x")
        .dir("/photos/empty");
    env_with(Arc::new(fs), IoQueue::new(4))
}

#[tokio::test]
async fn a_series_shows_its_first_volume_first_page_in_natural_order() {
    let env = env();
    let cover = get(&env.app, "/api/v1/cover?p=/photos/series").await;
    let page = get(&env.app, "/api/v1/thumb?p=/photos/series/v1/p2.bmp").await;
    assert_eq!(cover.status, StatusCode::OK);
    assert_eq!(cover.header("content-type"), "image/jpeg");
    // p2 before p10 (natural), v1 before v2: byte-identical to that thumb.
    assert_eq!(cover.body, page.body);
    assert_eq!(cover.header("etag"), page.header("etag"));
    let tag = cover.header("etag").to_owned();
    let again = send(
        &env.app,
        "/api/v1/cover?p=/photos/series",
        &[("if-none-match", &tag)],
    )
    .await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn own_images_win_over_subfolders() {
    let env = env();
    let cover = get(&env.app, "/api/v1/cover?p=/photos/mixed").await;
    let own = get(&env.app, "/api/v1/thumb?p=/photos/mixed/own.bmp").await;
    assert_eq!((cover.status, &cover.body), (StatusCode::OK, &own.body));
}

#[tokio::test]
async fn no_image_within_three_levels_is_404_and_outside_the_roots_is_refused() {
    let env = env();
    for p in ["/photos/empty", "/photos/notes", "/photos/deep"] {
        let r = get(&env.app, &format!("/api/v1/cover?p={p}")).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{p}");
        assert_eq!(r.json()["code"], "not_found", "{p}");
    }
    let file = get(&env.app, "/api/v1/cover?p=/photos/mixed/own.bmp").await;
    assert_eq!(
        file.status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "a file is not a folder, as for /list"
    );
    let out = get(&env.app, "/api/v1/cover?p=/etc").await;
    assert!(
        out.status == StatusCode::FORBIDDEN || out.status == StatusCode::NOT_FOUND,
        "{:?}",
        out.status
    );
}
