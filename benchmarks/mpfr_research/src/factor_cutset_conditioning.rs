//! Research-only single-variable cutset conditioning of the unchanged
//! original Weighted CNF followed by bounded dense min-fill Factor-VE.
//!
//! The overwidth bucket witness selects candidate cut variables. Each
//! branch cofactor is obtained solely by fixing that original source
//! variable; the original literal's signed complex weight is multiplied
//! back exactly once. No width, cell, or product cap is enlarged.
//! Failed or partially solved branches are fully discarded; the caller
//! falls back to the untouched original CNF and Trail solver.
use crate::{factor_minfill_order, factor_variable_elimination, C, Clause, Outcome, Problem};
use std::time::Instant;

const MAX_SOURCE_VARS: usize = 4096;
const MAX_SCOPE: usize = 320;
const MAX_CLAUSES: usize = 3000;
const MAX_CLAUSE_WIDTH: usize = 12;
const DENSE_BUCKET_CAP: usize = 11;
const MAX_CANDIDATES: usize = 16;
const MAX_BRANCH_SOLVES: usize = 4;

#[derive(Default, Clone, Debug)]
pub(crate) struct CutsetStats {
    pub(crate) eligible: bool,
    pub(crate) capped: bool,
    pub(crate) reason: &'static str,
    pub(crate) source_bucket: usize,
    pub(crate) witness_candidates: usize,
    pub(crate) attempted: usize,
    pub(crate) planned_branches: usize,
    pub(crate) feasible_pairs: usize,
    pub(crate) branch_solves: usize,
    pub(crate) completed_var: usize,
    pub(crate) max_branch_bucket: usize,
    pub(crate) max_branch_products: usize,
    pub(crate) preparation_ns: u128,
}

fn reject(stats: &mut CutsetStats, reason: &'static str) {
    stats.capped = true;
    if stats.reason.is_empty() { stats.reason = reason; }
}

fn conditional_clauses(clauses: &[Clause], var: usize, assigned: bool) -> Vec<Clause> {
    let chosen_literal = if assigned { var as i32 } else { -(var as i32) };
    clauses.iter().filter_map(|clause| {
        if clause.contains(&chosen_literal) {
            None
        } else {
            Some(clause.iter().copied().filter(|lit| {
                lit.unsigned_abs() as usize != var
            }).collect())
        }
    }).collect()
}

fn branch_plan(clauses: &[Clause], remaining: &[usize]) -> Option<usize> {
    // An empty clause is a direct Boolean contradiction, independent of
    // the weights of all remaining variables.
    if clauses.iter().any(Vec::is_empty) {
        return Some(0);
    }
    let plan = factor_minfill_order::plan(clauses, remaining, DENSE_BUCKET_CAP);
    if plan.capped { None } else { Some(plan.max_bucket) }
}

fn branch_count(
    p: &Problem, scope: &[usize], clauses: &[Clause], bits: u32,
) -> (Option<Outcome>, usize) {
    if clauses.iter().any(Vec::is_empty) {
        return (Some(Outcome::unsat(bits)), 0);
    }
    let (count, stats) = factor_variable_elimination::count_conditioned_minfill(
        p, scope, clauses, bits,
    );
    (count, stats.products)
}

fn combine(p: &Problem, var: usize, negative: &Outcome, positive: &Outcome, bits: u32)
    -> Outcome {
    let mut count = C::zero(bits);
    if negative.sat {
        count = count.add(&p.weights[var - 1].negative.mul(&negative.count, bits), bits);
    }
    if positive.sat {
        count = count.add(&p.weights[var - 1].positive.mul(&positive.count, bits), bits);
    }
    Outcome { sat: negative.sat || positive.sat, count }
}

