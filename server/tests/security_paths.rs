//! Path policy: every way a logical path could escape its root.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

use photosee_core::ErrorCode;
use photosee_server::paths::{Policy, Root};
use std::path::{Path, PathBuf};

fn rooted(dir: &Path) -> Policy {
    Policy::Roots(vec![Root {
        name: "photos".into(),
        dir: dir.to_path_buf(),
    }])
}

fn code(p: &Policy, logical: &str) -> ErrorCode {
    p.resolve(logical).expect_err(logical).code
}

#[test]
fn rooted_resolves_inside_and_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("2024")).unwrap();
    let p = rooted(tmp.path());
    let phys = p.resolve("/photos/2024/a.jpg").unwrap();
    assert_eq!(phys, tmp.path().join("2024").join("a.jpg"));
    assert_eq!(p.to_logical(&phys).unwrap(), "/photos/2024/a.jpg");
    assert_eq!(p.resolve("/photos").unwrap(), tmp.path());
    assert_eq!(p.resolve("/photos/").unwrap(), tmp.path());
    assert_eq!(p.to_logical(tmp.path()).unwrap(), "/photos");
}

#[test]
fn rooted_rejects_traversal_and_odd_segments() {
    let tmp = tempfile::tempdir().unwrap();
    let p = rooted(tmp.path());
    for bad in [
        "/photos/../x",
        "/photos/a/../../x",
        "/photos/./a",
        "/photos//a",
        "/photos/a\\..\\..\\x",
        "/photos/C:/Windows",
        "/photos/a.jpg:stream",
        "photos/a.jpg",
        "/photos/a\0.jpg",
    ] {
        assert_eq!(code(&p, bad), ErrorCode::Forbidden, "{bad:?}");
    }
    assert_eq!(code(&p, "/other/a.jpg"), ErrorCode::NotFound);
    assert_eq!(code(&p, "/"), ErrorCode::NotFound);
}

#[cfg(windows)]
#[test]
fn rooted_rejects_dos_devices_on_windows() {
    let tmp = tempfile::tempdir().unwrap();
    let p = rooted(tmp.path());
    for bad in [
        "/photos/CON",
        "/photos/nul.jpg",
        "/photos/com1.png",
        "/photos/LPT9",
    ] {
        assert_eq!(code(&p, bad), ErrorCode::Forbidden, "{bad:?}");
    }
    assert!(p.resolve("/photos/console.jpg").is_ok());
    assert!(p.resolve("/photos/COM10.jpg").is_ok());
}

/// A link inside the root that points outside it must not be followed.
/// Symlinks need privileges on Windows; directory junctions do not, and
/// canonicalize resolves both.
fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|o| o.status.success())
    }
}

#[test]
fn rooted_rejects_link_escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.jpg"), b"x").unwrap();
    let link = root.path().join("escape");
    if !link_dir(outside.path(), &link) {
        eprintln!("skipped: could not create a directory link");
        return;
    }
    let p = rooted(root.path());
    assert_eq!(code(&p, "/photos/escape/secret.jpg"), ErrorCode::Forbidden);
    assert_eq!(code(&p, "/photos/escape/missing.jpg"), ErrorCode::Forbidden);
}

#[test]
fn nested_roots_map_to_the_inner_one() {
    let tmp = tempfile::tempdir().unwrap();
    let inner = tmp.path().join("inner");
    let p = Policy::Roots(vec![
        Root {
            name: "all".into(),
            dir: tmp.path().to_path_buf(),
        },
        Root {
            name: "in".into(),
            dir: inner.clone(),
        },
    ]);
    assert_eq!(p.to_logical(&inner.join("a.jpg")).unwrap(), "/in/a.jpg");
    let outside = PathBuf::from(if cfg!(windows) { r"C:\x.jpg" } else { "/x.jpg" });
    assert_eq!(
        p.to_logical(&outside).unwrap_err().code,
        ErrorCode::Forbidden
    );
}

#[test]
fn any_local_requires_absolute_and_no_parent() {
    let p = Policy::AnyLocal;
    let abs = if cfg!(windows) {
        r"C:\Photos\a.jpg"
    } else {
        "/photos/a.jpg"
    };
    assert_eq!(p.resolve(abs).unwrap(), PathBuf::from(abs));
    assert_eq!(p.to_logical(Path::new(abs)).unwrap(), abs);
    assert_eq!(code(&p, "relative/a.jpg"), ErrorCode::Forbidden);
    let up = if cfg!(windows) {
        r"C:\Photos\..\a.jpg"
    } else {
        "/photos/../a.jpg"
    };
    assert_eq!(code(&p, up), ErrorCode::Forbidden);
    #[cfg(windows)]
    assert!(p.resolve(r"\\nas\share\b.jpg").is_ok());
}
