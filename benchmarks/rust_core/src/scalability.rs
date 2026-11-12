//! Deterministic medium-CNF workloads with independent closed-form WMC oracles.
//! This is a bounded synthetic family, NOT a QuPRS end-to-end performance claim.

use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

#[derive(Clone, Copy, Debug)]
pub enum Family {
    EqualityChain,
    IndependentPairs,
}

fn fraction(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn weights(variables: usize) -> Vec<Weights> {
    (1..=variables)
        .map(|v| {
            let i = v as i64;
            Weights::new(
                Gaussian::new(fraction(i % 5 + 1, 3), fraction((i + 1) % 3 - 1, 5)),
                Gaussian::new(fraction(i % 3 - 1, 2), fraction(i % 4, 7)),
            )
        })
        .collect()
}

fn check_size(family: Family, variables: usize) {
    assert!(
        (2..=64).contains(&variables),
        "bounded study: 2 to 64 variables"
    );
    if matches!(family, Family::IndependentPairs) {
        assert_eq!(variables % 2, 0, "independent pairs require an even width");
    }
}

/// EqualityChain: each neighbor is equivalent; precisely two SAT assignments.
/// IndependentPairs: each (odd,even) pair forces the even variable true, and
/// leaves its odd partner free. A product of independent two-variable factors.
pub fn fixture(family: Family, variables: usize) -> Problem {
    check_size(family, variables);
    let mut clauses = Vec::new();
    match family {
        Family::EqualityChain => {
            for v in 1..variables as i32 {
                clauses.push(vec![-v, v + 1]);
                clauses.push(vec![v, -(v + 1)]);
            }
        }
        Family::IndependentPairs => {
            for v in (1..variables as i32).step_by(2) {
                clauses.push(vec![v, v + 1]);
                clauses.push(vec![-v, v + 1]);
            }
        }
    }
    Problem::new(variables, clauses, weights(variables)).expect("fixed valid medium CNF")
}

/// Closed-form count derived from Boolean constraints, not DPLL enumeration.
/// It deliberately retains exact Gaussian-rational arithmetic.
pub fn expected(family: Family, variables: usize) -> Gaussian {
    check_size(family, variables);
    let w = weights(variables);
    match family {
        Family::EqualityChain => {
            let positives = w
                .iter()
                .fold(Gaussian::one(), |acc, pair| acc.mul(&pair.positive));
            let negatives = w
                .iter()
                .fold(Gaussian::one(), |acc, pair| acc.mul(&pair.negative));
            positives.add(&negatives)
        }
        Family::IndependentPairs => w.chunks(2).fold(Gaussian::one(), |acc, pair| {
            let free = pair[0].positive.add(&pair[0].negative);
            acc.mul(&free.mul(&pair[1].positive))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{solve, solve_with_limits, Limits, SolveError};

    #[test]
    fn medium_exact_full_wmc_matches_independent_closed_form() {
        for family in [Family::EqualityChain, Family::IndependentPairs] {
            for variables in [8, 16, 32, 64] {
                let problem = fixture(family, variables);
                let report = solve(&problem).expect("bounded medium fixture must solve");
                assert!(report.satisfiable, "fixed formulas are SAT: {family:?}");
                assert_eq!(
                    report.count,
                    expected(family, variables),
                    "{family:?} with {variables} variables"
                );
                println!(
                    "medium-full-wmc {family:?} variables={variables} nodes={} cache_hits={} decompositions={} cache_entries={} key_scope={} key_clauses={} key_literals={} value_bits={} max_value_bits={}",
                    report.stats.nodes,
                    report.stats.cache_hits,
                    report.stats.decompositions,
                    report.stats.cache_entries,
                    report.stats.cache_scope_items,
                    report.stats.cache_clause_items,
                    report.stats.cache_literal_items,
                    report.stats.cache_value_bits,
                    report.stats.max_cached_value_bits
                );
            }
        }
    }

    #[test]
    fn medium_cache_cap_and_disconnected_component_contracts() {
        for variables in [16, 32, 64] {
            let problem = fixture(Family::IndependentPairs, variables);
            let capped = solve_with_limits(
                &problem,
                Limits {
                    cache_entries: 8,
                    ..Limits::default()
                },
            )
            .unwrap();
            let no_cache = solve_with_limits(
                &problem,
                Limits {
                    cache_entries: 0,
                    ..Limits::default()
                },
            )
            .unwrap();
            assert_eq!(capped.count, expected(Family::IndependentPairs, variables));
            assert_eq!(capped.count, no_cache.count);
            assert_eq!(capped.satisfiable, no_cache.satisfiable);
            assert_eq!(no_cache.stats.cache_entries, 0);
            assert!(capped.stats.cache_entries <= 8);
            assert!(capped.stats.decompositions > 0);
        }
    }

    #[test]
    fn medium_resource_exhaustion_never_returns_a_partial_count() {
        let problem = fixture(Family::IndependentPairs, 64);
        assert_eq!(
            solve_with_limits(
                &problem,
                Limits {
                    nodes: 1,
                    ..Limits::default()
                },
            ),
            Err(SolveError::NodeBudgetExceeded)
        );
        assert_eq!(
            solve_with_limits(
                &problem,
                Limits {
                    depth: 1,
                    ..Limits::default()
                },
            ),
            Err(SolveError::DepthBudgetExceeded)
        );
        assert_eq!(
            solve_with_limits(
                &problem,
                Limits {
                    cache_entries: 0,
                    ..Limits::default()
                },
            )
            .unwrap()
            .count,
            expected(Family::IndependentPairs, 64)
        );
    }
}
