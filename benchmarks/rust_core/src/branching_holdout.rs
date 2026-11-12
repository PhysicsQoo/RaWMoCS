//! Fixed research-only irregular branching fixtures and independent exact
//! complex Full WMC oracles. These synthetic fixtures are NOT general SATLIB
//! performance evidence and do not change existing cache-admission policies.
use crate::work_proxy_holdout::{independent_wmc, selector_free_cnf, Holdout};
use rawmocs_core::{Gaussian, Problem, Weights};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchingFamily {
    Ring,
    Chorded,
    Mixed,
    Skewed,
}

impl BranchingFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ring => "ring",
            Self::Chorded => "chorded",
            Self::Mixed => "mixed",
            Self::Skewed => "skewed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ring" => Some(Self::Ring),
            "chorded" => Some(Self::Chorded),
            "mixed" => Some(Self::Mixed),
            "skewed" => Some(Self::Skewed),
            _ => None,
        }
    }
}

/// Predeclare a distinct connected CNF with a variable-degree hub and both
/// binary / ternary mixed-polarity clauses, no duplicated free selector.
fn skewed_clauses(n: usize) -> Vec<Vec<i32>> {
    assert!([8, 10, 12].contains(&n));
    let v = |i: usize| ((i % n) + 1) as i32;
    let mut clauses = Vec::new();
    for i in 0..n {
        clauses.push(vec![v(i), if i % 3 == 0 { -v(i + 1) } else { v(i + 1) }]);
    }
    for i in 2..n {
        if i % 2 == 0 {
            clauses.push(vec![v(0), -v(i), v(i + 2)]);
        } else {
            clauses.push(vec![-v(0), v(i)]);
        }
    }
    for i in 0..(n / 3) {
        clauses.push(vec![v(i + 1), -v(i + 3), v(i + 5)]);
    }
    clauses
}

fn exact_gaussian_wmc(clauses: &[Vec<i32>], variables: usize) -> (bool, Gaussian) {
    let pos = Gaussian::integers(2, 1);
    let neg = Gaussian::integers(1, -1);
    let mut counts = vec![0i64; variables + 1];
    let mut sat = false;
    // Independent truth-table evaluation, NOT a DPLL/search/cache oracle.
    for mask in 0usize..(1usize << variables) {
        let holds = clauses.iter().all(|clause| {
            clause.iter().any(|&lit| {
                let value = mask & (1usize << (lit.unsigned_abs() as usize - 1)) != 0;
                if lit > 0 {
                    value
                } else {
                    !value
                }
            })
        });
        if holds {
            sat = true;
            counts[variables - mask.count_ones() as usize] += 1;
        }
    }
    let mut pows_pos = vec![Gaussian::one()];
    let mut pows_neg = vec![Gaussian::one()];
    for i in 0..variables {
        pows_pos.push(pows_pos[i].mul(&pos));
        pows_neg.push(pows_neg[i].mul(&neg));
    }
    let mut sum = Gaussian::zero();
    for (negatives, &models) in counts.iter().enumerate() {
        if models != 0 {
            sum = sum.add(
                &pows_pos[variables - negatives]
                    .mul(&pows_neg[negatives])
                    .mul(&Gaussian::integers(models, 0)),
            );
        }
    }
    (sat, sum)
}

pub fn fixture(family: BranchingFamily, variables: usize) -> (Problem, bool, Gaussian) {
    assert!([8, 10, 12].contains(&variables));
    if family == BranchingFamily::Skewed {
        let clauses = skewed_clauses(variables);
        let (sat, count) = exact_gaussian_wmc(&clauses, variables);
        let weights = (0..variables)
            .map(|_| Weights::new(Gaussian::integers(2, 1), Gaussian::integers(1, -1)))
            .collect();
        (
            Problem::new(variables, clauses, weights).expect("predeclared skewed connected CNF"),
            sat,
            count,
        )
    } else {
        let existing = Holdout::parse(family.label()).expect("known source-only holdout");
        let p = selector_free_cnf(existing, variables);
        let (sat, count) = independent_wmc(existing, variables);
        (p, sat, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{
        solve_with_limits, solve_with_pivot_policy, Limits, PivotPolicy, SolveError,
    };

    #[test]
    fn exact_full_complex_wmc_and_sat_match_all_pivots_and_cache_states() {
        for family in [
            BranchingFamily::Ring,
            BranchingFamily::Chorded,
            BranchingFamily::Mixed,
            BranchingFamily::Skewed,
        ] {
            for variables in [8, 10, 12] {
                let (problem, sat, count) = fixture(family, variables);
                for cap in [0, 10_000] {
                    let limits = Limits {
                        cache_entries: cap,
                        ..Limits::default()
                    };
                    let base = solve_with_limits(&problem, limits).unwrap();
                    assert_eq!(base.satisfiable, sat);
                    assert_eq!(base.count, count);
                    let freq =
                        solve_with_pivot_policy(&problem, limits, PivotPolicy::Frequency).unwrap();
                    assert_eq!(freq.count, count);
                    assert_eq!(freq.satisfiable, sat);
                    assert_eq!(freq.stats.nodes, base.stats.nodes);
                    assert_eq!(freq.stats.cache_hits, base.stats.cache_hits);
                    assert_eq!(freq.stats.decompositions, base.stats.decompositions);
                    for policy in [PivotPolicy::ShortClause, PivotPolicy::MinFill] {
                        let answer = solve_with_pivot_policy(&problem, limits, policy).unwrap();
                        assert_eq!(
                            answer.count, count,
                            "{family:?}/{variables}/{cap}/{policy:?}"
                        );
                        assert_eq!(answer.satisfiable, sat);
                        assert!(answer.stats.pivot_decisions > 0);
                    }
                }
            }
        }
    }

    #[test]
    fn limits_remain_fail_closed_under_all_research_pivots() {
        let (problem, _, _) = fixture(BranchingFamily::Skewed, 12);
        for policy in [
            PivotPolicy::Frequency,
            PivotPolicy::ShortClause,
            PivotPolicy::MinFill,
        ] {
            let err = solve_with_pivot_policy(
                &problem,
                Limits {
                    nodes: 0,
                    ..Limits::default()
                },
                policy,
            );
            assert_eq!(err, Err(SolveError::NodeBudgetExceeded));
        }
    }
}
