//! Research-only bounded source-CNF Shannon cutset + dense Factor-VE.
//!
//! The native CLI receives the identical original QuPRS weighted CNF.
//! The solver conditions one original surviving variable internally, then
//! counts both cofactors with the pre-existing width-11 dense Factor-VE.
//! Ownership of the conditioned variable's two signed MPFR weights is explicit.
//! Any rejection abandons both partial branches and runs unchanged Trail.
use crate::{factor_minfill_order, factor_variable_elimination, C, Clause, Outcome, Problem};
use std::collections::HashMap;

const MAX_SCOPE: usize = 320;
const MAX_CLAUSES: usize = 3000;
const MAX_ROOT_BUCKET: usize = 16;
const MAX_CANDIDATES: usize = 24;
const DENSE_MAX_BAG: usize = 11;

#[derive(Default, Clone, Debug)]
pub(crate) struct CutsetStats {
    pub(crate) eligible: bool,
    pub(crate) capped: bool,
    pub(crate) reason: &'static str,
    pub(crate) original_bucket: usize,
    pub(crate) candidate_probes: usize,
    pub(crate) feasible_candidates: usize,
    pub(crate) selected_variable: usize,
    pub(crate) positive_bucket: usize,
    pub(crate) negative_bucket: usize,
    pub(crate) branches_completed: usize,
    pub(crate) branch_products: usize,
    pub(crate) source_scope: usize,
    pub(crate) source_clauses: usize,
}

fn reject(stats: &mut CutsetStats, reason: &'static str) -> Option<Outcome> {
    stats.capped = true;
    stats.reason = reason;
    None
}

/// The explicit Shannon cofactor is an internal, logically exact
/// transformation. It does not regenerate or alter the input source CNF.
/// Remove clauses made true; remove only the false conditioned literal.
fn cofactor(clauses: &[Clause], variable: usize, positive: bool) -> Vec<Clause> {
    let literal = if positive { variable as i32 } else { -(variable as i32) };
    let mut result = Vec::with_capacity(clauses.len());
    for clause in clauses {
        if clause.contains(&literal) { continue; }
        let mut out = Vec::with_capacity(clause.len());
        for &lit in clause {
            if lit != -literal { out.push(lit); }
        }
        result.push(out);
    }
    result
}

fn rank_candidates(clauses: &[Clause], scope: &[usize]) -> Vec<usize> {
    let mut index: HashMap<usize, usize> = HashMap::with_capacity(scope.len());
    for (i, &v) in scope.iter().enumerate() {
        index.insert(v, i);
    }
    let mut degrees = vec![0_usize; scope.len()];
    let mut neighbours = vec![vec![false; scope.len()]; scope.len()];
    for clause in clauses {
        let mut vars: Vec<usize> = clause.iter()
            .filter_map(|lit| index.get(&(lit.unsigned_abs() as usize)).copied())
            .collect();
        vars.sort_unstable();
        vars.dedup();
        for (pos, &a) in vars.iter().enumerate() {
            for &b in &vars[pos + 1..] {
                if !neighbours[a][b] {
                    neighbours[a][b] = true;
                    neighbours[b][a] = true;
                    degrees[a] += 1;
                    degrees[b] += 1;
                }
            }
        }
    }
    let mut ordered = scope.to_vec();
    ordered.sort_unstable_by_key(|v| {
        let degree = degrees[index[v]];
        (std::cmp::Reverse(degree), *v)
    });
    ordered.truncate(MAX_CANDIDATES);
    ordered
}

pub(crate) fn count(p: &Problem, scope: &[usize], bits: u32)
    -> (Option<Outcome>, CutsetStats) {
    let mut stats = CutsetStats {
        source_scope: scope.len(),
        source_clauses: p.clauses.len(),
        ..CutsetStats::default()
    };
    let answer = attempt(p, scope, bits, &mut stats);
    (answer, stats)
}

