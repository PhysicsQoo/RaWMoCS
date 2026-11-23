//! Bounded exact sum-product on binary-clause primal forests.
//! Opt-in research only. No source CNF rewrite or SAT-only reduction.
//! For cyclic/wide residuals return None: caller must use original Trail.
//! Every positive/negative signed Complex weight is owned once at its vertex.
use crate::{C, Clause, Outcome, Problem};
use std::collections::BTreeMap;

const MAX_SCOPE: usize = 512;
const MAX_CLAUSES: usize = 2_048;
const MAX_SOURCE_VARS: usize = 8_192;

#[derive(Default)]
pub(crate) struct ForestStats {
    pub(crate) eligible: bool,
    pub(crate) reason: &'static str,
    pub(crate) vertices: usize,
    pub(crate) edges: usize,
    pub(crate) roots: usize,
    pub(crate) states: usize,
}

fn find(parent: &mut [usize], mut v: usize) -> usize {
    while parent[v] != v {
        parent[v] = parent[parent[v]];
        v = parent[v];
    }
    v
}

fn clause_mask(first: i32, second: i32, low: usize) -> u8 {
    let mut mask = 0_u8;
    for a in 0..2 {
        for b in 0..2 {
            let first_true = if first.unsigned_abs() as usize == low {
                a == 1
            } else {
                b == 1
            };
            let second_true = if second.unsigned_abs() as usize == low {
                a == 1
            } else {
                b == 1
            };
            if first_true == (first > 0) || second_true == (second > 0) {
                mask |= 1 << (a + 2 * b);
            }
        }
    }
    mask
}

