//! HTTP Range support for passthrough responses: one `bytes=a-b`, `bytes=a-`
//! or `bytes=-n` range. Multi-range and malformed headers are ignored (the
//! full body is sent, as RFC 9110 allows); a well-formed range that starts
//! past the end is answered 416. Package G.

/// What a Range header asks for, against a body of known length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeReq {
    /// Malformed or multi-range: act as if the header were absent.
    Ignore,
    /// Well formed but nothing in the body matches: 416.
    Unsatisfiable,
    /// Inclusive byte positions, already clamped to the body.
    Bytes(u64, u64),
}

/// A position: `None` = empty text, `Some(None)` = digits that overflow u64,
/// `Some(Some(n))` = a number. Anything else (signs, letters) is `Err`.
fn position(s: &str) -> Result<Option<Option<u64>>, ()> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    // `u64::from_str` accepts a leading '+', which is not a valid position.
    if !s.bytes().all(|c| c.is_ascii_digit()) {
        return Err(());
    }
    Ok(Some(s.parse::<u64>().ok()))
}

pub fn classify(header: &str, len: u64) -> RangeReq {
    let Some((unit, spec)) = header.trim().split_once('=') else {
        return RangeReq::Ignore;
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") || spec.contains(',') {
        return RangeReq::Ignore;
    }
    let Some((a, b)) = spec.split_once('-') else {
        return RangeReq::Ignore;
    };
    let (Ok(a), Ok(b)) = (position(a), position(b)) else {
        return RangeReq::Ignore;
    };
    match (a, b) {
        (None, Some(n)) => suffix(n, len),
        (Some(start), None) => from(start, None, len),
        (Some(start), Some(end)) => match (start, end) {
            (Some(s), Some(e)) if e < s => RangeReq::Ignore,
            _ => from(start, Some(end), len),
        },
        (None, None) => RangeReq::Ignore,
    }
}

/// `bytes=-n`: the last n bytes. `None` = n beyond u64, i.e. the whole body.
fn suffix(n: Option<u64>, len: u64) -> RangeReq {
    let n = n.unwrap_or(u64::MAX);
    if n == 0 || len == 0 {
        RangeReq::Unsatisfiable
    } else {
        RangeReq::Bytes(len.saturating_sub(n), len - 1)
    }
}

/// `bytes=a-` and `bytes=a-b`. A start beyond u64 is past every body.
fn from(start: Option<u64>, end: Option<Option<u64>>, len: u64) -> RangeReq {
    let Some(start) = start.filter(|s| *s < len) else {
        return RangeReq::Unsatisfiable;
    };
    let last = len - 1;
    // An end beyond u64 is clamped like any end past the body.
    let end = end.flatten().map_or(last, |e| e.min(last));
    RangeReq::Bytes(start, end)
}

/// Parses a Range header against a body of `len` bytes: `Some((first, last))`
/// inclusive, or `None` for anything invalid, multi-range or unsatisfiable.
/// Routes use `classify` when they must tell 200 from 416.
pub fn parse(header: &str, len: u64) -> Option<(u64, u64)> {
    match classify(header, len) {
        RangeReq::Bytes(a, b) => Some((a, b)),
        RangeReq::Ignore | RangeReq::Unsatisfiable => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RangeReq::{Bytes, Ignore, Unsatisfiable};

    #[test]
    fn table() {
        let cases: &[(&str, u64, RangeReq)] = &[
            ("bytes=0-0", 10, Bytes(0, 0)),
            ("bytes=0-4", 10, Bytes(0, 4)),
            ("bytes=2-", 10, Bytes(2, 9)),
            ("bytes=9-", 10, Bytes(9, 9)),
            ("bytes=-3", 10, Bytes(7, 9)),
            ("bytes=-10", 10, Bytes(0, 9)),
            // End and suffix past the body are clamped, not rejected.
            ("bytes=5-999", 10, Bytes(5, 9)),
            ("bytes=-999", 10, Bytes(0, 9)),
            ("bytes=5-99999999999999999999", 10, Bytes(5, 9)),
            ("bytes=-99999999999999999999", 10, Bytes(0, 9)),
            ("  Bytes=1-2 ", 10, Bytes(1, 2)),
            ("bytes=0-0", 1, Bytes(0, 0)),
            // Unsatisfiable: start at or past the end, empty suffix, empty body.
            ("bytes=10-", 10, Unsatisfiable),
            ("bytes=10-12", 10, Unsatisfiable),
            ("bytes=-0", 10, Unsatisfiable),
            ("bytes=0-", 0, Unsatisfiable),
            ("bytes=-5", 0, Unsatisfiable),
            ("bytes=99999999999999999999-", 10, Unsatisfiable),
            // Ignored: malformed, wrong unit, multi-range, reversed.
            ("bytes=5-2", 10, Ignore),
            ("bytes=0-1,3-4", 10, Ignore),
            ("bytes=", 10, Ignore),
            ("bytes=-", 10, Ignore),
            ("bytes=a-b", 10, Ignore),
            ("bytes=+1-2", 10, Ignore),
            ("bytes=1", 10, Ignore),
            ("items=0-1", 10, Ignore),
            ("0-1", 10, Ignore),
            ("", 10, Ignore),
        ];
        for (h, len, want) in cases {
            assert_eq!(classify(h, *len), *want, "{h:?} on {len} bytes");
        }
    }

    #[test]
    fn parse_is_none_unless_satisfiable() {
        assert_eq!(parse("bytes=1-2", 10), Some((1, 2)));
        assert_eq!(parse("bytes=10-", 10), None);
        assert_eq!(parse("bytes=0-1,3-4", 10), None);
    }
}