fn attempt(p: &Problem, scope: &[usize], bits: u32, stats: &mut CutsetStats)
    -> Option<Outcome> {
    if p.n == 0 || p.n > 4096 || scope.len() < 2 ||
        scope.len() > MAX_SCOPE || p.clauses.len() > MAX_CLAUSES ||
        p.weights.len() < p.n {
        return reject(stats, "cutset_source_envelope");
    }
    let mut present = vec![false; p.n + 1];
    for &v in scope {
        if v == 0 || v > p.n || present[v] {
            return reject(stats, "cutset_invalid_scope");
        }
        present[v] = true;
    }
    for clause in &p.clauses {
        if clause.iter().any(|lit| {
            let v = lit.unsigned_abs() as usize;
            v == 0 || v > p.n || !present[v]
        }) {
            return reject(stats, "cutset_outside_scope");
        }
    }
    stats.eligible = true;
    let root = factor_minfill_order::plan(&p.clauses, scope, DENSE_MAX_BAG);
    stats.original_bucket = root.max_bucket;
    if !root.capped {
        // Dense VE already fits its structural guard; use existing faster arm.
        return reject(stats, "cutset_not_needed");
    }
    if root.max_bucket > MAX_ROOT_BUCKET {
        return reject(stats, "cutset_root_width");
    }

    let mut selected: Option<(usize, Vec<usize>, Vec<Clause>, Vec<Clause>, usize, usize)> = None;
    for variable in rank_candidates(&p.clauses, scope) {
        stats.candidate_probes += 1;
        let remaining: Vec<usize> = scope.iter().copied().filter(|&v| v != variable).collect();
        let negative = cofactor(&p.clauses, variable, false);
        let positive = cofactor(&p.clauses, variable, true);
        let lo = factor_minfill_order::plan(&negative, &remaining, DENSE_MAX_BAG);
        let hi = factor_minfill_order::plan(&positive, &remaining, DENSE_MAX_BAG);
        if !lo.capped && !hi.capped {
            stats.feasible_candidates += 1;
            let score = (
                lo.max_bucket.max(hi.max_bucket),
                lo.fill_edges + hi.fill_edges,
                variable,
            );
            if selected.as_ref().is_none_or(|(old, _, _, _, low_bucket, high_bucket)| {
                score < (low_bucket.max(high_bucket), usize::MAX, *old)
            }) {
                selected = Some((
                    variable, remaining, negative, positive,
                    lo.max_bucket, hi.max_bucket,
                ));
            }
            // A verified single-variable cutset suffices. The first admissible
            // high-degree variable avoids costly per-candidate dense attempts.
            break;
        }
    }

    let Some((variable, remaining, negative, positive, low_width, high_width)) = selected
    else { return reject(stats, "cutset_no_width11_variable") };
    stats.selected_variable = variable;
    stats.negative_bucket = low_width;
    stats.positive_bucket = high_width;

    // Use identical pinned original weights for all unconditioned variables.
    // Dense solver will not include the fixed variable because scope excludes it.
    let lower_p = Problem {
        n: p.n, clauses: negative, weights: p.weights.clone(),
    };
    let upper_p = Problem {
        n: p.n, clauses: positive, weights: p.weights.clone(),
    };
    let (lo, lo_stats) =
        factor_variable_elimination::count_scope_probe_minfill(&lower_p, &remaining, bits);
    if lo.is_none() {
        return reject(stats, "cutset_lower_dense_cap");
    }
    stats.branches_completed = 1;
    stats.branch_products += lo_stats.products;
    let (hi, hi_stats) =
        factor_variable_elimination::count_scope_probe_minfill(&upper_p, &remaining, bits);
    if hi.is_none() {
        return reject(stats, "cutset_upper_dense_cap");
    }
    stats.branches_completed = 2;
    stats.branch_products += hi_stats.products;
    let lo = lo.unwrap();
    let hi = hi.unwrap();

    // SAT is existential, independent of signed cancellation.
    // Both complex branches retain exactly one conditioned-variable weight.
    let mut total = C::zero(bits);
    if lo.sat {
        total = total.add(&p.weights[variable - 1].negative.mul(&lo.count, bits), bits);
    }
    if hi.sat {
        total = total.add(&p.weights[variable - 1].positive.mul(&hi.count, bits), bits);
    }
    Some(Outcome { sat: lo.sat || hi.sat, count: total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, trail_engine::TrailEngine};

    fn independent_brute(p: &Problem, bits: u32) -> Outcome {
        assert!(p.n <= 12);
        let mut sat = false;
        let mut total = C::zero(bits);
        for mask in 0..(1_usize << p.n) {
            if p.clauses.iter().any(|c| !c.iter().any(|&lit| {
                let var = lit.unsigned_abs() as usize - 1;
                (((mask >> var) & 1) != 0) == (lit > 0)
            })) {
                continue;
            }
            sat = true;
            let mut weighted = C::one(bits);
            for v in 0..p.n {
                weighted = weighted.mul(if mask & (1 << v) != 0 {
                    &p.weights[v].positive
                } else { &p.weights[v].negative }, bits);
            }
            total = total.add(&weighted, bits);
        }
        Outcome { sat, count: total }
    }

    fn clique12(extra: &str) -> String {
        let mut s = format!("p cnf 12 66\nc t wmc\n{extra}");
        for a in 1..=12 {
            for b in a + 1..=12 {
                s.push_str(&format!("{a} {b} 0\n"));
            }
        }
        s
    }

    #[test]
    fn twelve_clique_needs_cutset_and_matches_independent_oracle() {
        for bits in [100, 333] {
            let p = parse_emitted(&clique12(
                "c p weight 1 0.5 0.25 0\n\
c p weight -1 -0.25 -0.5 0\n\
c p weight 7 -0.5 0.5 0\n\
c p weight -7 0.25 -0.25 0\n",
            ), bits).unwrap();
            let scope: Vec<usize> = (1..=12).collect();
            let (dense, dense_stats) =
                factor_variable_elimination::count_scope_probe_minfill(&p, &scope, bits);
            assert!(dense.is_none() && dense_stats.capped);
            let (cut, stats) = count(&p, &scope, bits);
            assert!(cut.is_some() && !stats.capped, "{stats:?}");
            assert_eq!(stats.branches_completed, 2);
            assert_eq!(stats.original_bucket, 12);
            let got = cut.unwrap();
            let exact = independent_brute(&p, bits);
            assert_eq!(got.sat, exact.sat);
            assert_eq!(got.count.re, exact.count.re);
            assert_eq!(got.count.im, exact.count.im);
            let mut trail = TrailEngine::new(&p, bits, 256, 100_000);
            let baseline = trail.count(scope, 0).unwrap();
            assert_eq!(got.sat, baseline.sat);
            assert_eq!(got.count.re, baseline.count.re);
            assert_eq!(got.count.im, baseline.count.im);
        }
    }

    #[test]
    fn structural_rejection_does_not_change_original_trail() {
        let p = parse_emitted("p cnf 3 1\nc t wmc\n1 -2 0\n", 100).unwrap();
        let scope = vec![1, 2, 3];
        let (cut, stats) = count(&p, &scope, 100);
        assert!(cut.is_none() && stats.capped && stats.reason == "cutset_not_needed");
        let mut trail = TrailEngine::new(&p, 100, 128, 100_000);
        let old = trail.count(scope, 0).unwrap();
        let brute = independent_brute(&p, 100);
        assert_eq!(old.sat, brute.sat);
        assert_eq!(old.count.re, brute.count.re);
        assert_eq!(old.count.im, brute.count.im);
    }
}