/// Count *exactly* the supplied scope/clauses. Caller owns all forced or
/// free-variable factors outside that scope. A successful result includes
/// Boolean SAT even if the signed complex sum cancels numerically to zero.
pub(crate) fn count_clauses(
    p: &Problem, scope: &[usize], clauses: &[Clause], bits: u32,
) -> (Option<Outcome>, ForestStats) {
    let mut stats = ForestStats {
        vertices: scope.len(), ..ForestStats::default()
    };
    if scope.len() > MAX_SCOPE || clauses.len() > MAX_CLAUSES || p.n > MAX_SOURCE_VARS {
        stats.reason = "BUDGET";
        return (None, stats);
    }
    if scope.windows(2).any(|w| w[0] >= w[1])
        || scope.iter().any(|&v| v == 0 || v > p.n)
        || p.weights.len() < p.n
    {
        stats.reason = "INVALID_SCOPE";
        return (None, stats);
    }
    let mut present = vec![false; p.n + 1];
    for &var in scope {
        present[var] = true;
    }
    let mut unary = vec![[true; 2]; p.n + 1];
    // A pair may carry several original clauses. Their allowed assignments
    // are intersected, never approximated as a single SAT constraint.
    let mut edges = BTreeMap::<(usize, usize), u8>::new();
    for clause in clauses {
        if clause.len() > 2 {
            stats.reason = "WIDE_CLAUSE";
            return (None, stats);
        }
        for &lit in clause {
            let var = lit.unsigned_abs() as usize;
            if var == 0 || var > p.n || !present[var] {
                stats.reason = "CROSS_SCOPE";
                return (None, stats);
            }
        }
        if clause.is_empty() {
            stats.eligible = true;
            stats.reason = "BOOLEAN_UNSAT";
            return (Some(Outcome::unsat(bits)), stats);
        }
        if clause.len() == 1 || clause[0].unsigned_abs() == clause[1].unsigned_abs() {
            if clause.len() == 2 && clause[0] == -clause[1] {
                continue; // tautological source clause
            }
            let lit = clause[0];
            unary[lit.unsigned_abs() as usize][usize::from(lit < 0)] = false;
            continue;
        }
        let a = clause[0].unsigned_abs() as usize;
        let b = clause[1].unsigned_abs() as usize;
        let pair = (a.min(b), a.max(b));
        let allowed = clause_mask(clause[0], clause[1], pair.0);
        *edges.entry(pair).or_insert(0b1111) &= allowed;
    }
    stats.edges = edges.len();
    let mut parent: Vec<usize> = (0..=p.n).collect();
    let mut graph = vec![Vec::<(usize, u8)>::new(); p.n + 1];
    for (&(a, b), &mask) in &edges {
        let ra = find(&mut parent, a);
        let rb = find(&mut parent, b);
        if ra == rb {
            stats.reason = "CYCLIC";
            return (None, stats);
        }
        parent[rb] = ra;
        graph[a].push((b, mask));
        graph[b].push((a, mask));
    }
    stats.eligible = true;
    stats.reason = "FOREST";
    let mut visited = vec![false; p.n + 1];
    let mut tree_parent = vec![0_usize; p.n + 1];
    let mut roots = Vec::new();
    let mut order = Vec::with_capacity(scope.len());
    for &root in scope {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        roots.push(root);
        let mut todo = vec![root];
        while let Some(v) = todo.pop() {
            order.push(v);
            for &(child, _) in &graph[v] {
                if child != tree_parent[v] && !visited[child] {
                    tree_parent[child] = v;
                    visited[child] = true;
                    todo.push(child);
                }
            }
        }
    }
    stats.roots = roots.len();
    let mut sat = vec![[false; 2]; p.n + 1];
    let mut value = vec![[C::zero(bits), C::zero(bits)]; p.n + 1];
    for &v in order.iter().rev() {
        for truth in 0..2 {
            if !unary[v][truth] {
                continue;
            }
            let mut count = if truth == 1 {
                p.weights[v - 1].positive.clone()
            } else {
                p.weights[v - 1].negative.clone()
            };
            let mut possible = true;
            for &(child, mask) in &graph[v] {
                if tree_parent[child] != v {
                    continue;
                }
                let mut child_sum = C::zero(bits);
                let mut child_possible = false;
                for assignment in 0..2 {
                    let index = if v < child {
                        truth + 2 * assignment
                    } else {
                        assignment + 2 * truth
                    };
                    if (mask & (1 << index)) != 0 && sat[child][assignment] {
                        child_possible = true;
                        child_sum = child_sum.add(&value[child][assignment], bits);
                    }
                }
                if !child_possible {
                    possible = false;
                    break;
                }
                count = count.mul(&child_sum, bits);
            }
            if possible {
                sat[v][truth] = true;
                value[v][truth] = count;
            }
            stats.states += 1;
        }
    }
    let mut total = C::one(bits);
    for root in roots {
        let mut subtotal = C::zero(bits);
        let mut possible = false;
        for truth in 0..2 {
            if sat[root][truth] {
                possible = true;
                subtotal = subtotal.add(&value[root][truth], bits);
            }
        }
        if !possible {
            return (Some(Outcome::unsat(bits)), stats);
        }
        total = total.mul(&subtotal, bits);
    }
    (Some(Outcome { sat: true, count: total }), stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, CachePolicy, Weights};
    fn problem(n: usize, clauses: Vec<Clause>, bits: u32) -> Problem {
        let weights = (0..n).map(|i| Weights {
            positive: C::decimal(bits, if i % 2 == 0 { "0.25" } else { "-0.125" }, "0.25").unwrap(),
            negative: C::decimal(bits, if i % 3 == 0 { "-0.5" } else { "0.125" }, "-0.125").unwrap(),
        }).collect();
        Problem { n, clauses, weights }
    }
    fn brute(p: &Problem, bits: u32) -> Outcome {
        assert!(p.n <= 12);
        let mut count = C::zero(bits);
        let mut sat = false;
        for mask in 0..(1_usize << p.n) {
            if !p.clauses.iter().all(|clause| clause.iter().any(|&lit| {
                let bit = (mask >> (lit.unsigned_abs() as usize - 1)) & 1 != 0;
                bit == (lit > 0)
            })) { continue; }
            sat = true;
            let mut term = C::one(bits);
            for (var, w) in p.weights.iter().enumerate() {
                term = term.mul(
                    if mask >> var & 1 == 1 { &w.positive } else { &w.negative },
                    bits,
                );
            }
            count = count.add(&term, bits);
        }
        Outcome { sat, count }
    }
    #[test]
    fn exact_signed_complex_forest_matches_independent_truth_table_and_trail() {
        let cases = [
            (10, vec![
                vec![1, 2], vec![-2, 3], vec![-3, -4],
                vec![4, 5], vec![-5, 6], vec![6, -7],
                vec![7, 8], vec![8, -9], vec![9, 10],
                vec![-1, -2], vec![3], vec![-10],
            ]),
            (8, vec![vec![1, 2], vec![-2, 3], vec![4, -5],
                     vec![-5, 6], vec![1], vec![-3],
                     vec![7, 8], vec![-7, -8]]),
            (2, vec![vec![1], vec![-1]]),
            (1, vec![vec![]]),
            (3, vec![vec![1, 2], vec![-1, 2], vec![3, -3]]),
        ];
        for bits in [100_u32, 333] {
            for (n, clauses) in cases.clone() {
                let p = problem(n, clauses, bits);
                let scope: Vec<_> = (1..=p.n).collect();
                let (answer, stats) = count_clauses(&p, &scope, &p.clauses, bits);
                assert!(stats.eligible);
                let answer = answer.expect("acyclic binary CNF must be counted");
                let oracle = brute(&p, bits);
                assert_eq!(answer.sat, oracle.sat);
                assert_eq!(answer.count.re, oracle.count.re);
                assert_eq!(answer.count.im, oracle.count.im);
                let mut trail = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                let expected = trail.count(p.clauses.clone(), scope, 0).unwrap();
                assert_eq!(answer.sat, expected.sat);
                assert_eq!(answer.count.re, expected.count.re);
                assert_eq!(answer.count.im, expected.count.im);
            }
        }
    }
    #[test]
    fn signed_zero_is_not_boolean_unsat_and_rejected_graphs_fall_back() {
        for bits in [100_u32, 333] {
            let mut p = problem(2, vec![vec![1, 2]], bits);
            p.weights[0].positive = C::zero(bits);
            p.weights[0].negative = C::zero(bits);
            let (result, _) = count_clauses(&p, &[1, 2], &p.clauses, bits);
            let result = result.unwrap();
            assert!(result.sat);
            assert_eq!(result.count.re, 0);
            assert_eq!(result.count.im, 0);
            for clauses in [
                vec![vec![1, 2], vec![2, 3], vec![1, 3]],
                vec![vec![1, 2, 3]],
                vec![vec![1, 4]],
            ] {
                let p = problem(4, clauses, bits);
                let (out, stats) = count_clauses(&p, &[1, 2, 3], &p.clauses, bits);
                assert!(out.is_none());
                assert!(!stats.eligible);
            }
        }
    }
    #[test]
    fn reversible_trail_forest_dp_reduces_search_nodes_without_weight_loss() {
        use crate::trail_engine::TrailEngine;
        let clauses: Vec<Clause> = (1..10)
            .map(|v| vec![v, -(v + 1)])
            .collect();
        for bits in [100_u32, 333] {
            let p = problem(10, clauses.clone(), bits);
            for capacity in [0_usize, 1, 128] {
                let run = |enabled: bool| {
                    let mut t = TrailEngine::new(&p, bits, capacity, 100_000);
                    t.use_parent_ids = true;
                    t.use_compact_residual = true;
                    t.use_prehashed_cache = true;
                    t.use_fused_compact = true;
                    t.use_weight_class_cache = true;
                    t.use_inline_weight_relabel = true;
                    t.use_forest_dp = enabled;
                    let out = t.count((1..=p.n).collect(), 0).unwrap();
                    (out, t)
                };
                let (base, control) = run(false);
                let (fast, candidate) = run(true);
                assert_eq!(fast.sat, base.sat);
                assert_eq!(fast.count.re, base.count.re);
                assert_eq!(fast.count.im, base.count.im);
                assert_eq!(control.forest_dp_completed, 0);
                assert!(candidate.forest_dp_completed > 0);
                assert!(candidate.stats.nodes < control.stats.nodes);
            }
        }
    }

}
