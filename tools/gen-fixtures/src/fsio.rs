//! Output entries, writing them to disk, and comparing two trees.

use crate::codec::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One generated item. Paths are relative with `/` separators.
pub enum Entry {
    File(String, Vec<u8>),
    Dir(String),
}

pub fn write_all(root: &Path, entries: &[Entry]) -> Result<()> {
    for e in entries {
        match e {
            Entry::File(rel, data) => {
                let p = root.join(rel);
                if let Some(parent) = p.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(p, data)?;
            }
            Entry::Dir(rel) => fs::create_dir_all(root.join(rel))?,
        }
    }
    Ok(())
}

/// Relative path -> file bytes (`None` for a directory).
type Listing = BTreeMap<String, Option<Vec<u8>>>;

fn list_into(root: &Path, dir: &Path, out: &mut Listing) -> Result<()> {
    for item in fs::read_dir(dir)? {
        let path: PathBuf = item?.path();
        let rel = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            out.insert(rel, None);
            list_into(root, &path, out)?;
        } else {
            out.insert(rel, Some(fs::read(&path)?));
        }
    }
    Ok(())
}

fn listing(root: &Path) -> Result<Listing> {
    let mut out = Listing::new();
    if root.is_dir() {
        list_into(root, root, &mut out)?;
    }
    // .gitkeep only keeps the directory in git; it is not a fixture.
    out.remove(".gitkeep");
    Ok(out)
}

/// Human-readable differences between `expected` (fresh) and `actual` (on disk).
pub fn diff_trees(expected: &Path, actual: &Path) -> Result<Vec<String>> {
    let (want, have) = (listing(expected)?, listing(actual)?);
    let mut problems = Vec::new();
    for (rel, data) in &want {
        match have.get(rel) {
            None => problems.push(format!("missing: {rel}")),
            Some(d) if d != data => problems.push(format!("differs: {rel}")),
            Some(_) => {}
        }
    }
    for rel in have.keys().filter(|r| !want.contains_key(*r)) {
        problems.push(format!("unexpected: {rel}"));
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("gen-fixtures-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn identical_trees_have_no_diff_and_changes_are_reported() -> Result<()> {
        let entries = vec![
            Entry::File("a/x.bin".into(), vec![1, 2]),
            Entry::Dir("empty".into()),
        ];
        let (a, b) = (scratch("a"), scratch("b"));
        write_all(&a, &entries)?;
        write_all(&b, &entries)?;
        assert!(diff_trees(&a, &b)?.is_empty());
        fs::write(b.join("a/x.bin"), [9])?;
        fs::write(b.join("extra"), [0])?;
        let d = diff_trees(&a, &b)?;
        assert_eq!(
            d,
            vec![
                "differs: a/x.bin".to_string(),
                "unexpected: extra".to_string()
            ]
        );
        fs::remove_dir_all(a)?;
        fs::remove_dir_all(b)?;
        Ok(())
    }
}
