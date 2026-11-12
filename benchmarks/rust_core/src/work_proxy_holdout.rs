//! Selector-free, source-pinned irregular CNF holdout for observed-work
//! cache admission. The oracle enumerates assignments independently of DPLL.

use rawmocs_core::{Gaussian, Problem, Weights};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holdout {
    Ring,
    Chorded,
    Mixed,
}

impl Holdout {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ring => "ring",
            Self::Chorded => "chorded",
            Self::Mixed => "mixed",
        }
    }

    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "ring" => Some(Self::Ring),
            "chorded" => Some(Self::Chorded),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

fn clauses(kind: Holdout, n: usize) -> Vec<Vec<i32>> {
    assert!([8, 10, 12].contains(&n), "fixed selector-free holdout sizes");
    let v = |index: usize| (index % n + 1) as i32;
    let mut clauses: Vec<Vec<i32>> = (0..n).map(|i| vec![v(i), v(i + 1)]).collect();
    if kind != Holdout::Ring {
        // Connected cycle plus distant chords: no artificial selector.
        for i in 0..n / 2 {
            clauses.push(vec![v(i), v(i + n / 2)]);
        }
    }
    if kind == Holdout::Mixed {
        // A distinct mixed-sign 3-CNF family with nonuniform clause length.
        for i in (0..n).step_by(2) {
            clauses.push(vec![v(i), -v(i + 1), v(i + 3)]);
        }
    }
    clauses
}

fn positive() -> Gaussian {
    Gaussian::integers(2, 1)
}

fn negative() -> Gaussian {
    Gaussian::integers(1, -1)
}

pub fn selector_free_cnf(kind: Holdout, n: usize) -> Problem {
    let weights = (0..n)
        .map(|_| Weights::new(positive(), negative()))
        .collect();
    Problem::new(n, clauses(kind, n), weights).expect("predeclared connected CNF")
}

/// Independent exhaustive truth-assignment oracle, not the production
/// solver's reduction, search, component splitting or memo table.
/// Group models by the number of negative-weight (false) assignments,
/// then exactly sum the Gaussian integer weighted cardinality polynomial.
pub fn independent_wmc(kind: Holdout, n: usize) -> (bool, Gaussian) {
    let formula = clauses(kind, n);
    let mut counts = vec![0i64; n + 1];
    let mut satisfiable = false;
    for assignment in 0usize..(1usize << n) {
        let holds = formula.iter().all(|clause| {
            clause.iter().any(|&literal| {
                let true_bit = (assignment & (1usize << (literal.unsigned_abs() as usize - 1))) != 0;
                if literal > 0 {
                    true_bit
                } else {
                    !true_bit
                }
            })
        });
        if holds {
            satisfiable = true;
            let false_count = n - assignment.count_ones() as usize;
            counts[false_count] += 1;
        }
    }
    let mut pos_powers = vec![Gaussian::one()];
    let mut neg_powers = vec![Gaussian::one()];
    for i in 0..n {
        pos_powers.push(pos_powers[i].mul(&positive()));
        neg_powers.push(neg_powers[i].mul(&negative()));
    }
    let mut total = Gaussian::zero();
    for (false_count, models) in counts.into_iter().enumerate() {
        if models > 0 {
            let count = pos_powers[n - false_count]
                .mul(&neg_powers[false_count])
                .mul(&Gaussian::integers(models, 0));
            total = total.add(&count);
        }
    }
    (satisfiable, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{
        solve_with_limits, solve_with_observed_work_ratio, Limits, SolveError,
    };

    #[test]
    fn selector_free_holdouts_match_independent_exact_complex_wmc() {
        for kind in [Holdout::Ring, Holdout::Chorded, Holdout::Mixed] {
            for n in [8, 10, 12] {
                let input = selector_free_cnf(kind, n);
                let (sat, expected) = independent_wmc(kind, n);
                assert!(sat, "source-locked {kind:?} {n} must be SAT");
                let off = solve_with_limits(
                    &input,
                    Limits {
                        cache_entries: 0,
                        ..Limits::default()
                    },
                )
                .unwrap();
                let legacy = solve_with_limits(&input, Limits::default()).unwrap();
                assert_eq!(off.satisfiable, sat);
                assert_eq!(off.count, expected, "off {kind:?} {n}");
                assert_eq!(legacy.count, expected, "legacy {kind:?} {n}");
                for ratio in [1, 4] {
                    let work =
                        solve_with_observed_work_ratio(&input, Limits::default(), ratio).unwrap();
                    assert_eq!(work.satisfiable, sat);
                    assert_eq!(work.count, expected, "{kind:?} {n} ratio {ratio}");
                    assert_eq!(work.stats.cache_payload_rejections, 0);
                }
            }
        }
    }

    #[test]
    fn budget_errors_remain_fail_closed_with_work_proxy() {
        let input = selector_free_cnf(Holdout::Mixed, 10);
        assert_eq!(
            solve_with_observed_work_ratio(
                &input,
                Limits {
                    nodes: 0,
                    ..Limits::default()
                },
                4
            ),
            Err(SolveError::NodeBudgetExceeded)
        );
    }

    #[test]
    fn guarded_positive_control_keeps_reuse_available() {
        let input = crate::admission_holdout::guarded_graph(
            crate::admission_holdout::Graph::Cycle,
            14,
        );
        let expected = crate::admission_holdout::independent_exact_wmc(
            crate::admission_holdout::Graph::Cycle,
            14,
        );
        let legacy = solve_with_limits(&input, Limits::default()).unwrap();
        assert!(legacy.stats.cache_hits > 0);
        for ratio in [1, 4] {
            let got = solve_with_observed_work_ratio(&input, Limits::default(), ratio).unwrap();
            assert_eq!(got.count, expected);
            assert!(got.stats.cache_work_proxy_rejections > 0);
        }
    }
}
