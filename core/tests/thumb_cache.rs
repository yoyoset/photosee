//! `ThumbCache`: round trip, LRU order, budget, failure tolerance.
#![allow(clippy::unwrap_used, reason = "test code")]

use photosee_core::thumbcache::ThumbCache;
use std::fs::File;
use std::path::Path;
use std::time::{Duration, SystemTime};

const KB: usize = 1000;

fn key(n: u8) -> String {
    format!("{n:02x}{n:02x}00000000000a")
}

fn file_of(dir: &Path, k: &str) -> std::path::PathBuf {
    dir.join(&k[..2]).join(format!("{k}.jpg"))
}

fn set_age(dir: &Path, k: &str, secs_ago: u64) {
    let t = SystemTime::now() - Duration::from_secs(secs_ago);
    File::options()
        .write(true)
        .open(file_of(dir, k))
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn exists(dir: &Path, k: &str) -> bool {
    file_of(dir, k).exists()
}

#[test]
fn round_trip_and_layout() {
    let dir = tempfile::tempdir().unwrap();
    let c = ThumbCache::new(dir.path().to_path_buf(), 1 << 20);
    assert_eq!(c.get(&key(1)), None);
    c.put(&key(1), b"hello").unwrap();
    assert_eq!(c.get(&key(1)).as_deref(), Some(&b"hello"[..]));
    assert!(exists(dir.path(), &key(1)), "dir/<first 2 hex>/<key>.jpg");
    c.put(&key(1), b"replaced").unwrap();
    assert_eq!(c.get(&key(1)).as_deref(), Some(&b"replaced"[..]));
    // No temp files left behind.
    let leftovers = std::fs::read_dir(dir.path().join("01"))
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().unwrap() != "jpg")
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn eviction_is_least_recently_used() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    // Budget 5 KB; each entry 1 KB.
    let c = ThumbCache::new(p.to_path_buf(), 5 * KB as u64);
    let blob = vec![7u8; KB];
    for n in 1..=5 {
        c.put(&key(n), &blob).unwrap();
    }
    // Known ages: 1 is oldest, 5 newest.
    for n in 1..=5u8 {
        set_age(p, &key(n), 100 - u64::from(n) * 10);
    }
    // Reading entry 1 makes it the most recently used.
    assert!(c.get(&key(1)).is_some());
    // 6th entry crosses the budget: 6 KB > 5 KB, evict down to 90% (4.5 KB),
    // i.e. the oldest two (2 and 3).
    c.put(&key(6), &blob).unwrap();
    assert!(exists(p, &key(1)), "bumped by get");
    assert!(!exists(p, &key(2)));
    assert!(!exists(p, &key(3)));
    for n in [4, 5, 6] {
        assert!(exists(p, &key(n)), "{n}");
    }
}

#[test]
fn existing_files_count_towards_the_budget_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    {
        let c = ThumbCache::new(p.to_path_buf(), 10 * KB as u64);
        for n in 1..=3 {
            c.put(&key(n), &vec![0u8; 3 * KB]).unwrap();
        }
        set_age(p, &key(1), 300);
    }
    // A fresh instance learns the 9 KB on disk from one scan on first put.
    let c = ThumbCache::new(p.to_path_buf(), 10 * KB as u64);
    c.put(&key(4), &vec![0u8; 3 * KB]).unwrap();
    assert!(!exists(p, &key(1)), "oldest evicted");
    assert!(exists(p, &key(4)));
}

#[test]
fn budget_zero_disables_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let c = ThumbCache::new(dir.path().to_path_buf(), 0);
    c.put(&key(1), b"data").unwrap();
    assert_eq!(c.get(&key(1)), None);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn unwritable_directory_is_swallowed() {
    let dir = tempfile::tempdir().unwrap();
    // A directory path that sits below a regular file can never be created.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let c = ThumbCache::new(blocker.join("cache"), 1 << 20);
    c.put(&key(1), b"data").unwrap();
    assert_eq!(c.get(&key(1)), None);
    assert!(c.io_errors() >= 1);
}

#[test]
fn hostile_keys_cannot_escape_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let c = ThumbCache::new(dir.path().join("c"), 1 << 20);
    c.put("../../escape", b"x").unwrap();
    c.put("", b"x").unwrap();
    assert_eq!(c.get("../../escape"), None);
    assert!(!dir.path().join("escape.jpg").exists());
}
