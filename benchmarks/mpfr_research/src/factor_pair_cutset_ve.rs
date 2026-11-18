//! Research-only, bounded *two-variable* Shannon cutset for original Weighted CNF.
//!
//! First run graph-only sufficient width-11 admission after deleting the two
//! chosen variables from the source primal graph. Then count all four exact
//! source-CNF cofactors using the existing dense complex Factor-VE.
//! If any planner, wall-clock work gate, factor arithmetic, or branch fails,
//! discard all partial data and run the original source Trail in the caller.
use crate::{factor_minfill_order, factor_pair_fastgraph::FastGraph, factor_variable_elimination, C, Clause, Outcome, Problem};
use std::collections::HashMap;
use std::time::Instant;

const MAX_SOURCE_VARS: usize = 4096;
const MAX_SCOPE: usize = 320;
const MAX_CLAUSES: usize = 3000;
const MAX_ROOT_BUCKET: usize = 14;
const MAX_BAG: usize = 11;
const MAX_TOP_VERTICES: usize = 12;
const MAX_PAIR_PROBES: usize = 24;
// A finite, measured planning budget; WallTime includes every failed attempt.
const MAX_PAIR_PLANNING_NS: u128 = 180_000_000;

#[derive(Default, Clone, Debug)]
pub(crate) struct PairStats {
    pub(crate) eligible: bool,
    pub(crate) capped: bool,
    pub(crate) reason: &'static str,
    pub(crate) root_bucket: usize,
    pub(crate) fast_graph: bool,
    pub(crate) graph_build_ns: u128,
    pub(crate) vertex_candidates: usize,
    pub(crate) pair_probes: usize,
    pub(crate) feasible_pairs: usize,
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) graph_upper_bucket: usize,
    pub(crate) branch_width: usize,
    pub(crate) branches_completed: usize,
    pub(crate) products: usize,
    pub(crate) planning_ns: u128,
}

fn reject(stats: &mut PairStats, reason: &'static str) -> Option<Outcome> {
    stats.capped = true;
    stats.reason = reason;
    None
}

fn rank_candidates(clauses: &[Clause], scope: &[usize]) -> Vec<usize> {
    let positions: HashMap<usize, usize> = scope.iter().copied()
        .enumerate().map(|(i, v)| (v, i)).collect();
    let n = scope.len();
    let mut adjacency = vec![vec![false; n]; n];
    let mut degree = vec![0_usize; n];
    for clause in clauses {
        let mut indices: Vec<usize> = clause.iter()
            .filter_map(|lit| positions.get(&(lit.unsigned_abs() as usize)).copied())
            .collect();
        indices.sort_unstable();
        indices.dedup();
        for (i, &a) in indices.iter().enumerate() {
            for &b in &indices[i + 1..] {
                if !adjacency[a][b] {
                    adjacency[a][b] = true;
                    adjacency[b][a] = true;
                    degree[a] += 1;
                    degree[b] += 1;
                }
            }
        }
    }
    let mut result = scope.to_vec();
    result.sort_unstable_by_key(|v| (std::cmp::Reverse(degree[positions[v]]), *v));
    result.truncate(MAX_TOP_VERTICES);
    result
}

fn deleted_graph_clauses(clauses: &[Clause], x: usize, y: usize) -> Vec<Clause> {
    clauses.iter().map(|clause| clause.iter().copied().filter(|lit| {
        let variable = lit.unsigned_abs() as usize;
        variable != x && variable != y
    }).collect()).collect()
}

fn cofactor(clauses: &[Clause], x: usize, vx: bool, y: usize, vy: bool) -> Vec<Clause> {
    let lx = if vx { x as i32 } else { -(x as i32) };
    let ly = if vy { y as i32 } else { -(y as i32) };
    let mut result = Vec::with_capacity(clauses.len());
    for clause in clauses {
        if clause.contains(&lx) || clause.contains(&ly) { continue; }
        result.push(clause.iter().copied().filter(|&lit| lit != -lx && lit != -ly).collect());
    }
    result
}

pub(crate) fn count(p: &Problem, scope: &[usize], bits: u32)
    -> (Option<Outcome>, PairStats) {
    count_with_graph(p, scope, bits, false)
}

pub(crate) fn count_fast(p: &Problem, scope: &[usize], bits: u32)
    -> (Option<Outcome>, PairStats) {
    count_with_graph(p, scope, bits, true)
}

