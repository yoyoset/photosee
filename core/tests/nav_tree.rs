//! Tree shapes for next_folder / prev_folder: deep trees, empty folders, an
//! oracle (full pre-order walk) comparison and the outside-root error.
#![allow(clippy::unwrap_used)] // integration-test helpers; AGENTS rule 4 allows unwrap in tests

use photosee_core::error::ErrorCode;
use photosee_core::io::FsBackend;
use photosee_core::nav::{next_folder, prev_folder};
use photosee_core::scan;
use photosee_core::testfs::MemFs;
use std::path::{Path, PathBuf};

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

/// Root has no images. Pre-order sequence of folders that do:
/// a, b/b1/deep, b/b2, d. Folders b, b/b1, c, d/z are empty or intermediate.
fn deep_tree() -> MemFs {
    let fs = MemFs::new();
    fs.file("/r/a/1.jpg", b"x")
        .file("/r/b/b1/deep/p.jpg", b"x")
        .file("/r/b/b2/x.jpg", b"x")
        .dir("/r/c")
        .file("/r/d/y.jpg", b"x")
        .dir("/r/d/z");
    fs
}

fn all_dirs(fs: &dyn FsBackend, dir: &Path, out: &mut Vec<PathBuf>) {
    out.push(dir.to_path_buf());
    for d in scan::list(fs, dir).unwrap().dirs {
        all_dirs(fs, &dir.join(d.name), out);
    }
}

/// The spec written the slow way: the whole pre-order walk, filtered.
fn oracle_sequence(fs: &dyn FsBackend, root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    all_dirs(fs, root, &mut dirs);
    dirs.retain(|d| !scan::list(fs, d).unwrap().images.is_empty());
    dirs
}

/// For every folder (empty ones included) the fast code must agree with the
/// oracle: next = first S member after it in pre-order, prev = last before it.
fn assert_matches_oracle(fs: &dyn FsBackend, root: &Path) {
    let mut order = Vec::new();
    all_dirs(fs, root, &mut order);
    let seq = oracle_sequence(fs, root);
    for (i, cur) in order.iter().enumerate() {
        let want_next = order[i + 1..].iter().find(|d| seq.contains(d)).cloned();
        let want_prev = order[..i].iter().rev().find(|d| seq.contains(d)).cloned();
        assert_eq!(
            next_folder(fs, root, cur).unwrap(),
            want_next,
            "next {cur:?}"
        );
        assert_eq!(
            prev_folder(fs, root, cur).unwrap(),
            want_prev,
            "prev {cur:?}"
        );
    }
}

#[test]
fn three_levels_with_empty_intermediates() {
    let fs = deep_tree();
    let root = p("/r");
    let seq = oracle_sequence(&fs, &root);
    assert_eq!(seq, [p("/r/a"), p("/r/b/b1/deep"), p("/r/b/b2"), p("/r/d")]);
    // Walk forward from the root, then back from the end.
    let mut at = root.clone();
    let mut seen = Vec::new();
    while let Some(n) = next_folder(&fs, &root, &at).unwrap() {
        seen.push(n.clone());
        at = n;
    }
    assert_eq!(seen, seq);
    let mut back = Vec::new();
    while let Some(n) = prev_folder(&fs, &root, &at).unwrap() {
        back.push(n.clone());
        at = n;
    }
    seen.pop();
    seen.reverse();
    assert_eq!(back, seen);
}

#[test]
fn current_may_be_an_empty_folder() {
    let fs = deep_tree();
    let root = p("/r");
    // /r/c is empty and sits between b's subtree and d.
    assert_eq!(
        next_folder(&fs, &root, &p("/r/c")).unwrap(),
        Some(p("/r/d"))
    );
    assert_eq!(
        prev_folder(&fs, &root, &p("/r/c")).unwrap(),
        Some(p("/r/b/b2"))
    );
    // /r/b is an image-less parent: next goes down into it, prev goes to a.
    assert_eq!(
        next_folder(&fs, &root, &p("/r/b")).unwrap(),
        Some(p("/r/b/b1/deep"))
    );
    assert_eq!(
        prev_folder(&fs, &root, &p("/r/b")).unwrap(),
        Some(p("/r/a"))
    );
    // /r/d/z is d's empty child: d is before it, nothing after it.
    assert_eq!(next_folder(&fs, &root, &p("/r/d/z")).unwrap(), None);
    assert_eq!(
        prev_folder(&fs, &root, &p("/r/d/z")).unwrap(),
        Some(p("/r/d"))
    );
}

#[test]
fn fast_walk_agrees_with_full_walk_oracle() {
    let fs = deep_tree();
    assert_matches_oracle(&fs, &p("/r"));
    // Same tree with images at the root and a case/number-sensitive layout.
    let fs = MemFs::new();
    fs.file("/t/top.jpg", b"x")
        .file("/t/d10/x.jpg", b"x")
        .file("/t/d2/x.jpg", b"x")
        .file("/t/D2/inner/x.jpg", b"x")
        .dir("/t/d2/empty")
        .file("/t/.hid/x.jpg", b"x")
        .file("/t/@eaDir/x.jpg", b"x");
    assert_matches_oracle(&fs, &p("/t"));
    let seq = oracle_sequence(&fs, &p("/t"));
    assert_eq!(seq, [p("/t"), p("/t/D2/inner"), p("/t/d2"), p("/t/d10")]);
}

#[test]
fn root_without_images_and_no_image_folders_anywhere() {
    let fs = MemFs::new();
    fs.dir("/e/one").dir("/e/two/three");
    let root = p("/e");
    for cur in ["/e", "/e/one", "/e/two", "/e/two/three"] {
        assert_eq!(next_folder(&fs, &root, &p(cur)).unwrap(), None, "{cur}");
        assert_eq!(prev_folder(&fs, &root, &p(cur)).unwrap(), None, "{cur}");
    }
}

#[test]
fn root_with_images_is_first_and_prev_of_first_child() {
    let fs = MemFs::new();
    fs.file("/r/top.jpg", b"x").file("/r/a/1.jpg", b"x");
    let root = p("/r");
    assert_eq!(next_folder(&fs, &root, &root).unwrap(), Some(p("/r/a")));
    assert_eq!(
        prev_folder(&fs, &root, &p("/r/a")).unwrap(),
        Some(root.clone())
    );
    assert_eq!(prev_folder(&fs, &root, &root).unwrap(), None);
}

#[test]
fn current_outside_root_is_forbidden() {
    let fs = deep_tree();
    fs.file("/rx/1.jpg", b"x");
    let root = p("/r");
    for bad in ["/rx", "/other", "/", "/r/../rx"] {
        for f in [next_folder, prev_folder] {
            let err = f(&fs, &root, &p(bad)).unwrap_err();
            assert_eq!(err.code, ErrorCode::Forbidden, "{bad}");
        }
    }
}
