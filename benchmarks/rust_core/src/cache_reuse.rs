//! Controlled, disconnected but nontrivial repeated-residual Full WMC family.
//! Each 3-variable gadget has one genuinely reused residual after DPLL.
//! This is not a representative real-world CNF or a projected-WMC fixture.

use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

fn r(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn selector_weights() -> Weights {
    Weights::new(
        Gaussian::new(r(1, 2), r(1, 3)),
        Gaussian::new(r(3, 5), r(-1, 7)),
    )
}

pub fn repeated_residuals(gadgets: usize) -> Problem {
    assert!(
        (1..=16).contains(&gadgets),
        "bounded research family: 1..=16 gadgets"
    );
    let mut clauses = Vec::with_capacity(4 * gadgets);
    let mut weights = Vec::with_capacity(3 * gadgets);
    for group in 0..gadgets {
        let x = (group * 3 + 1) as i32;
        let y = x + 1;
        let z = x + 2;
        // (x|y)&(!x|y)&(x|z)&(!x|z) forces y and z true, leaves x free.
        // The two x branches have *identical* residual unit clauses y,z.
        clauses.push(vec![x, y]);
        clauses.push(vec![-x, y]);
        clauses.push(vec![x, z]);
        clauses.push(vec![-x, z]);
        weights.push(selector_weights());
        weights.push(Weights::new(Gaussian::integers(2, 1), Gaussian::one()));
        weights.push(Weights::new(Gaussian::integers(1, -1), Gaussian::one()));
    }
    Problem::new(3 * gadgets, clauses, weights).expect("fixed valid repeated residual CNF")
}

/// Independent algebraic oracle: per-gadget WMC is
/// (1/2 + i/3 + 3/5 - i/7) (2+i)(1-i)
/// = (11/10 + 4i/21)(3-i) = 733/210 - 37i/70.
/// SAT is true, with exactly 2^gadgets Boolean satisfying assignments.
/// The oracle does not invoke the solver or derive expected counts from CNF.
pub fn independent_exact_wmc(gadgets: usize) -> Gaussian {
    assert!((1..=16).contains(&gadgets));
    let factor = Gaussian::new(r(733, 210), r(-37, 70));
    (0..gadgets).fold(Gaussian::one(), |total, _| total.mul(&factor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{
        solve_with_cache_payload_budget, solve_with_limits, CachePayloadBudget, Limits,
    };

    #[test]
    fn repeated_residuals_have_independent_exact_counts_and_real_cache_hits() {
        for gadgets in [1, 2, 4, 8, 16] {
            let input = repeated_residuals(gadgets);
            let oracle = independent_exact_wmc(gadgets);
            let full = solve_with_limits(&input, Limits::default()).unwrap();
            let no_cache = solve_with_limits(
                &input,
                Limits {
                    cache_entries: 0,
                    ..Limits::default()
                },
            )
            .unwrap();
            assert!(full.satisfiable);
            assert!(no_cache.satisfiable);
            assert_eq!(full.count, oracle, "gadget count {gadgets}");
            assert_eq!(no_cache.count, oracle);
            assert!(full.stats.cache_hits >= gadgets as u64);
            assert_eq!(no_cache.stats.cache_hits, 0);
            assert_eq!(no_cache.stats.cache_entries, 0);
        }
    }

    #[test]
    fn repeated_residuals_have_controlled_quota_tradeoffs() {
        for gadgets in [4, 8, 16] {
            let input = repeated_residuals(gadgets);
            let oracle = independent_exact_wmc(gadgets);
            let tight = solve_with_cache_payload_budget(
                &input,
                Limits::default(),
                CachePayloadBudget {
                    scope_items: 2,
                    clause_items: 2,
                    literal_items: 2,
                    value_bits: 32,
                },
            )
            .unwrap();
            let roomy = solve_with_cache_payload_budget(
                &input,
                Limits::default(),
                CachePayloadBudget {
                    scope_items: 1_024,
                    clause_items: 1_024,
                    literal_items: 2_048,
                    value_bits: 16_384,
                },
            )
            .unwrap();
            assert!(tight.satisfiable && roomy.satisfiable);
            assert_eq!(tight.count, oracle);
            assert_eq!(roomy.count, oracle);
            assert!(tight.stats.cache_payload_rejections > 0);
            assert_eq!(roomy.stats.cache_payload_rejections, 0);
            assert!(tight.stats.cache_hits >= 1);
            assert!(roomy.stats.cache_hits >= gadgets as u64);
            assert!(tight.stats.cache_scope_items <= 2);
            assert!(tight.stats.cache_clause_items <= 2);
            assert!(tight.stats.cache_literal_items <= 2);
            assert!(tight.stats.cache_value_bits <= 32);
        }
    }
}
