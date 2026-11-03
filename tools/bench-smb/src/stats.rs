//! Pure statistics helpers, kept free of IO so they can be unit-tested.

/// Sorted copy; `total_cmp` keeps NaN from panicking or poisoning the order.
fn sorted(v: &[f64]) -> Vec<f64> {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s
}

/// Median; for an even count, the mean of the two middle values.
pub fn median(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let s = sorted(v);
    let mid = s.len() / 2;
    if s.len() % 2 == 1 {
        Some(s[mid])
    } else {
        Some((s[mid - 1] + s[mid]) / 2.0)
    }
}

/// 90th percentile by nearest rank (the smallest value with >=90% of the
/// samples at or below it). Nearest rank always returns a real sample, which
/// is easier to reason about than interpolation for 50-file runs.
pub fn p90(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let s = sorted(v);
    let rank = (0.9 * s.len() as f64).ceil() as usize;
    Some(s[rank.clamp(1, s.len()) - 1])
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    pub min: f64,
    pub median: f64,
    pub max: f64,
}

pub fn summarize(v: &[f64]) -> Option<Summary> {
    let s = sorted(v);
    Some(Summary {
        min: *s.first()?,
        median: median(v)?,
        max: *s.last()?,
    })
}

/// Smallest K whose throughput reaches >=90% of the best throughput.
/// `results` is `(k, mb_per_s)`; order does not matter. `None` when there is
/// no data or nothing was read (best <= 0), so the report never marks a K
/// based on a run where every file failed.
pub fn pick_k(results: &[(usize, f64)]) -> Option<usize> {
    let best = results
        .iter()
        .map(|r| r.1)
        .filter(|x| x.is_finite())
        .fold(0.0, f64::max);
    if best <= 0.0 {
        return None;
    }
    results
        .iter()
        .filter(|r| r.1 >= 0.9 * best)
        .map(|r| r.0)
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_odd_even_empty() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[7.0]), Some(7.0));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn p90_odd_even_empty() {
        let ten: Vec<f64> = (1..=10).map(f64::from).collect();
        assert_eq!(p90(&ten), Some(9.0));
        let nine: Vec<f64> = (1..=9).map(f64::from).collect();
        assert_eq!(p90(&nine), Some(9.0));
        let twenty: Vec<f64> = (1..=20).rev().map(f64::from).collect();
        assert_eq!(p90(&twenty), Some(18.0));
        assert_eq!(p90(&[5.0]), Some(5.0));
        assert_eq!(p90(&[]), None);
    }

    #[test]
    fn nan_does_not_panic() {
        assert!(median(&[1.0, f64::NAN, 2.0]).is_some());
        assert!(p90(&[f64::NAN]).is_some());
    }

    #[test]
    fn summary_min_median_max() {
        let s = summarize(&[3.0, 1.0, 2.0]).unwrap();
        assert_eq!((s.min, s.median, s.max), (1.0, 2.0, 3.0));
        assert_eq!(summarize(&[]), None);
    }

    #[test]
    fn pick_k_smallest_within_90_percent() {
        let r = [(1, 40.0), (2, 85.0), (4, 100.0), (8, 98.0), (16, 60.0)];
        assert_eq!(pick_k(&r), Some(4)); // 85 < 0.9 * 100
        let r2 = [(16, 95.0), (1, 91.0), (4, 100.0)];
        assert_eq!(pick_k(&r2), Some(1)); // order-independent, smallest K
    }

    #[test]
    fn pick_k_degenerate() {
        assert_eq!(pick_k(&[]), None);
        assert_eq!(pick_k(&[(1, 0.0), (2, 0.0)]), None);
        assert_eq!(pick_k(&[(8, 12.0)]), Some(8));
    }
}
