//! Natural ordering for file and folder names. Package A.

use std::cmp::Ordering;

/// "img2" < "img10"; case-insensitive first, then exact as a tiebreak so the
/// order is total and stable across platforms.
///
/// Case folding goes through `to_lowercase` (not ASCII-only) so `É`/`é` tie
/// on the first pass too. The crate may call different spellings Equal
/// (`img007` vs `img7`, or case variants), which is why the exact `str::cmp`
/// tiebreak is required: without it `a.jpg` and `A.jpg` would be Equal and a
/// listing's order would depend on the order the OS returned them in.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    alphanumeric_sort::compare_str(a.to_lowercase(), b.to_lowercase()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = names.iter().map(|s| (*s).to_owned()).collect();
        v.sort_by(|a, b| natural_cmp(a, b));
        v
    }

    fn pos(v: &[String], name: &str) -> usize {
        v.iter().position(|s| s == name).unwrap()
    }

    #[test]
    fn numbers_compare_by_value() {
        assert_eq!(
            sorted(&["img10.jpg", "img2.jpg", "img1.jpg"]),
            ["img1.jpg", "img2.jpg", "img10.jpg"]
        );
    }

    #[test]
    fn case_is_folded_first_then_exact_tiebreak() {
        assert_eq!(
            sorted(&["b.jpg", "A.jpg", "a.jpg"]),
            ["A.jpg", "a.jpg", "b.jpg"]
        );
        assert_eq!(natural_cmp("a.jpg", "A.jpg"), Ordering::Greater);
        assert_eq!(natural_cmp("A.jpg", "a.jpg"), Ordering::Less);
        assert_eq!(natural_cmp("a.jpg", "a.jpg"), Ordering::Equal);
    }

    #[test]
    fn unicode_names_are_ordered_and_do_not_panic() {
        let v = sorted(&[
            "照片10.jpg",
            "照片2.jpg",
            "中文.jpg",
            "Éa.jpg",
            "éa.jpg",
            "z.jpg",
        ]);
        assert!(pos(&v, "照片2.jpg") < pos(&v, "照片10.jpg"));
        // É and é fold together: they sit next to each other (upper first),
        // not at opposite ends of the list. Accents are not stripped, so both
        // sort after plain ASCII "z" by code point.
        assert_eq!(pos(&v, "éa.jpg"), pos(&v, "Éa.jpg") + 1);
        assert!(pos(&v, "z.jpg") < pos(&v, "Éa.jpg"));
    }

    #[test]
    fn leading_zeros_are_stable_and_distinct() {
        // Whatever the crate decides about 007 vs 7, the result must be a
        // strict, antisymmetric order (never Equal for different spellings).
        let o = natural_cmp("img007.jpg", "img7.jpg");
        assert_ne!(o, Ordering::Equal);
        assert_eq!(natural_cmp("img7.jpg", "img007.jpg"), o.reverse());
        assert_eq!(
            sorted(&["img10.jpg", "img007.jpg", "img7.jpg", "img8.jpg"]),
            sorted(&["img7.jpg", "img8.jpg", "img10.jpg", "img007.jpg"])
        );
    }

    /// Deterministic shuffle (xorshift) so the test needs no rand crate.
    fn shuffle(v: &mut [String], mut seed: u64) {
        for i in (1..v.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            v.swap(i, (seed % (i as u64 + 1)) as usize);
        }
    }

    #[test]
    fn sorting_any_permutation_gives_the_same_result() {
        let base: Vec<String> = [
            "img1.jpg",
            "IMG1.jpg",
            "img01.jpg",
            "img001.jpg",
            "img2.jpg",
            "img10.jpg",
            "a.jpg",
            "A.jpg",
            "中文.jpg",
            "照片2.jpg",
            "照片10.jpg",
            "1a",
            "01",
            "1-1",
            "1",
            "x_1",
            "x1",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let mut reference = base.clone();
        reference.sort_by(|a, b| natural_cmp(a, b));
        for seed in 1..50u64 {
            let mut v = base.clone();
            shuffle(&mut v, seed * 0x9E37_79B9);
            v.sort_by(|a, b| natural_cmp(a, b));
            assert_eq!(v, reference, "seed {seed}");
            // Sorting an already sorted list again changes nothing.
            let mut again = v.clone();
            again.sort_by(|a, b| natural_cmp(a, b));
            assert_eq!(again, v);
        }
    }

    #[test]
    fn order_is_total_and_transitive_on_tricky_names() {
        let names = [
            "a", "A", "1", "01", "1a", "01a", "1-1", "a1", "a01", "a1b", "a01b", "_", "", "é", "É",
            "z9", "z10", "z009",
        ];
        for a in names {
            for b in names {
                let ab = natural_cmp(a, b);
                assert_eq!(ab, natural_cmp(b, a).reverse(), "{a:?} {b:?}");
                assert_eq!(ab == Ordering::Equal, a == b, "{a:?} {b:?}");
                for c in names {
                    if ab != Ordering::Greater && natural_cmp(b, c) != Ordering::Greater {
                        assert_ne!(natural_cmp(a, c), Ordering::Greater, "{a:?} {b:?} {c:?}");
                    }
                }
            }
        }
    }
}