fn count_with_graph(p: &Problem, scope: &[usize], bits: u32, fast: bool)
    -> (Option<Outcome>, PairStats) {
    let mut stats = PairStats { fast_graph: fast, ..PairStats::default() };
    let answer = attempt(p, scope, bits, fast, &mut stats);
    (answer, stats)
}

fn attempt(p: &Problem, scope: &[usize], bits: u32, fast: bool,
           stats: &mut PairStats) -> Option<Outcome> {
    if p.n == 0 || p.n > MAX_SOURCE_VARS || scope.len() < 3
        || scope.len() > MAX_SCOPE || p.clauses.len() > MAX_CLAUSES
        || p.weights.len() < p.n {
        return reject(stats, "pair_source_envelope");
    }
    let mut present = vec![false; p.n + 1];
    for &v in scope {
        if v == 0 || v > p.n || present[v] {
            return reject(stats, "pair_invalid_scope");
        }
        present[v] = true;
    }
    if p.clauses.iter().flatten().any(|lit| {
        let v = lit.unsigned_abs() as usize;
        v == 0 || v > p.n || !present[v]
    }) {
        return reject(stats, "pair_clause_scope");
    }
    stats.eligible = true;
    let root = factor_minfill_order::plan(&p.clauses, scope, MAX_BAG);
    stats.root_bucket = root.max_bucket;
    if !root.capped { return reject(stats, "pair_not_needed"); }
    if root.max_bucket > MAX_ROOT_BUCKET { return reject(stats, "pair_root_too_wide"); }

    let begin = Instant::now();
    let graph_begin = Instant::now();
    let fast_graph = if fast {
        match FastGraph::build(&p.clauses, scope) {
            Some(g) => Some(g),
            None => return reject(stats, "pair_fast_graph_source"),
        }
    } else {
        None
    };
    stats.graph_build_ns = graph_begin.elapsed().as_nanos();
    let ranked = if let Some(graph) = &fast_graph {
        graph.ranked(MAX_TOP_VERTICES)
    } else {
        rank_candidates(&p.clauses, scope)
    };
    stats.vertex_candidates = ranked.len();
    let mut selected: Option<(usize, usize, Vec<usize>, usize)> = None;

    'pairs: for (i, &x) in ranked.iter().enumerate() {
        for &y in &ranked[i + 1..] {
            if stats.pair_probes >= MAX_PAIR_PROBES
                || begin.elapsed().as_nanos() > MAX_PAIR_PLANNING_NS {
                break 'pairs;
            }
            stats.pair_probes += 1;
            let remaining: Vec<usize> = scope.iter().copied()
                .filter(|&v| v != x && v != y).collect();
            // The deleted-variable CNF primal graph is a SUPERGRAPH of
            // every possible conditioned cofactor graph. If it fits
            // width 11, it proves at least one safe elimination order
            // for all four conditioned branches without doing arithmetic.
            let plan = if let Some(graph) = &fast_graph {
                graph.plan_deleted(x, y, MAX_BAG)
            } else {
                let clauses = deleted_graph_clauses(&p.clauses, x, y);
                factor_minfill_order::plan(&clauses, &remaining, MAX_BAG)
            };
            if !plan.capped {
                stats.feasible_pairs += 1;
                selected = Some((x, y, remaining, plan.max_bucket));
                break 'pairs;
            }
        }
    }
    stats.planning_ns = begin.elapsed().as_nanos();
    let Some((x, y, remaining, bucket)) = selected else {
        return reject(stats, if stats.pair_probes >= MAX_PAIR_PROBES {
            "pair_probe_budget"
        } else if stats.planning_ns > MAX_PAIR_PLANNING_NS {
            "pair_planning_budget"
        } else {
            "pair_no_width11_superset"
        });
    };
    stats.x = x;
    stats.y = y;
    stats.graph_upper_bucket = bucket;

    let mut sat = false;
    let mut total = C::zero(bits);
    for vx in [false, true] {
        for vy in [false, true] {
            let conditioned = cofactor(&p.clauses, x, vx, y, vy);
            let reduced = Problem { n: p.n, clauses: conditioned, weights: p.weights.clone() };
            let (out, info) = factor_variable_elimination::count_scope_probe_minfill(
                &reduced, &remaining, bits);
            stats.branch_width = stats.branch_width.max(info.width);
            stats.products += info.products;
            let Some(out) = out else { return reject(stats, "pair_dense_cap"); };
            stats.branches_completed += 1;
            if !out.sat { continue; }
            sat = true;
            let wx = if vx { &p.weights[x - 1].positive }
                else { &p.weights[x - 1].negative };
            let wy = if vy { &p.weights[y - 1].positive }
                else { &p.weights[y - 1].negative };
            let weighted = wx.mul(wy, bits).mul(&out.count, bits);
            total = total.add(&weighted, bits);
        }
    }
    Some(Outcome { sat, count: total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, trail_engine::TrailEngine};

    fn independent_brute(p: &Problem, bits: u32) -> Outcome {
        assert!(p.n <= 13);
        let mut sat = false;
        let mut count = C::zero(bits);
        for mask in 0..(1_usize << p.n) {
            let valid = p.clauses.iter().all(|clause| clause.iter().any(|&lit| {
                let v = lit.unsigned_abs() as usize - 1;
                (mask & (1 << v) != 0) == (lit > 0)
            }));
            if !valid { continue; }
            sat = true;
            let mut value = C::one(bits);
            for (v, weights) in p.weights.iter().enumerate().take(p.n) {
                value = value.mul(if mask & (1 << v) != 0 {
                    &weights.positive
                } else { &weights.negative }, bits);
            }
            count = count.add(&value, bits);
        }
        Outcome { sat, count }
    }

    #[test]
    fn thirteen_variable_clique_requires_two_conditioned_variables() {
        let mut input = String::from(
            "p cnf 13 78\nc t wmc\n\
c p weight 1 0.5 0.25 0\nc p weight -1 -0.25 -0.5 0\n\
c p weight 2 -0.5 0.5 0\nc p weight -2 0.25 -0.25 0\n",
        );
        for a in 1..=13 {
            for b in a + 1..=13 { input.push_str(&format!("{a} {b} 0\n")); }
        }
        for bits in [100, 333] {
            let problem = parse_emitted(&input, bits).unwrap();
            let scope: Vec<usize> = (1..=problem.n).collect();
            let root = factor_minfill_order::plan(&problem.clauses, &scope, MAX_BAG);
            assert!(root.capped && root.max_bucket == 13);
            let (got, stats) = count(&problem, &scope, bits);
            assert!(got.is_some() && !stats.capped, "{stats:?}");
            assert_eq!(stats.branches_completed, 4);
            assert_eq!(stats.graph_upper_bucket, 11);
            let got = got.unwrap();
            let (fast, fast_stats) = count_fast(&problem, &scope, bits);
            assert!(fast.is_some() && !fast_stats.capped, "{fast_stats:?}");
            assert!(fast_stats.fast_graph);
            assert_eq!(fast_stats.branches_completed, 4);
            assert_eq!(fast_stats.graph_upper_bucket, stats.graph_upper_bucket);
            let fast = fast.unwrap();
            assert_eq!(fast.sat, got.sat);
            assert_eq!(fast.count.re, got.count.re);
            assert_eq!(fast.count.im, got.count.im);
            let reference = independent_brute(&problem, bits);
            assert_eq!(got.sat, reference.sat);
            assert_eq!(got.count.re, reference.count.re);
            assert_eq!(got.count.im, reference.count.im);
            let mut trail = TrailEngine::new(&problem, bits, 512, 100_000);
            let old = trail.count(scope, 0).unwrap();
            assert_eq!(got.sat, old.sat);
            assert_eq!(got.count.re, old.count.re);
            assert_eq!(got.count.im, old.count.im);
        }
    }

    #[test]
    fn fitting_original_graph_does_not_enter_expensive_cutset() {
        let p = parse_emitted("p cnf 4 2\nc t wmc\n1 -2 0\n3 4 0\n", 100).unwrap();
        let scope: Vec<usize> = (1..=p.n).collect();
        let (out, stats) = count(&p, &scope, 100);
        assert!(out.is_none() && stats.capped);
        assert_eq!(stats.reason, "pair_not_needed");
        let (fast, fast_stats) = count_fast(&p, &scope, 100);
        assert!(fast.is_none() && fast_stats.capped);
        assert_eq!(fast_stats.reason, "pair_not_needed");
        let mut trail = TrailEngine::new(&p, 100, 128, 100_000);
        let old = trail.count(scope, 0).unwrap();
        let independent = independent_brute(&p, 100);
        assert_eq!(old.sat, independent.sat);
        assert_eq!(old.count.re, independent.count.re);
        assert_eq!(old.count.im, independent.count.im);
    }
}
