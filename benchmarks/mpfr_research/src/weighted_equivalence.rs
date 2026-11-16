//! Bounded model-bijective elimination of equivalent signed literals.
//!
//! Detects literal equivalence via SCCs of the ORIGINAL weighted CNF's
//! binary implication graph. Eliminating an equivalent variable without
//! folding both signed complex literal weights is WMC-unsound; here the
//! representative's true/false weights contain the products of all class
//! member weights with the proven signs. No new output representation,
//! QASM/Path-sum information, or SAT-only simplification is used.
use crate::{Clause, Problem, Weights, C};
use std::collections::{BTreeMap, BTreeSet};

const MAX_VARIABLES: usize = 4096;
const MAX_CLAUSES: usize = 50_000;
const MAX_EDGES: usize = 150_000;

#[derive(Clone, Debug, Default)]
pub(crate) struct EquivalenceStats {
    pub(crate) binary_clauses: usize,
    pub(crate) eliminated_vars: usize,
    pub(crate) removed_clauses: usize,
    pub(crate) representatives: usize,
    pub(crate) contradictory: bool,
    pub(crate) censored: bool,
}

fn original(p: &Problem, stats: EquivalenceStats) -> (Problem, Vec<usize>, EquivalenceStats) {
    (
        Problem { n: p.n, clauses: p.clauses.clone(), weights: p.weights.clone() },
        (1..=p.n).collect(),
        stats,
    )
}

fn literal_index(literal: i32) -> usize {
    2 * (literal.unsigned_abs() as usize - 1) + usize::from(literal < 0)
}

