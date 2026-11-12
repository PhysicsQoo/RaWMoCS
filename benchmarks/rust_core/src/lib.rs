//! Deterministic synthetic Rust performance fixtures, not QuPRS end-to-end CNFs.
pub mod scalability;
use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

#[derive(Clone, Copy, Debug)]
pub enum Workload {
    ForcedFree,
    Connected,
    Disjoint,
    Cache,
}

fn ratio(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

pub fn fixture(kind: Workload) -> Problem {
    let (n, clauses) = match kind {
        Workload::ForcedFree => (12, vec![vec![1], vec![-2], vec![3]]),
        Workload::Connected => (
            6,
            vec![
                vec![1, 2],
                vec![-1, 3],
                vec![-2, 4],
                vec![-3, 5],
                vec![-4, -5, 6],
                vec![2, -6],
                vec![-1, -4, 5],
                vec![1, -3, -6],
            ],
        ),
        Workload::Disjoint => (
            10,
            vec![
                vec![1, 2],
                vec![-1, 2],
                vec![3, 4],
                vec![-3, 4],
                vec![5, 6],
                vec![-5, 6],
                vec![7, 8],
                vec![-7, 8],
            ],
        ),
        Workload::Cache => (3, vec![vec![1, 2], vec![-1, 2], vec![1, 3], vec![-1, 3]]),
    };
    let weights = (0..n)
        .map(|i| {
            let k = i as i64 + 1;
            Weights::new(
                Gaussian::new(ratio(2 * k + 1, k + 2), ratio(k % 3 - 1, k + 3)),
                Gaussian::new(ratio(1 - k, k + 4), ratio(k + 1, 2 * k + 3)),
            )
        })
        .collect();
    Problem::new(n, clauses, weights).expect("fixed valid CNF")
}

fn bigint(s: &str) -> BigInt {
    BigInt::parse_bytes(s.as_bytes(), 10).expect("fixed integer")
}

pub fn arithmetic_operands() -> (Gaussian, Gaussian) {
    let a = Gaussian::new(
        BigRational::new(
            bigint("123456789123456789123456789123456789"),
            bigint("111111111111111111111111111111111117"),
        ),
        BigRational::new(
            bigint("99999999999999999999999999999999989"),
            bigint("33333333333333333333333333333333331"),
        ),
    );
    let b = Gaussian::new(
        BigRational::new(
            bigint("-87654321987654321987654321987654321"),
            bigint("77777777777777777777777777777777773"),
        ),
        BigRational::new(
            bigint("9876543219876543219876543219876543"),
            bigint("55555555555555555555555555555555559"),
        ),
    );
    (a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{solve, solve_with_limits, Limits};

    #[test]
    fn inputs_finish_and_cache_does_not_change_counts() {
        for kind in [
            Workload::ForcedFree,
            Workload::Connected,
            Workload::Disjoint,
            Workload::Cache,
        ] {
            let p = fixture(kind);
            let on = solve(&p).expect("bounded solver completed");
            let off = solve_with_limits(
                &p,
                Limits {
                    cache_entries: 0,
                    ..Limits::default()
                },
            )
            .expect("bounded solver completed");
            assert_eq!(on.satisfiable, off.satisfiable, "{kind:?}");
            assert_eq!(on.count, off.count, "{kind:?}");
            assert_eq!(off.stats.cache_entries, 0);
        }
    }

    #[test]
    fn named_cases_really_exercise_decomposition_and_cache_hits() {
        let split = solve(&fixture(Workload::Disjoint)).unwrap();
        assert!(split.stats.decompositions > 0, "must decompose");
        let reused = solve(&fixture(Workload::Cache)).unwrap();
        assert!(reused.stats.cache_hits > 0, "must reuse cached residual");
    }

    #[test]
    fn big_rational_operands_multiply_exactly() {
        let (a, b) = arithmetic_operands();
        assert_eq!(a.mul(&b), b.mul(&a));
        assert_ne!(a.mul(&b), Gaussian::zero());
    }
}
