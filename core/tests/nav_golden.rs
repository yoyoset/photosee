//! Golden cases from contracts/nav.json against the docs/api.md fixture tree.

use photosee_core::ErrorCode;
use photosee_core::nav::{next_folder, prev_folder};
use photosee_core::testfs::MemFs;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn golden_tree() -> MemFs {
    let fs = MemFs::new();
    fs.file("/photos/a10.jpg", b"x")
        .file("/photos/a2.jpg", b"x")
        .file("/photos/.hidden.jpg", b"x")
        .file("/photos/notes.txt", b"x")
        .file("/photos/2023/b.png", b"x")
        .dir("/photos/2024")
        .file("/photos/2025/c.jpg", b"x");
    fs
}

#[test]
fn every_case_in_nav_json_matches() {
    let fs = golden_tree();
    let doc: Value = serde_json::from_str(include_str!("../../contracts/nav.json")).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let p = Path::new(case["p"].as_str().unwrap());
        let root = Path::new(case["root"].as_str().unwrap());
        // Compared as paths, not strings: on Windows `join` yields `\` between
        // the segments it adds, and that is the same path.
        let want = case["dir"].as_str().map(PathBuf::from);
        let got = match case["d"].as_str().unwrap() {
            "next" => next_folder(&fs, root, p),
            "prev" => prev_folder(&fs, root, p),
            other => panic!("unknown direction {other}"),
        };
        assert_eq!(got.unwrap(), want, "case {case}");
    }
}

#[test]
fn golden_tree_outside_root_is_forbidden() {
    let fs = golden_tree();
    let root = Path::new("/photos/2023");
    for f in [next_folder, prev_folder] {
        let err = f(&fs, root, Path::new("/photos/2025")).unwrap_err();
        assert_eq!(err.code, ErrorCode::Forbidden);
    }
}
