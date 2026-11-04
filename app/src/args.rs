//! Command line: one optional path, a file or a folder.

use std::path::{Path, PathBuf};

/// The logical path (`Policy::AnyLocal` speaks native absolute paths) of the
/// first non-flag argument. `args` excludes argv[0]. A relative path means
/// relative to `cwd`: for a second launch that is the *second* process's
/// directory, which the single-instance callback hands over.
pub fn start_from(args: &[String], cwd: &Path) -> Option<String> {
    let arg = args
        .iter()
        .find(|a| !a.starts_with("--") && !a.is_empty())?;
    let p = PathBuf::from(arg);
    let joined = if p.is_absolute() { p } else { cwd.join(p) };
    // `absolute` (GetFullPathNameW) folds `.`, `..` and `/`; the server
    // refuses `..` outright because it would make two spellings of one file.
    std::path::absolute(joined)
        .ok()?
        .to_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn no_argument_means_no_start() {
        assert_eq!(start_from(&[], Path::new(r"C:\w")), None);
        assert_eq!(start_from(&a(&["--flag"]), Path::new(r"C:\w")), None);
    }

    #[cfg(windows)]
    #[test]
    fn absolute_path_is_kept() {
        let got = start_from(&a(&[r"C:\photos\a.jpg"]), Path::new(r"D:\w"));
        assert_eq!(got.as_deref(), Some(r"C:\photos\a.jpg"));
    }

    #[cfg(windows)]
    #[test]
    fn relative_path_resolves_against_the_given_cwd_and_loses_dotdot() {
        let got = start_from(&a(&[r"..\x\.\b.jpg"]), Path::new(r"D:\w\sub"));
        assert_eq!(got.as_deref(), Some(r"D:\w\x\b.jpg"));
        let got = start_from(&a(&["C:/photos/a.jpg"]), Path::new(r"D:\w"));
        assert_eq!(got.as_deref(), Some(r"C:\photos\a.jpg"));
    }
}
