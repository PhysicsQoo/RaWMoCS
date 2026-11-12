//! Research-only family of selector-guarded connected CNFs with varied graphs.
//! An independent exhaustive independent-set polynomial determines exact Full
//! Complex WMC, rather than calling DPLL, the production solver or its cache.
//! The family is synthetic: no projected-WMC or real-world runtime claim.

use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graph {
    Cycle,
    Rungs,
    Spokes,
}

impl Graph {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cycle => "cycle",
            Self::Rungs => "rungs",
            Self::Spokes => "spokes",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "cycle" => Some(Self::Cycle),
            "rungs" => Some(Self::Rungs),
            "spokes" => Some(Self::Spokes),
            _ => None,
        }
    }
}

fn ratio(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn selector() -> Weights {
    Weights::new(
        Gaussian::new(ratio(1, 2), ratio(1, 3)),
        Gaussian::new(ratio(3, 5), ratio(-1, 7)),
    )
}

fn vertex() -> Weights {
    Weights::new(Gaussian::integers(2, 1), Gaussian::integers(1, -1))
}

fn edges(graph: Graph, n: usize) -> Vec<(usize, usize)> {
    assert!([6, 10, 14].contains(&n), "predeclared even graph sizes");
    let mut edges = Vec::new();
    let mut add = |a: usize, b: usize| {
        let edge = (a.min(b), a.max(b));
        if !edges.contains(&edge) {
            edges.push(edge);
        }
    };
    for i in 0..n {
        add(i, (i + 1) % n);
    }
    match graph {
        Graph::Cycle => {}
        Graph::Rungs => {
            for i in 0..(n / 2) {
                add(i, i + n / 2);
            }
        }
        Graph::Spokes => {
            for i in (3..n).step_by(2) {
                add(0, i);
            }
        }
    }
    edges.sort_unstable();
    edges
}

/// Every graph edge (a,b) becomes (x|y_a|y_b)&(!x|y_a|y_b).
/// Both selector branches produce the same nontrivial connected residual;
/// fixed graph families differ in vertex degree, cycles and branching.
pub fn guarded_graph(graph: Graph, n: usize) -> Problem {
    let edges = edges(graph, n);
    let mut clauses = Vec::with_capacity(2 * edges.len());
    for (a, b) in edges {
        let a = (a + 2) as i32;
        let b = (b + 2) as i32;
        clauses.push(vec![1, a, b]);
        clauses.push(vec![-1, a, b]);
    }
    let mut weights = vec![selector()];
    weights.extend((0..n).map(|_| vertex()));
    Problem::new(n + 1, clauses, weights).expect("source-locked guarded graph")
}

/// Exact semantic oracle: enumerate assignments with false-vertex set F.
/// Each graph edge excludes (false,false), so F must be an independent set.
/// Sum complex weights by |F| using integer independent-set coefficients.
/// No recurrence or caching from the solver appears in this computation.
pub fn independent_exact_wmc(graph: Graph, n: usize) -> Gaussian {
    let edges = edges(graph, n);
    let mut coeff = vec![0i64; n + 1];
    for mask in 0usize..(1usize << n) {
        if edges.iter().all(|&(a, b)| mask & (1 << a) == 0 || mask & (1 << b) == 0) {
            coeff[mask.count_ones() as usize] += 1;
        }
    }
    let w = vertex();
    let mut positive_powers = vec![Gaussian::one()];
    let mut negative_powers = vec![Gaussian::one()];
    for i in 0..n {
        positive_powers.push(positive_powers[i].mul(&w.positive));
        negative_powers.push(negative_powers[i].mul(&w.negative));
    }
    let mut sum = Gaussian::zero();
    for (false_count, &models) in coeff.iter().enumerate() {
        if models != 0 {
            let term = positive_powers[n - false_count]
                .mul(&negative_powers[false_count])
                .mul(&Gaussian::integers(models, 0));
            sum = sum.add(&term);
        }
    }
    let s = selector();
    sum.mul(&s.positive.add(&s.negative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{solve_with_limits, solve_with_min_cache_clauses, Limits, SolveError};

    #[test]
    fn independent_oracle_agrees_with_older_cycle_transfer_recurrence() {
        for n in [6, 10, 14] {
            assert_eq!(
                independent_exact_wmc(Graph::Cycle, n),
                crate::expensive_reuse::independent_cycle_wmc(n)
            );
        }
    }

    #[test]
    fn all_research_thresholds_preserve_exact_wmc_and_sat() {
        for graph in [Graph::Cycle, Graph::Rungs, Graph::Spokes] {
            for n in [6, 10, 14] {
                let p = guarded_graph(graph, n);
                let expected = independent_exact_wmc(graph, n);
                let legacy = solve_with_limits(&p, Limits::default()).unwrap();
                let off = solve_with_limits(
                    &p,
                    Limits { cache_entries: 0, ..Limits::default() },
                ).unwrap();
                assert_eq!(legacy.count, expected, "{} {n}", graph.label());
                assert!(legacy.satisfiable && off.satisfiable);
                assert_eq!(off.count, expected);
                let same = solve_with_min_cache_clauses(&p, Limits::default(), 0).unwrap();
                assert_eq!(same.count, legacy.count);
                assert_eq!(same.stats.nodes, legacy.stats.nodes);
                assert_eq!(same.stats.cache_hits, legacy.stats.cache_hits);
                for threshold in [4, 12] {
                    let got = solve_with_min_cache_clauses(&p, Limits::default(), threshold).unwrap();
                    assert!(got.satisfiable);
                    assert_eq!(got.count, expected, "{} {n} threshold {threshold}", graph.label());
                    assert!(got.stats.cache_min_clause_rejections > 0);
                    assert_eq!(got.stats.cache_payload_rejections, 0);
                }
            }
        }
    }

    #[test]
    fn search_budget_failure_remains_fail_closed_for_research_policy() {
        let p = guarded_graph(Graph::Spokes, 10);
        assert_eq!(
            solve_with_min_cache_clauses(
                &p,
                Limits { nodes: 0, ..Limits::default() },
                12
            ),
            Err(SolveError::NodeBudgetExceeded)
        );
    }
}
