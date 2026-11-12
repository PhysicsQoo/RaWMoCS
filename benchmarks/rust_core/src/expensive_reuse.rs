//! Research-only connected CNF with an expensive identical residual.
//!
//! Every edge of an all-positive cycle is guarded by both phases of one
//! selector. Whichever selector phase DPLL chooses, it sees the *same*
//! non-unit, connected cycle. Unlike the three-variable control fixture,
//! the repeated residual requires a nontrivial weighted model count.
//! This is exact Full Complex WMC, not projected WMC or a QuPRS workload.

use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

fn ratio(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn selector_weights() -> Weights {
    Weights::new(
        Gaussian::new(ratio(1, 2), ratio(1, 3)),
        Gaussian::new(ratio(3, 5), ratio(-1, 7)),
    )
}

fn cycle_weights() -> Weights {
    Weights::new(Gaussian::integers(2, 1), Gaussian::integers(1, -1))
}

/// Clause family (x | y_i | y_(i+1)) & (!x | y_i | y_(i+1))
/// for every edge of a cycle on y_0..y_(n-1).
///
/// Logically equivalent to a positive-literal cycle CNF with x free.
/// x has 2n literal occurrences, versus four per y_i, so the current
/// frequency heuristic selects x before entering the shared residual.
pub fn guarded_cycle_residual(vertices: usize) -> Problem {
    assert!(
        (4..=18).contains(&vertices),
        "bounded connected research family: 4..=18 vertices"
    );
    let mut clauses = Vec::with_capacity(2 * vertices);
    for i in 0..vertices {
        let current = (i + 2) as i32;
        let next = ((i + 1) % vertices + 2) as i32;
        clauses.push(vec![1, current, next]);
        clauses.push(vec![-1, current, next]);
    }
    let mut weights = vec![selector_weights()];
    weights.extend((0..vertices).map(|_| cycle_weights()));
    Problem::new(vertices + 1, clauses, weights).expect("fixed valid guarded cycle CNF")
}

/// Independent transfer-DP oracle, not DPLL, component decomposition or cache.
///
/// States are the last y assignment. A positive cycle edge excludes only
/// (false, false); conditioning on the first y closes the final edge.
pub fn independent_cycle_wmc(vertices: usize) -> Gaussian {
    assert!((4..=18).contains(&vertices));
    let pair = cycle_weights();
    let mut total = Gaussian::zero();
    for first in [false, true] {
        let mut last_false = if first {
            Gaussian::zero()
        } else {
            pair.negative.clone()
        };
        let mut last_true = if first {
            pair.positive.clone()
        } else {
            Gaussian::zero()
        };
        for _ in 1..vertices {
            let next_false = last_true.mul(&pair.negative);
            let next_true = last_false.add(&last_true).mul(&pair.positive);
            last_false = next_false;
            last_true = next_true;
        }
        let valid = if first {
            last_false.add(&last_true)
        } else {
            last_true
        };
        total = total.add(&valid);
    }
    let selector = selector_weights();
    total.mul(&selector.positive.add(&selector.negative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{solve_with_limits, Limits};

    // Exhaustive Boolean assignment oracle is independent of transfer-DP.
    fn exhaustive(problem: &Problem) -> (bool, Gaussian) {
        let variables = problem.variables();
        assert!(variables <= 9);
        let mut sum = Gaussian::zero();
        let mut sat = false;
        for mask in 0..(1usize << variables) {
            let holds = problem.clauses().iter().all(|clause| {
                clause.iter().any(|&literal| {
                    let is_true = mask & (1usize << (literal.unsigned_abs() as usize - 1)) != 0;
                    if literal > 0 { is_true } else { !is_true }
                })
            });
            if !holds {
                continue;
            }
            sat = true;
            let mut product = Gaussian::one();
            for (index, weights) in problem.weights().iter().enumerate() {
                product = product.mul(if mask & (1usize << index) != 0 {
                    &weights.positive
                } else {
                    &weights.negative
                });
            }
            sum = sum.add(&product);
        }
        (sat, sum)
    }

    #[test]
    fn transfer_oracle_agrees_with_exhaustive_assignments() {
        for vertices in [4, 6, 8] {
            let problem = guarded_cycle_residual(vertices);
            let (sat, count) = exhaustive(&problem);
            assert!(sat);
            assert_eq!(count, independent_cycle_wmc(vertices));
        }
    }

    #[test]
    fn shared_nontrivial_residual_reuses_cache_without_changing_exact_wmc() {
        for vertices in [6, 10, 14] {
            let problem = guarded_cycle_residual(vertices);
            let expected = independent_cycle_wmc(vertices);
            let cached = solve_with_limits(&problem, Limits::default())
                .expect("bounded connected cycle should solve with cache");
            let uncached = solve_with_limits(
                &problem,
                Limits {
                    cache_entries: 0,
                    ..Limits::default()
                },
            )
            .expect("bounded connected cycle should solve without cache");
            assert!(cached.satisfiable && uncached.satisfiable);
            assert_eq!(cached.count, expected, "cache on; vertices {vertices}");
            assert_eq!(uncached.count, expected, "cache off; vertices {vertices}");
            assert!(
                cached.stats.cache_hits > 0,
                "nontrivial cycle must have actual repeated residual hits"
            );
            assert_eq!(uncached.stats.cache_hits, 0);
            assert_eq!(uncached.stats.cache_entries, 0);
            assert!(
                uncached.stats.nodes > cached.stats.nodes,
                "cache should avoid some repeated residual search nodes"
            );
        }
    }
}
