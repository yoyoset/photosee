//! `ArchiveFs`: .cbz files behave as folders through every FsBackend call.
#![allow(clippy::unwrap_used, reason = "test helpers")]

#[path = "archive_util.rs"]
mod util;

use photosee_core::archive::ArchiveFs;
use photosee_core::io::{FsBackend, read_all};
use photosee_core::testfs::MemFs;
use photosee_core::{EntryKind, ErrorCode, Format, nav, scan, thumb};
use std::path::Path;
use util::*;

fn names(fs: &dyn FsBackend, dir: &str) -> (Vec<String>, Vec<String>) {
    let l = scan::list(fs, Path::new(dir)).unwrap();
    let n = |v: Vec<photosee_core::Entry>| v.into_iter().map(|e| e.name).collect();
    (n(l.images), n(l.dirs))
}

fn library() -> ArchiveFs<MemFs> {
    let mem = MemFs::new();
    let page = png(8, 6);
    mem.file(
        "/lib/vol2.cbz",
        &zip(&[
            stored("p10.png", &page),
            deflated("p2.png", &page),
            stored("p1.png", &page),
        ]),
    )
    .file("/lib/vol1.cbz", &zip(&[stored("a.png", &page)]))
    .file("/lib/cover.jpg", b"x")
    .file("/lib/notes.zip", b"PK");
    ArchiveFs::new(mem)
}

#[test]
fn archives_list_as_folders_and_plain_zip_does_not() {
    let fs = library();
    assert_eq!(
        names(&fs, "/lib"),
        (
            vec!["cover.jpg".into()],
            vec!["vol1.cbz".into(), "vol2.cbz".into()]
        )
    );
    assert_eq!(
        fs.stat(Path::new("/lib/vol1.cbz")).unwrap().kind,
        EntryKind::Dir
    );
}

#[test]
fn pages_are_naturally_sorted_images() {
    let fs = library();
    let (images, dirs) = names(&fs, "/lib/vol2.cbz");
    assert_eq!(images, ["p1.png", "p2.png", "p10.png"]);
    assert!(dirs.is_empty());
    let e = fs.stat(Path::new("/lib/vol2.cbz/p2.png")).unwrap();
    assert_eq!(e.kind, EntryKind::Image(Format::Png));
    assert_eq!(e.size, png(8, 6).len() as u64);
}

#[test]
fn stored_and_deflated_pages_read_back_identically() {
    let fs = library();
    let page = png(8, 6);
    for p in ["/lib/vol2.cbz/p1.png", "/lib/vol2.cbz/p2.png"] {
        let src = fs.open(Path::new(p)).unwrap();
        assert_eq!(read_all(src.as_ref()).unwrap(), page, "{p}");
    }
}

#[test]
fn thumbnail_renders_from_a_deflated_page() {
    let fs = library();
    let t = thumb::make(&fs, Path::new("/lib/vol2.cbz/p2.png"), 64).unwrap();
    assert!(!t.bytes.is_empty());
}

#[test]
fn lone_wrapper_folder_is_collapsed_but_real_subfolders_stay() {
    let mem = MemFs::new();
    let page = png(4, 4);
    mem.file(
        "/c/wrapped.cbz",
        &zip(&[
            stored("Vol 1/", b""),
            stored("Vol 1/01.png", &page),
            stored("Vol 1/02.png", &page),
        ]),
    )
    .file(
        "/c/chapters.cbz",
        &zip(&[stored("ch1/01.png", &page), stored("ch2/01.png", &page)]),
    );
    let fs = ArchiveFs::new(mem);
    assert_eq!(names(&fs, "/c/wrapped.cbz").0, ["01.png", "02.png"]);
    assert_eq!(names(&fs, "/c/chapters.cbz").1, ["ch1", "ch2"]);
    assert_eq!(names(&fs, "/c/chapters.cbz/ch2").0, ["01.png"]);
    let src = fs.open(Path::new("/c/wrapped.cbz/02.png")).unwrap();
    assert_eq!(read_all(src.as_ref()).unwrap(), page);
}

#[test]
fn hostile_names_are_hidden() {
    let mem = MemFs::new();
    let page = png(4, 4);
    mem.file(
        "/h/x.cbz",
        &zip(&[
            stored("../escape.png", &page),
            stored("C:evil.png", &page),
            stored("__MACOSX/._a.png", &page),
            stored("ok.png", &page),
            stored("readme.txt", b"hi"),
        ]),
    );
    let fs = ArchiveFs::new(mem);
    assert_eq!(names(&fs, "/h/x.cbz"), (vec!["ok.png".into()], vec![]));
}

#[test]
fn missing_member_and_folder_are_not_found() {
    let fs = library();
    let e = fs.open(Path::new("/lib/vol2.cbz/nope.png")).err().unwrap();
    assert_eq!(e.code, ErrorCode::NotFound);
    let e = fs.read_dir(Path::new("/lib/vol2.cbz/nope")).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn corrupt_archive_lists_as_folder_but_fails_to_open_as_corrupt() {
    let mem = MemFs::new();
    mem.file("/b/broken.cbz", b"PK\x03\x04 not really a zip");
    let fs = ArchiveFs::new(mem);
    assert_eq!(names(&fs, "/b").1, ["broken.cbz"]);
    let e = fs.read_dir(Path::new("/b/broken.cbz")).unwrap_err();
    assert_eq!(e.code, ErrorCode::Corrupt);
}

#[test]
fn next_folder_walks_from_one_volume_to_the_next() {
    let fs = library();
    let next = nav::next_folder(&fs, Path::new("/lib"), Path::new("/lib/vol1.cbz")).unwrap();
    assert_eq!(next.as_deref(), Some(Path::new("/lib/vol2.cbz")));
}
