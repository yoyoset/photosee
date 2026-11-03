//! Command-line parsing. Hand-rolled: the tool has one positional and one flag,
//! and zero dependencies is a hard requirement.

use std::path::PathBuf;

pub const DEFAULT_RUNS: usize = 3;
/// Upper bound so a typo like `--runs 100000` cannot hammer the NAS for hours.
pub const MAX_RUNS: usize = 50;

pub const USAGE: &str = "usage: bench-smb <DIR> [--runs N]\n\n\
Read-only benchmark of a photo folder (local, mapped drive or UNC path).\n\
Prints a Markdown report to stdout.\n\n  \
--runs N   repetitions of the list / head / first-image phases (default 3, max 50)\n";

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Run { dir: PathBuf, runs: usize },
    Help,
}

fn parse_runs(s: &str) -> Result<usize, String> {
    match s.parse::<usize>() {
        Ok(n) if (1..=MAX_RUNS).contains(&n) => Ok(n),
        _ => Err(format!(
            "--runs expects an integer in 1..={MAX_RUNS}, got `{s}`"
        )),
    }
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Parsed, String> {
    let mut dir: Option<PathBuf> = None;
    let mut runs = DEFAULT_RUNS;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "-h" || a == "--help" {
            return Ok(Parsed::Help);
        } else if a == "--runs" {
            runs = parse_runs(&it.next().ok_or("--runs needs a value")?)?;
        } else if let Some(v) = a.strip_prefix("--runs=") {
            runs = parse_runs(v)?;
        } else if a.starts_with("--") {
            return Err(format!("unknown option `{a}`"));
        } else if dir.replace(PathBuf::from(&a)).is_some() {
            return Err(format!("unexpected extra argument `{a}`"));
        }
    }
    dir.map(|dir| Parsed::Run { dir, runs })
        .ok_or_else(|| "missing <DIR>".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Result<Parsed, String> {
        parse(a.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_and_runs() {
        assert_eq!(
            p(&["x"]),
            Ok(Parsed::Run {
                dir: "x".into(),
                runs: 3
            })
        );
        assert_eq!(
            p(&["x", "--runs", "5"]),
            Ok(Parsed::Run {
                dir: "x".into(),
                runs: 5
            })
        );
        assert_eq!(
            p(&["--runs=2", "x"]),
            Ok(Parsed::Run {
                dir: "x".into(),
                runs: 2
            })
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(p(&[]).is_err());
        assert!(p(&["x", "--runs", "0"]).is_err());
        assert!(p(&["x", "--runs", "abc"]).is_err());
        assert!(p(&["x", "--runs"]).is_err());
        assert!(p(&["x", "y"]).is_err());
        assert!(p(&["x", "--bogus"]).is_err());
        assert_eq!(p(&["--help"]), Ok(Parsed::Help));
    }
}
