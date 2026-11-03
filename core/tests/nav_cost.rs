//! `read_dir` cost, error handling and loop safety of folder navigation.
#![allow(clippy::unwrap_used)] // integration-test helpers; AGENTS rule 4 allows unwrap in tests

use photosee_core::error::{Error, ErrorCode, Result};
use photosee_core::io::{FsBackend, ReadAt};
use photosee_core::nav::{next_folder, prev_folder};
use photosee_core::testfs::MemFs;
use photosee_core::types::{Entry, EntryKind};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Wraps a backend, counts `read_dir` calls, and can inject failures.
struct Counting<'a> {
    inner: &'a dyn FsBackend,
    calls: Mutex<Vec<PathBuf>>,
    fail: HashMap<PathBuf, ErrorCode>,
}

impl<'a> Counting<'a> {
    fn new(inner: &'a dyn FsBackend) -> Self {
        Self {
            inner,
            calls: Mutex::default(),
            fail: HashMap::new(),
        }
    }
    fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    fn reset(&self) {
        self.calls.lock().unwrap().clear();
    }
}

impl FsBackend for Counting<'_> {
    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>> {
        self.calls.lock().unwrap().push(dir.to_path_buf());
        if let Some(code) = self.fail.get(dir) {
            return Err(Error::new(*code, "injected"));
        }
        self.inner.read_dir(dir)
    }
    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>> {
        self.inner.open(path)
    }
    fn stat(&self, path: &Path) -> Result<Entry> {
        self.inner.stat(path)
    }
}

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

#[test]
fn next_from_a_leaf_with_one_following_sibling_costs_three_read_dirs() {
    let mem = MemFs::new();
    mem.file("/r/p/a/1.jpg", b"x").file("/r/p/b/2.jpg", b"x");
    let fs = Counting::new(&mem);
    let got = next_folder(&fs, &p("/r"), &p("/r/p/a")).unwrap();
    assert_eq!(got, Some(p("/r/p/b")));
    // a (its children), p (its siblings), b (does it have images?).
    assert_eq!(fs.count(), 3, "{:?}", fs.calls.lock().unwrap());
    fs.reset();
    let got = prev_folder(&fs, &p("/r"), &p("/r/p/b")).unwrap();
    assert_eq!(got, Some(p("/r/p/a")));
    // p (its siblings), a (images and children). prev never lists current.
    assert_eq!(fs.count(), 2, "{:?}", fs.calls.lock().unwrap());
}

#[test]
fn wide_root_cost_does_not_grow_with_sibling_count() {
    let mem = MemFs::new();
    for i in 0..500 {
        mem.file(&format!("/w/f{i:04}/1.jpg"), b"x");
    }
    let fs = Counting::new(&mem);
    let root = p("/w");
    assert_eq!(
        next_folder(&fs, &root, &p("/w/f0100")).unwrap(),
        Some(p("/w/f0101"))
    );
    let next_cost = fs.count();
    fs.reset();
    assert_eq!(
        prev_folder(&fs, &root, &p("/w/f0100")).unwrap(),
        Some(p("/w/f0099"))
    );
    println!("wide 500: next={next_cost} prev={}", fs.count());
    assert!(next_cost <= 3 && fs.count() <= 3);
}

#[test]
fn climbing_over_empty_folders_costs_one_read_dir_each() {
    let mem = deep_tree();
    let fs = Counting::new(&mem);
    // From deep: deep, b1 (no later sibling), b (siblings), b2 (images?).
    let got = next_folder(&fs, &p("/r"), &p("/r/b/b1/deep")).unwrap();
    assert_eq!(got, Some(p("/r/b/b2")));
    assert_eq!(fs.count(), 4);
    fs.reset();
    // From b2: b2, b (siblings: none later), r (siblings: c empty, d).
    let got = next_folder(&fs, &p("/r"), &p("/r/b/b2")).unwrap();
    assert_eq!(got, Some(p("/r/d")));
    println!("b2 -> d reads: {:?}", fs.calls.lock().unwrap());
    assert_eq!(fs.count(), 5);
}

#[test]
fn unreadable_neighbour_is_skipped_but_timeouts_surface() {
    let mem = deep_tree();
    let mut fs = Counting::new(&mem);
    fs.fail.insert(p("/r/b/b2"), ErrorCode::Forbidden);
    fs.fail.insert(p("/r/c"), ErrorCode::NotFound);
    let got = next_folder(&fs, &p("/r"), &p("/r/b/b1/deep")).unwrap();
    assert_eq!(
        got,
        Some(p("/r/d")),
        "forbidden and vanished folders are skipped"
    );
    fs.fail.insert(p("/r/b/b2"), ErrorCode::Timeout);
    let err = next_folder(&fs, &p("/r"), &p("/r/b/b1/deep")).unwrap_err();
    assert_eq!(err.code, ErrorCode::Timeout);
    // The folder the user is in failing is always an error.
    fs.fail.insert(p("/r/d"), ErrorCode::NotFound);
    let err = next_folder(&fs, &p("/r"), &p("/r/d")).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

/// Every directory holds one sub-directory called "loop" and nothing else.
struct Endless(Mutex<usize>);

impl FsBackend for Endless {
    fn read_dir(&self, _dir: &Path) -> Result<Vec<Entry>> {
        *self.0.lock().unwrap() += 1;
        Ok(vec![Entry {
            name: "loop".into(),
            kind: EntryKind::Dir,
            size: 0,
            mtime_ms: 0,
        }])
    }
    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>> {
        Err(Error::new(ErrorCode::NotFound, path.display().to_string()))
    }
    fn stat(&self, path: &Path) -> Result<Entry> {
        Err(Error::new(ErrorCode::NotFound, path.display().to_string()))
    }
}

#[test]
fn directory_loops_end_instead_of_overflowing_the_stack() {
    let fs = Endless(Mutex::new(0));
    assert_eq!(next_folder(&fs, &p("/loop"), &p("/loop")).unwrap(), None);
    let n = *fs.0.lock().unwrap();
    assert!(n <= 70, "descent must be capped, read {n} dirs");
    *fs.0.lock().unwrap() = 0;
    let deep = p("/loop/loop");
    assert_eq!(prev_folder(&fs, &p("/loop"), &deep).unwrap(), None);
    assert!(*fs.0.lock().unwrap() <= 4);
}