fn graph_components(graph: &[Vec<usize>], reverse: &[Vec<usize>]) -> Vec<usize> {
    // Iterative Kosaraju, bounded by the input DIMACS variable/edge caps.
    // No recursion depending on adversarial graph depth.
    let n = graph.len();
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);
    for start in 0..n {
        if visited[start] { continue; }
        visited[start] = true;
        let mut stack = vec![(start, 0_usize)];
        while let Some((node, next_index)) = stack.last_mut() {
            if *next_index < graph[*node].len() {
                let child = graph[*node][*next_index];
                *next_index += 1;
                if !visited[child] {
                    visited[child] = true;
                    stack.push((child, 0));
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }
    let mut component = vec![usize::MAX; n];
    let mut number = 0;
    for &start in order.iter().rev() {
        if component[start] != usize::MAX { continue; }
        let mut stack = vec![start];
        component[start] = number;
        while let Some(node) = stack.pop() {
            for &parent in &reverse[node] {
                if component[parent] == usize::MAX {
                    component[parent] = number;
                    stack.push(parent);
                }
            }
        }
        number += 1;
    }
    component
}

/// A bounded source-CNF-only exact transformation; returned scope excludes
/// eliminated variables. This scope is essential: multiplying eliminated
/// variables' free-weight sums again would silently overcount.
pub(crate) fn reduce(
    p: &Problem, bits: u32
) -> (Problem, Vec<usize>, EquivalenceStats) {
    let mut stats = EquivalenceStats::default();
    if p.n == 0 || p.n > MAX_VARIABLES || p.clauses.len() > MAX_CLAUSES {
        stats.censored = true;
        return original(p, stats);
    }
    let size = 2 * p.n;
    let mut graph = vec![Vec::new(); size];
    let mut reverse = vec![Vec::new(); size];
    let mut edges = 0;
    for clause in &p.clauses {
        if clause.len() != 2 { continue; }
        let (a, b) = (clause[0], clause[1]);
        if a == -b || a == b { continue; }
        stats.binary_clauses += 1;
        // (a OR b) is equivalent to both implications.
        for (from, to) in [(-a, b), (-b, a)] {
            if edges == MAX_EDGES {
                stats.censored = true;
                return original(p, stats);
            }
            let from = literal_index(from);
            let to = literal_index(to);
            graph[from].push(to);
            reverse[to].push(from);
            edges += 1;
        }
    }
    if edges == 0 { return original(p, stats); }
    let scc = graph_components(&graph, &reverse);
    for variable in 1..=p.n {
        if scc[2 * (variable - 1)] == scc[2 * (variable - 1) + 1] {
            // Boolean contradiction, independent of any complex weight.
            stats.contradictory = true;
            return (
                Problem { n: p.n, clauses: vec![Vec::new()], weights: p.weights.clone() },
                (1..=p.n).collect(),
                stats,
            );
        }
    }
    // Canonical class ID is the unordered pair of SCCs for v and !v.
    // The smallest source variable is the representative. SCC orientation
    // establishes whether each class member equals rep or negates rep.
    let mut representatives: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut mapping = vec![(0_usize, false); p.n + 1];
    let mut classes: BTreeMap<usize, Vec<(usize, bool)>> = BTreeMap::new();
    for variable in 1..=p.n {
        let positive = scc[2 * (variable - 1)];
        let negative = scc[2 * (variable - 1) + 1];
        let key = (positive.min(negative), positive.max(negative));
        let &rep = representatives.entry(key).or_insert(variable);
        let rep_positive = scc[2 * (rep - 1)];
        let same = positive == rep_positive;
        // Same unordered SCC pair: either same polarity or opposite.
        debug_assert!(same || positive == scc[2 * (rep - 1) + 1]);
        mapping[variable] = (rep, same);
        classes.entry(rep).or_default().push((variable, same));
    }
    stats.representatives = classes.len();
    stats.eliminated_vars = p.n - classes.len();
    if stats.eliminated_vars == 0 {
        return original(p, stats);
    }
    let mut weights = p.weights.clone();
    for (&rep, members) in &classes {
        if members.len() == 1 { continue; }
        let mut positive = C::one(bits);
        let mut negative = C::one(bits);
        for &(var, same) in members {
            let w = &p.weights[var - 1];
            let (for_positive, for_negative) =
                if same { (&w.positive, &w.negative) }
                else { (&w.negative, &w.positive) };
            positive = positive.mul(for_positive, bits);
            negative = negative.mul(for_negative, bits);
        }
        weights[rep - 1] = Weights { positive, negative };
    }
    let mut clauses = Vec::with_capacity(p.clauses.len());
    let mut unique = BTreeSet::<Clause>::new();
    for clause in &p.clauses {
        let mut reduced = Vec::with_capacity(clause.len());
        for &lit in clause {
            let (rep, same) = mapping[lit.unsigned_abs() as usize];
            let positive = (lit > 0) == same;
            reduced.push(if positive { rep as i32 } else { -(rep as i32) });
        }
        reduced.sort_unstable();
        reduced.dedup();
        if reduced.iter().any(|v| reduced.binary_search(&(-v)).is_ok()) {
            stats.removed_clauses += 1;
            continue;
        }
        if unique.insert(reduced.clone()) {
            clauses.push(reduced);
        } else {
            stats.removed_clauses += 1;
        }
    }
    let scope = classes.keys().copied().collect();
    (Problem { n: p.n, clauses, weights }, scope, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, CachePolicy, Engine, Outcome};

    fn evaluate(clauses: &[Clause], mask: usize) -> bool {
        clauses.iter().all(|clause| clause.iter().any(|&lit| {
            let value = mask & (1 << (lit.unsigned_abs() as usize - 1)) != 0;
            value == (lit > 0)
        }))
    }
    fn brute_weighted(p: &Problem, bits: u32) -> Outcome {
        let mut value = C::zero(bits);
        let mut sat = false;
        for mask in 0..(1_usize << p.n) {
            if !evaluate(&p.clauses, mask) { continue; }
            sat = true;
            let mut weight = C::one(bits);
            for v in 1..=p.n {
                let w = &p.weights[v - 1];
                weight = weight.mul(if mask & (1 << (v - 1)) != 0 {
                    &w.positive
                } else {
                    &w.negative
                }, bits);
            }
            value = value.add(&weight, bits);
        }
        Outcome { sat, count: value }
    }
    fn make_problem(clauses: &[Clause], bits: u32) -> Problem {
        let mut source = format!("p cnf 6 {}\nc t wmc\n", clauses.len());
        for v in 1..=6 {
            source.push_str(&format!("c p weight {v} 0.25 0.125 0\n"));
            source.push_str(&format!("c p weight -{v} -0.125 0.25 0\n"));
        }
        for clause in clauses {
            for lit in clause { source.push_str(&format!("{lit} ")); }
            source.push_str("0\n");
        }
        parse_emitted(&source, bits).unwrap()
    }
    fn compare(p: &Problem, bits: u32) -> EquivalenceStats {
        let brute = brute_weighted(p, bits);
        let mut control = Engine::new(p, bits, CachePolicy::StdOwned);
        let original = control.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
        let (reduced, scope, stats) = reduce(p, bits);
        let mut engine = Engine::new(&reduced, bits, CachePolicy::StdOwned);
        let optimized = engine.count(reduced.clauses.clone(), scope, 0).unwrap();
        assert_eq!(original.sat, brute.sat);
        assert_eq!(original.count.re, brute.count.re);
        assert_eq!(original.count.im, brute.count.im);
        assert_eq!(optimized.sat, brute.sat);
        assert_eq!(optimized.count.re, brute.count.re);
        assert_eq!(optimized.count.im, brute.count.im);
        stats
    }

    #[test]
    fn equivalent_and_opposite_literals_fold_complex_weights_without_overcount() {
        let clauses = vec![
            vec![-1, 2], vec![1, -2], // x1 = x2
            vec![-2, -3], vec![2, 3], // x2 = !x3
            vec![3, 4], vec![-4, 5],
        ];
        for bits in [100, 333] {
            let p = make_problem(&clauses, bits);
            let stats = compare(&p, bits);
            assert_eq!(stats.eliminated_vars, 2);
            assert!(stats.removed_clauses >= 4);
        }
    }

    #[test]
    fn randomized_small_binary_equivalence_with_mixed_clauses_and_free_variables() {
        for bits in [100, 333] {
            for seed in 0..48_usize {
                let mut clauses = vec![vec![-1, 2], vec![1, -2]];
                if seed & 1 != 0 {
                    clauses.extend([vec![-2, -3], vec![2, 3]]);
                }
                if seed & 2 != 0 {
                    clauses.extend([vec![-4, 5], vec![4, -5]]);
                }
                for k in 0..3 {
                    let a = (1 + ((seed + 3 * k) % 6)) as i32;
                    let b = (1 + ((seed * 3 + 2 * k + 1) % 6)) as i32;
                    let c = (1 + ((seed * 5 + k + 2) % 6)) as i32;
                    clauses.push(vec![
                        if k & 1 == 0 { a } else { -a },
                        if seed & 4 == 0 { b } else { -b }, c
                    ]);
                }
                if seed & 8 != 0 { clauses.push(vec![6]); }
                if seed & 16 != 0 { clauses.push(vec![-6]); }
                let p = make_problem(&clauses, bits);
                compare(&p, bits);
            }
        }
    }

    #[test]
    fn equivalent_literals_with_cancelling_wmc_are_sat_not_unsat() {
        for bits in [100, 333] {
            let text = "p cnf 2 2\nc t wmc\nc p weight 1 1 0 0\nc p weight -1 -1 0 0\n-1 2 0\n1 -2 0\n";
            let p = parse_emitted(text, bits).unwrap();
            let (reduced, scope, stats) = reduce(&p, bits);
            assert_eq!(stats.eliminated_vars, 1);
            let mut engine = Engine::new(&reduced, bits, CachePolicy::StdOwned);
            let result = engine.count(reduced.clauses.clone(), scope, 0).unwrap();
            assert!(result.sat);
            assert_eq!(result.count.re, 0);
            assert_eq!(result.count.im, 0);
        }
    }

    #[test]
    fn binary_scc_boolean_contradiction_is_not_numerical_cancellation() {
        let clauses = vec![
            vec![1, 2], vec![-1, 2], vec![1, -2], vec![-1, -2],
        ];
        for bits in [100, 333] {
            let p = make_problem(&clauses, bits);
            let stats = compare(&p, bits);
            assert!(stats.contradictory);
        }
    }

    #[test]
    fn workload_cap_returns_the_exact_original_source() {
        let mut p = make_problem(&[vec![-1, 2], vec![1, -2]], 100);
        p.clauses.extend(vec![vec![1, 2]; MAX_CLAUSES]);
        let (reduced, scope, stats) = reduce(&p, 100);
        assert!(stats.censored);
        assert_eq!(reduced.clauses, p.clauses);
        assert_eq!(scope, (1..=p.n).collect::<Vec<_>>());
    }
}