pub(crate) fn count(
    p: &Problem, scope: &[usize], bits: u32,
) -> (Option<Outcome>, CutsetStats) {
    let mut stats = CutsetStats::default();
    let t0 = Instant::now();
    if p.n == 0 || p.n > MAX_SOURCE_VARS || p.weights.len() < p.n
        || scope.is_empty() || scope.len() > MAX_SCOPE
        || p.clauses.len() > MAX_CLAUSES
        || p.clauses.iter().any(|clause| clause.len() > MAX_CLAUSE_WIDTH) {
        reject(&mut stats, "cutset_source_envelope");
        return (None, stats);
    }
    let mut remaining = scope.to_vec();
    remaining.sort_unstable();
    if remaining.iter().any(|&v| v == 0 || v > p.n)
        || remaining.windows(2).any(|w| w[0] == w[1]) {
        reject(&mut stats, "cutset_invalid_scope");
        return (None, stats);
    }
    let mut present = vec![false; p.n + 1];
    for &v in &remaining { present[v] = true; }
    if p.clauses.iter().flat_map(|clause| clause.iter())
        .any(|lit| {
            let v = lit.unsigned_abs() as usize;
            v == 0 || v > p.n || !present[v]
        }) {
        reject(&mut stats, "cutset_clause_scope");
        return (None, stats);
    }
    stats.eligible = true;
    let original = factor_minfill_order::plan(&p.clauses, &remaining, DENSE_BUCKET_CAP);
    stats.source_bucket = original.max_bucket;
    if !original.capped {
        // Source is already in the existing proven dense Factor-VE
        // envelope: no branching, same original source and weights.
        let (out, ve) = factor_variable_elimination::count_conditioned_minfill(
            p, &remaining, &p.clauses, bits,
        );
        stats.max_branch_bucket = ve.width;
        stats.max_branch_products = ve.products;
        stats.preparation_ns = t0.elapsed().as_nanos();
        if out.is_none() { reject(&mut stats, "cutset_dense_work_cap"); }
        return (out, stats);
    }
    if original.first_over_bucket_vars.is_empty() {
        reject(&mut stats, "cutset_no_bottleneck_witness");
        stats.preparation_ns = t0.elapsed().as_nanos();
        return (None, stats);
    }
    stats.witness_candidates = original.first_over_bucket_vars.len();
    let mut occurrence = vec![0_usize; p.n + 1];
    for clause in &p.clauses {
        for &lit in clause {
            occurrence[lit.unsigned_abs() as usize] += 1;
        }
    }
    // First try the offending primal bucket's actual vertices, then at
    // most the highest-occurrence source variables as a controlled backup.
    let mut candidates = original.first_over_bucket_vars;
    candidates.sort_by_key(|&v| (std::cmp::Reverse(occurrence[v]), v));
    candidates.truncate(MAX_CANDIDATES);
    let mut by_frequency = remaining.clone();
    by_frequency.sort_by_key(|&v| (std::cmp::Reverse(occurrence[v]), v));
    for v in by_frequency {
        if candidates.len() >= MAX_CANDIDATES { break; }
        if !candidates.contains(&v) { candidates.push(v); }
    }

    for var in candidates {
        if stats.branch_solves >= MAX_BRANCH_SOLVES {
            reject(&mut stats, "cutset_branch_work");
            break;
        }
        stats.attempted += 1;
        let rest: Vec<usize> = remaining.iter().copied().filter(|&v| v != var).collect();
        let off_clauses = conditional_clauses(&p.clauses, var, false);
        let on_clauses = conditional_clauses(&p.clauses, var, true);
        stats.planned_branches += 2;
        let off_width = branch_plan(&off_clauses, &rest);
        let on_width = branch_plan(&on_clauses, &rest);
        if off_width.is_none() || on_width.is_none() { continue; }
        let bw = off_width.unwrap().max(on_width.unwrap());
        stats.max_branch_bucket = stats.max_branch_bucket.max(bw);
        stats.feasible_pairs += 1;
        stats.branch_solves += 2;
        let (off, off_products) = branch_count(p, &rest, &off_clauses, bits);
        stats.max_branch_products = stats.max_branch_products.max(off_products);
        let Some(off) = off else { continue };
        let (on, on_products) = branch_count(p, &rest, &on_clauses, bits);
        stats.max_branch_products = stats.max_branch_products.max(on_products);
        let Some(on) = on else { continue };
        stats.completed_var = var;
        stats.preparation_ns = t0.elapsed().as_nanos();
        return (Some(combine(p, var, &off, &on, bits)), stats);
    }

    if !stats.capped { reject(&mut stats, "cutset_no_both_branches"); }
    stats.preparation_ns = t0.elapsed().as_nanos();
    (None, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, trail_engine::TrailEngine};

    fn brute(p: &Problem, bits: u32) -> Outcome {
        assert!(p.n <= 12);
        let mut sat = false;
        let mut count = C::zero(bits);
        for mask in 0..(1_usize << p.n) {
            if p.clauses.iter().any(|clause| clause.iter().all(|lit| {
                let bit = ((mask >> (lit.unsigned_abs() as usize - 1)) & 1) != 0;
                bit != (*lit > 0)
            })) { continue; }
            sat = true;
            let mut product = C::one(bits);
            for i in 0..p.n {
                let weight = if ((mask >> i) & 1) == 1 {
                    &p.weights[i].positive
                } else {
                    &p.weights[i].negative
                };
                product = product.mul(weight, bits);
            }
            count = count.add(&product, bits);
        }
        Outcome { sat, count }
    }

    fn check(raw: &str, expect_condition: bool) {
        for bits in [100, 333] {
            let p = parse_emitted(raw, bits).unwrap();
            let scope: Vec<usize> = (1..=p.n).collect();
            let oracle = brute(&p, bits);
            let (answer, stats) = count(&p, &scope, bits);
            assert!(answer.is_some(), "{stats:?}");
            if expect_condition {
                assert!(stats.completed_var > 0, "{stats:?}");
                assert!(stats.source_bucket > 11, "{stats:?}");
                assert!(stats.feasible_pairs > 0, "{stats:?}");
            }
            let actual = answer.unwrap();
            assert_eq!(actual.sat, oracle.sat);
            assert_eq!(actual.count.re, oracle.count.re);
            assert_eq!(actual.count.im, oracle.count.im);
            let mut trail = TrailEngine::new(&p, bits, 128, 100_000);
            let reference = trail.count(scope, 0).unwrap();
            assert_eq!(actual.sat, reference.sat);
            assert_eq!(actual.count.re, reference.count.re);
            assert_eq!(actual.count.im, reference.count.im);
        }
    }

    #[test]
    fn conditions_a_real_overwidth_twelve_variable_primal_clique() {
        let all_positive = (1..=12).map(|v| v.to_string())
            .collect::<Vec<_>>().join(" ");
        let all_negative = (1..=12).map(|v| format!("-{v}"))
            .collect::<Vec<_>>().join(" ");
        let raw = format!(
            "p cnf 12 2\nc t wmc\n\
             c p weight 1 -0.5 0.25 0\nc p weight -1 0.75 -0.5 0\n\
             c p weight 2 0.5 0.25 0\nc p weight -2 -0.25 0.125 0\n\
             {all_positive} 0\n{all_negative} 0\n",
        );
        check(&raw, true);
    }

    #[test]
    fn already_feasible_dense_scope_keeps_weights_and_sat_zero() {
        check("p cnf 4 2\nc t wmc\n\
c p weight 1 1 0 0\nc p weight -1 -1 0 0\n\
1 2 0\n-2 3 0\n", false);
        check("p cnf 2 2\nc t wmc\n1 0\n-1 0\n", false);
        check("p cnf 4 1\nc t wmc\n0\n", false);
    }

    #[test]
    fn source_scope_rejection_never_uses_partial_branch_result() {
        let p = parse_emitted("p cnf 4 1\nc t wmc\n1 2 0\n", 100).unwrap();
        let (answer, stats) = count(&p, &[1, 1], 100);
        assert!(answer.is_none() && stats.capped);
        assert_eq!(stats.reason, "cutset_invalid_scope");
        let mut trail = TrailEngine::new(&p, 100, 128, 100_000);
        let answer = trail.count(vec![1, 2, 3, 4], 0).unwrap();
        let oracle = brute(&p, 100);
        assert_eq!(answer.sat, oracle.sat);
        assert_eq!(answer.count.re, oracle.count.re);
        assert_eq!(answer.count.im, oracle.count.im);
    }
}
