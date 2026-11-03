//! Natural ordering for file and folder names. Package A.

use std::cmp::Ordering;

/// "img2" < "img10"; case-insensitive first, then exact as a tiebreak so the
/// order is total and stable across platforms.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    a.cmp(b) // package A: replace with alphanumeric-sort based ordering
}
