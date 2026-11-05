//! Turns a flat member list into directories the browser can walk.
//!
//! Two facts from the user's library shape this (2026-11-05, 32 .cbz on 13):
//! - 23 of 32 archives wrap every page in one top-level folder named after
//!   the volume. Showing that folder would make every volume a two-click
//!   affair, so a lone wrapper folder is collapsed into the archive root.
//! - Explicit directory members (`name/`) exist but are optional; folders
//!   are derived from file paths instead.

use super::zip::Member;
use std::collections::{BTreeMap, BTreeSet};

/// Path inside the archive, '/'-joined, no leading or trailing '/'.
/// The empty string is the archive root.
pub type InnerPath = String;

#[derive(Debug, Default)]
pub struct Tree {
    /// Files by their (collapsed) inner path, pointing into `members`.
    pub files: BTreeMap<InnerPath, usize>,
    /// Every folder, the root ("") included.
    pub dirs: BTreeSet<InnerPath>,
    pub members: Vec<Member>,
}

/// Splits a member name into clean segments, or `None` if it should not be
/// shown. Names come from the archive, so they can contain `..`, absolute
/// paths, backslashes or drive letters. Nothing is ever written to disk, but
/// such a name would not survive the round trip through a native `Path`
/// (`\` and `:` are separators or prefixes on Windows), so it is dropped.
fn segments(name: &str) -> Option<Vec<&str>> {
    let segs: Vec<&str> = name
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    let bad = |s: &&str| *s == ".." || s.contains(':') || s.eq_ignore_ascii_case("__MACOSX");
    if segs.is_empty() || segs.iter().any(bad) {
        return None;
    }
    Some(segs)
}

/// Number of leading folders that every file shares and that hold nothing
/// else: `Vol 1/001.jpg, Vol 1/002.jpg` → 1. Stops at the first level that
/// has a file or more than one folder.
fn wrapper_depth(paths: &[Vec<&str>]) -> usize {
    let mut depth = 0;
    loop {
        let mut first: Option<&str> = None;
        for p in paths {
            // A file at this level (or the lone folder name being a file)
            // means there is no wrapper to strip.
            if p.len() <= depth + 1 {
                return depth;
            }
            match first {
                None => first = Some(p[depth]),
                Some(f) if f == p[depth] => {}
                Some(_) => return depth,
            }
        }
        if first.is_none() {
            return depth;
        }
        depth += 1;
    }
}

pub fn build(members: Vec<Member>) -> Tree {
    // Directory members carry no data; folders come from file paths.
    let kept: Vec<(usize, Vec<&str>)> = members
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.name.ends_with('/') && !m.name.ends_with('\\'))
        .filter_map(|(i, m)| segments(&m.name).map(|s| (i, s)))
        .collect();
    let paths: Vec<Vec<&str>> = kept.iter().map(|(_, s)| s.clone()).collect();
    let strip = wrapper_depth(&paths);
    let mut tree = Tree::default();
    tree.dirs.insert(String::new());
    for (i, segs) in &kept {
        let segs = &segs[strip..];
        for n in 1..segs.len() {
            tree.dirs.insert(segs[..n].join("/"));
        }
        // First member wins when two names collapse to the same path.
        tree.files.entry(segs.join("/")).or_insert(*i);
    }
    tree.members = members;
    tree
}

/// The last segment and the parent of an inner path.
pub fn split_last(p: &str) -> (&str, &str) {
    p.rsplit_once('/')
        .map_or(("", p), |(parent, name)| (parent, name))
}

impl Tree {
    /// Direct children of `dir`: (name, file member or `None` for a folder).
    pub fn children(&self, dir: &str) -> Vec<(&str, Option<&Member>)> {
        let sub = self
            .dirs
            .iter()
            .filter(|d| !d.is_empty() && split_last(d).0 == dir)
            .map(|d| (split_last(d).1, None));
        let files = self
            .files
            .iter()
            .filter(|(f, _)| split_last(f).0 == dir)
            .map(|(f, &i)| (split_last(f).1, Some(&self.members[i])));
        sub.chain(files).collect()
    }

    pub fn file(&self, path: &str) -> Option<&Member> {
        self.files.get(path).map(|&i| &self.members[i])
    }
}
