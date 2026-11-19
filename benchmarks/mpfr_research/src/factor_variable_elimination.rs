//! Exact bounded signed-complex sum-product variable elimination.
//! Retains SAT separately from WMC: zero complex weight is not UNSAT.
//! Input clauses, weights and scope are never mutated. On any bound
//! failure the caller uses the unchanged original Trail solver.
use crate::{factor_minfill_order, C, Clause, Outcome, Problem};
use std::time::Instant;

const MAX_VARS: usize = 4096;
const MAX_SCOPE: usize = 256;
// Research-only scope admission probe; all exponential factor limits unchanged.
pub(crate) const PROBE_SCOPE: usize = 320;
const MAX_CLAUSES: usize = 3000;
const MAX_CLAUSE_WIDTH: usize = 12;
const MAX_BAG: usize = 11;
// Research-only bounded wider bag admission. Other exponential budgets
// remain fixed, with no production/default expansion.
const PROBE_BAG: usize = 14;
const MAX_CELLS: usize = 150_000;
const MAX_PRODUCTS: usize = 600_000;

struct Factor {
    vars: Vec<usize>,
    values: Vec<Outcome>,
}

#[derive(Default, Clone, Debug)]
pub(crate) struct VeStats {
    pub(crate) eligible: bool,
    pub(crate) capped: bool,
    pub(crate) factors: usize,
    pub(crate) eliminated: usize,
    pub(crate) width: usize,
    pub(crate) cells: usize,
    pub(crate) products: usize,
    pub(crate) zero_sat: usize,
    pub(crate) cap_reason: &'static str,
    pub(crate) source_vars: usize,
    pub(crate) scope_vars: usize,
    pub(crate) source_clauses: usize,
    pub(crate) source_max_clause_width: usize,
    pub(crate) minfill_enabled: bool,
    pub(crate) minfill_plan_width: usize,
    pub(crate) minfill_fill_edges: usize,
    pub(crate) minfill_candidates: u64,
    pub(crate) minfill_plan_ns: u128,
    pub(crate) preflight_enabled: bool,
    pub(crate) preflight_products_upper: usize,
    pub(crate) preflight_cells_upper: usize,
}

fn reject(mut stats: VeStats, reason: &'static str) -> (Option<Outcome>, VeStats) {
    stats.capped = true;
    stats.cap_reason = reason;
    (None, stats)
}

fn clause_factor(
    literals: &[i32], present: &[bool], bits: u32, stats: &mut VeStats,
) -> Option<Factor> {
    let mut vars: Vec<usize> = literals.iter()
        .map(|lit| lit.unsigned_abs() as usize).collect();
    if vars.iter().any(|&v| v == 0 || v >= present.len() || !present[v]) {
        return None;
    }
    vars.sort_unstable();
    vars.dedup();
    if vars.len() > MAX_CLAUSE_WIDTH { return None; }
    let cells = 1_usize << vars.len();
    if stats.cells.saturating_add(cells) > MAX_CELLS { return None; }
    stats.cells += cells;
    let mut values = Vec::with_capacity(cells);
    for mask in 0..cells {
        let sat = literals.iter().any(|&lit| {
            let pos = vars.binary_search(&(lit.unsigned_abs() as usize)).unwrap();
            (((mask >> pos) & 1) == 1) == (lit > 0)
        });
        values.push(Outcome {
            sat,
            count: if sat { C::one(bits) } else { C::zero(bits) },
        });
    }
    Some(Factor { vars, values })
}

fn selected_bucket(factors: &[Factor], remaining: &[usize]) -> (usize, Vec<usize>, Vec<usize>) {
    type BucketCandidate = (usize, usize, usize, Vec<usize>, Vec<usize>);
    let mut selected: Option<BucketCandidate> = None;
    for &v in remaining {
        let mut incident = Vec::new();
        let mut union = Vec::new();
        for (i, factor) in factors.iter().enumerate() {
            if factor.vars.binary_search(&v).is_ok() {
                incident.push(i);
                union.extend(&factor.vars);
            }
        }
        union.sort_unstable();
        union.dedup();
        let priority = (union.len(), incident.len(), v);
        if selected.as_ref().is_none_or(|s| priority < (s.0, s.1, s.2)) {
            selected = Some((priority.0, priority.1, priority.2, incident, union));
        }
    }
    let (_, _, v, incident, union) = selected.unwrap();
    (v, incident, union)
}

/// A scope-only upper bound on the sum-product loop work. It never reads
/// weights or builds MPFR tables; signed/zero weights cannot invalidate it.
/// UNSAT factor entries may short-circuit, so this is not a lower bound.
fn projected_minfill_work(
    clauses: &[Clause], scope: &[usize], order: &[usize],
) -> (usize, usize) {
    let mut scopes: Vec<Vec<usize>> = clauses
        .iter()
        .map(|clause| {
            let mut vars: Vec<usize> = clause
                .iter()
                .map(|literal| literal.unsigned_abs() as usize)
                .collect();
            vars.sort_unstable();
            vars.dedup();
            vars
        })
        .collect();
    let mut cells = scopes.iter().fold(0_usize, |total, vars| {
        total.saturating_add(1_usize << vars.len())
    });
    cells = cells.saturating_add(scope.len().saturating_mul(2));
    for &variable in scope {
        scopes.push(vec![variable]);
    }
    let mut products = 0_usize;
    for &variable in order {
        let mut union = Vec::new();
        let mut affected = 0_usize;
        for vars in &scopes {
            if vars.binary_search(&variable).is_ok() {
                affected += 1;
                union.extend(vars);
            }
        }
        union.sort_unstable();
        union.dedup();
        if union.is_empty() || union.len() > PROBE_BAG {
            return (usize::MAX, usize::MAX);
        }
        let rest: Vec<usize> = union.into_iter()
            .filter(|&var| var != variable)
            .collect();
        let n_cells = 1_usize << rest.len();
        cells = cells.saturating_add(n_cells);
        products = products.saturating_add(
            affected.saturating_mul(n_cells).saturating_mul(2),
        );
        scopes.retain(|vars| vars.binary_search(&variable).is_err());
        scopes.push(rest);
    }
    (products, cells)
}

fn value_at(
    factors: &[&Factor], rest: &[usize], variable: usize, positive: bool,
    assignment: usize, bits: u32, stats: &mut VeStats,
) -> Option<Outcome> {
    let mut count = C::one(bits);
    for factor in factors {
        stats.products += 1;
        if stats.products > MAX_PRODUCTS { return None; }
        let mut index = 0_usize;
        for (bit, &v) in factor.vars.iter().enumerate() {
            let on = if v == variable {
                positive
            } else {
                let position = rest.binary_search(&v).unwrap();
                ((assignment >> position) & 1) == 1
            };
            index |= usize::from(on) << bit;
        }
        let entry = &factor.values[index];
        if !entry.sat { return Some(Outcome::unsat(bits)); }
        count = count.mul(&entry.count, bits);
    }
    Some(Outcome { sat: true, count })
}

pub(crate) fn count(
    p: &Problem, scope: &[usize], bits: u32,
) -> (Option<Outcome>, VeStats) {
    count_clauses(p, scope, &p.clauses, bits)
}

/// Controlled envelope experiment: ONLY the pre-admission surviving-variable
/// limit differs. The width, product, factor-cell and source-clause limits
/// are exactly the same as baseline Factor-VE.
pub(crate) fn count_scope_probe(
    p: &Problem, scope: &[usize], bits: u32,
) -> (Option<Outcome>, VeStats) {
    count_clauses_with_limit(p, scope, &p.clauses, bits, PROBE_SCOPE, false, MAX_BAG, false)
}

/// Research-only alternative: same 320 source-scope admission and exactly
/// the same exponential work caps, but use bounded primal min-fill ordering.
pub(crate) fn count_scope_probe_minfill(
    p: &Problem, scope: &[usize], bits: u32,
) -> (Option<Outcome>, VeStats) {
    count_clauses_with_limit(p, scope, &p.clauses, bits, PROBE_SCOPE, true, MAX_BAG, false)
}

/// Research-only scope 320 + width 14 min-fill VE probe. Hard cells/products
/// and original-circuit signed weights remain unchanged; any exceeded budget
/// returns None and the caller uses the same original weighted Trail.
pub(crate) fn count_scope_probe_minfill_bag14(
    p: &Problem, scope: &[usize], bits: u32,
) -> (Option<Outcome>, VeStats) {
    count_clauses_with_limit(p, scope, &p.clauses, bits, PROBE_SCOPE, true, PROBE_BAG, false)
}

/// Research-only structural admission before expensive signed-complex tables.
/// A rejected estimate always returns to the unchanged original DPLL path.
pub(crate) fn count_scope_probe_minfill_bag14_preflight(
    p: &Problem, scope: &[usize], bits: u32,
) -> (Option<Outcome>, VeStats) {
    count_clauses_with_limit(p, scope, &p.clauses, bits, PROBE_SCOPE, true, PROBE_BAG, true)
}

/// Count a *live*, already-cofactored residual CNF with unchanged original
/// source literal weights and an exact disjoint residual variable scope.
/// Callers must separately multiply forced-prefix and free-variable weights.
pub(crate) fn count_clauses(
    p: &Problem, scope: &[usize], clauses: &[Clause], bits: u32,
) -> (Option<Outcome>, VeStats) {
    count_clauses_with_limit(p, scope, clauses, bits, MAX_SCOPE, false, MAX_BAG, false)
}

fn selected_bucket_for_variable(
    factors: &[Factor], variable: usize,
) -> (usize, Vec<usize>, Vec<usize>) {
    let mut incident = Vec::new();
    let mut union = Vec::new();
    for (index, factor) in factors.iter().enumerate() {
        if factor.vars.binary_search(&variable).is_ok() {
            incident.push(index);
            union.extend(&factor.vars);
        }
    }
    union.sort_unstable();
    union.dedup();
    (variable, incident, union)
}

// Legacy bounded VE entrypoint carries four independent research-policy knobs.
// The checked opt-in controls are intentionally explicit; no production ABI changes.
#[allow(clippy::too_many_arguments)]
fn count_clauses_with_limit(
    p: &Problem, scope: &[usize], clauses: &[Clause], bits: u32,
    scope_cap: usize, minfill: bool, bag_cap: usize, preflight: bool,
) -> (Option<Outcome>, VeStats) {
    let mut stats = VeStats {
        preflight_enabled: preflight,
        source_vars: p.n,
        scope_vars: scope.len(),
        source_clauses: clauses.len(),
        source_max_clause_width: clauses.iter().map(Vec::len).max().unwrap_or(0),
        ..VeStats::default()
    };
    if p.n == 0 { return reject(stats, "empty_source_variables"); }
    if p.n > MAX_VARS { return reject(stats, "source_variables"); }
    if scope.len() > scope_cap { return reject(stats, "source_scope"); }
    if clauses.len() > MAX_CLAUSES { return reject(stats, "source_clauses"); }
    if p.weights.len() < p.n { return reject(stats, "missing_weights"); }
    let mut present = vec![false; p.n + 1];
    for &v in scope {
        if v == 0 || v > p.n || present[v] { return reject(stats, "invalid_scope"); }
        present[v] = true;
    }
    let early_order = if preflight {
        let start = Instant::now();
        let plan = factor_minfill_order::plan(clauses, scope, bag_cap);
        stats.minfill_plan_ns = start.elapsed().as_nanos();
        stats.minfill_enabled = true;
        stats.minfill_plan_width = plan.max_bucket;
        stats.minfill_fill_edges = plan.fill_edges;
        stats.minfill_candidates = plan.candidate_evaluations;
        if plan.capped {
            stats.width = plan.max_bucket;
            return reject(stats, "bucket_width");
        }
        let (products, cells) = projected_minfill_work(clauses, scope, &plan.order);
        stats.preflight_products_upper = products;
        stats.preflight_cells_upper = cells;
        stats.eligible = true;
        if products > MAX_PRODUCTS {
            return reject(stats, "preflight_products");
        }
        if cells > MAX_CELLS {
            return reject(stats, "preflight_cells");
        }
        Some(plan.order)
    } else {
        None
    };
    let mut factors = Vec::with_capacity(clauses.len() + scope.len());
    for clause in clauses {
        let mut vars = clause.iter().map(|lit| lit.unsigned_abs() as usize).collect::<Vec<_>>();
        if vars.iter().any(|&var| var == 0 || var > p.n || !present[var]) {
            return reject(stats, "clause_outside_scope");
        }
        vars.sort_unstable();
        vars.dedup();
        if vars.len() > MAX_CLAUSE_WIDTH { return reject(stats, "source_clause_width"); }
        if stats.cells.saturating_add(1_usize << vars.len()) > MAX_CELLS {
            return reject(stats, "source_factor_cells");
        }
        let Some(factor) = clause_factor(clause, &present, bits, &mut stats) else {
            return reject(stats, "source_factor_invalid");
        };
        factors.push(factor);
    }
    for &v in scope {
        if stats.cells.saturating_add(2) > MAX_CELLS { return reject(stats, "source_unary_cells"); }
        stats.cells += 2;
        factors.push(Factor {
            vars: vec![v],
            values: vec![
                Outcome { sat: true, count: p.weights[v - 1].negative.clone() },
                Outcome { sat: true, count: p.weights[v - 1].positive.clone() },
            ],
        });
    }
    stats.factors = factors.len();
    stats.eligible = true;
    stats.minfill_enabled = minfill;
    let minfill_order = if early_order.is_some() {
        early_order
    } else if minfill {
        let start = Instant::now();
        let plan = factor_minfill_order::plan(clauses, scope, bag_cap);
        stats.minfill_plan_ns = start.elapsed().as_nanos();
        stats.minfill_plan_width = plan.max_bucket;
        stats.minfill_fill_edges = plan.fill_edges;
        stats.minfill_candidates = plan.candidate_evaluations;
        if plan.capped {
            stats.width = plan.max_bucket;
            return reject(stats, "bucket_width");
        }
        Some(plan.order)
    } else {
        None
    };
    let mut remaining = scope.to_vec();
    remaining.sort_unstable();
    while !remaining.is_empty() {
        let (variable, incident, union) = if let Some(order) = &minfill_order {
            selected_bucket_for_variable(&factors, order[stats.eliminated])
        } else {
            selected_bucket(&factors, &remaining)
        };
        stats.width = stats.width.max(union.len());
        if union.len() > bag_cap || union.is_empty() {
            return reject(stats, "bucket_width");
        }
        let rest: Vec<usize> = union.into_iter().filter(|&v| v != variable).collect();
        let n_cells = 1_usize << rest.len();
        if stats.cells.saturating_add(n_cells) > MAX_CELLS { return reject(stats, "bucket_cells"); }
        stats.cells += n_cells;
        let affected: Vec<&Factor> = incident.iter().map(|&i| &factors[i]).collect();
        let mut new_values = Vec::with_capacity(n_cells);
        for assignment in 0..n_cells {
            let Some(negative) = value_at(
                &affected, &rest, variable, false, assignment, bits, &mut stats
            ) else { return reject(stats, "bucket_products") };
            let Some(positive) = value_at(
                &affected, &rest, variable, true, assignment, bits, &mut stats
            ) else { return reject(stats, "bucket_products") };
            let sat = negative.sat || positive.sat;
            let mut weighted = C::zero(bits);
            if negative.sat { weighted = weighted.add(&negative.count, bits); }
            if positive.sat { weighted = weighted.add(&positive.count, bits); }
            if sat && weighted.re == 0 && weighted.im == 0 { stats.zero_sat += 1; }
            new_values.push(Outcome { sat, count: weighted });
        }
        let mut affected_flags = vec![false; factors.len()];
        for i in incident { affected_flags[i] = true; }
        let mut index = 0;
        factors.retain(|_| {
            let keep = !affected_flags[index];
            index += 1;
            keep
        });
        factors.push(Factor { vars: rest, values: new_values });
        remaining.retain(|&v| v != variable);
        stats.eliminated += 1;
    }
    let mut outcome = Outcome { sat: true, count: C::one(bits) };
    for factor in factors {
        if !factor.vars.is_empty() || factor.values.len() != 1 {
            return reject(stats, "non_scalar_final_factor");
        }
        let scalar = &factor.values[0];
        if !scalar.sat {
            outcome = Outcome::unsat(bits);
            break;
        }
        outcome.count = outcome.count.mul(&scalar.count, bits);
    }
    (Some(outcome), stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, trail_engine::TrailEngine};

    fn independent_brute(p: &Problem, bits: u32) -> Outcome {
        let mut sat = false;
        let mut total = C::zero(bits);
        for mask in 0..(1_usize << p.n) {
            if !p.clauses.iter().all(|clause| clause.iter().any(|&lit| {
                let var = lit.unsigned_abs() as usize - 1;
                (((mask >> var) & 1) == 1) == (lit > 0)
            })) { continue; }
            sat = true;
            let mut value = C::one(bits);
            for v in 1..=p.n {
                value = value.mul(
                    if ((mask >> (v - 1)) & 1) == 1 {
                        &p.weights[v - 1].positive
                    } else {
                        &p.weights[v - 1].negative
                    }, bits
                );
            }
            total = total.add(&value, bits);
        }
        Outcome { sat, count: total }
    }

    fn check(text: &str) {
        for bits in [100, 333] {
            let p = parse_emitted(text, bits).unwrap();
            let scope: Vec<usize> = (1..=p.n).collect();
            let exact = independent_brute(&p, bits);
            let (candidate, stats) = count(&p, &scope, bits);
            assert!(stats.eligible && !stats.capped, "{stats:?}");
            let result = candidate.unwrap();
            let (minfill, fill_stats) = count_scope_probe_minfill(&p, &scope, bits);
            assert!(fill_stats.eligible && !fill_stats.capped, "{fill_stats:?}");
            let minfill = minfill.unwrap();
            let (preflight, admission) =
                count_scope_probe_minfill_bag14_preflight(&p, &scope, bits);
            assert!(admission.preflight_enabled);
            assert!(!admission.capped, "{admission:?}");
            assert!(admission.preflight_products_upper >= admission.products);
            let preflight = preflight.unwrap();
            assert_eq!(preflight.sat, exact.sat);
            assert_eq!(preflight.count.re, exact.count.re);
            assert_eq!(preflight.count.im, exact.count.im);
            assert_eq!(minfill.sat, exact.sat);
            assert_eq!(minfill.count.re, exact.count.re);
            assert_eq!(minfill.count.im, exact.count.im);
            assert_eq!(result.sat, exact.sat);
            assert_eq!(result.count.re, exact.count.re);
            assert_eq!(result.count.im, exact.count.im);
            let mut trail = TrailEngine::new(&p, bits, 128, 100_000);
            let old = trail.count(scope, 0).unwrap();
            assert_eq!(result.sat, old.sat);
            assert_eq!(result.count.re, old.count.re);
            assert_eq!(result.count.im, old.count.im);
        }
    }


    #[test]
    fn wider_bag14_probe_admits_exact_source_scope_without_changing_default() {
        // A single twelve-literal original source clause induces a width-12
        // bucket even though all weights are 1. Default MAX_BAG=11 must
        // reject it; the separately opt-in bounded bag14 calculates WMC
        // = 2^12 - 1 exactly, including at 333-bit MPFR precision.
        let input = "p cnf 12 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 11 12 0\n";
        for bits in [100_u32, 333] {
            let p = parse_emitted(input, bits).unwrap();
            let scope = (1..=12).collect::<Vec<_>>();
            let (control, control_stats) = count_scope_probe_minfill(&p, &scope, bits);
            assert!(control.is_none());
            assert_eq!(control_stats.cap_reason, "bucket_width");
            let (trial, stats) = count_scope_probe_minfill_bag14(&p, &scope, bits);
            assert!(stats.eligible && !stats.capped, "{stats:?}");
            assert!(stats.width <= PROBE_BAG);
            assert!(stats.cells <= MAX_CELLS);
            assert!(stats.products <= MAX_PRODUCTS);
            let actual = trial.unwrap();
            assert!(actual.sat);
            let expected = C::decimal(bits, "4095", "0").unwrap();
            assert_eq!(actual.count.re, expected.re);
            assert_eq!(actual.count.im, expected.im);
        }
    }

    #[test]
    fn signed_complex_connected_cnf_is_exact() {
        check("p cnf 4 4\nc t wmc\n\
c p weight 1 -0.5 0.25 0\nc p weight -1 0.75 -0.5 0\n\
c p weight 2 0.25 0.5 0\nc p weight -2 -0.5 0.25 0\n\
1 -2 0\n2 3 0\n-3 4 0\n-4 1 0\n");
    }
    #[test]
    fn sat_with_zero_signed_complex_sum_is_not_unsat() {
        check("p cnf 2 1\nc t wmc\n\
c p weight 1 1 0 0\nc p weight -1 -1 0 0\n2 0\n");
    }
    #[test]
    fn true_unsat_and_empty_clause_are_zero() {
        check("p cnf 2 2\nc t wmc\n1 0\n-1 0\n");
        check("p cnf 2 1\nc t wmc\n0\n");
    }
    #[test]
    fn free_and_disconnected_factors_have_correct_weight_ownership() {
        check("p cnf 6 4\nc t wmc\n\
c p weight 6 -0.5 0.5 0\nc p weight -6 0.25 -0.25 0\n\
1 2 0\n-1 3 0\n4 -5 0\n-4 6 0\n");
    }
    #[test]
    fn research_scope_probe_changes_only_admission_not_signed_wmc() {
        // Synthetic boundary fixture, not a performance benchmark.
        // All 257 root units are Boolean satisfiable and count exactly 1.
        let mut cnf = String::from("p cnf 257 257\nc t wmc\n");
        for var in 1..=257 {
            cnf.push_str(&format!("{var} 0\n"));
        }
        for bits in [100, 333] {
            let p = parse_emitted(&cnf, bits).unwrap();
            let scope: Vec<usize> = (1..=257).collect();
            let (baseline, before) = count(&p, &scope, bits);
            assert!(baseline.is_none());
            assert_eq!(before.cap_reason, "source_scope");
            let (candidate, after) = count_scope_probe(&p, &scope, bits);
            let (minfill, minfill_stats) = count_scope_probe_minfill(&p, &scope, bits);
            assert!(minfill_stats.eligible && !minfill_stats.capped, "{minfill_stats:?}");
            let candidate_minfill = minfill.expect("bounded 257-unit minfill VE");
            let candidate_ref = candidate.as_ref().expect("bounded 257-unit VE");
            assert_eq!(candidate_minfill.sat, candidate_ref.sat);
            assert_eq!(candidate_minfill.count.re, candidate_ref.count.re);
            assert_eq!(candidate_minfill.count.im, candidate_ref.count.im);
            assert!(after.eligible && !after.capped, "{after:?}");
            let answer = candidate.expect("bounded 257-unit VE");
            assert!(answer.sat);
            assert_eq!(answer.count.re, 1);
            assert_eq!(answer.count.im, 0);
            let mut trail = TrailEngine::new(&p, bits, 128, 100_000);
            let old = trail.count(scope, 0).unwrap();
            assert_eq!(answer.sat, old.sat);
            assert_eq!(answer.count.re, old.count.re);
            assert_eq!(answer.count.im, old.count.im);
        }
    }

    #[test]
    fn conservative_cost_preflight_rejects_dense_exact_work_before_mpfr_tables() {
        let mut cnf = String::from("p cnf 14 364\nc t wmc\n");
        for i in 1..=14 {
            for j in (i + 1)..=14 {
                for _ in 0..4 {
                    cnf.push_str(&format!("{i} {j} 0\n"));
                }
            }
        }
        for bits in [100_u32, 333] {
            let p = parse_emitted(&cnf, bits).unwrap();
            let scope: Vec<usize> = (1..=14).collect();
            let (candidate, stats) =
                count_scope_probe_minfill_bag14_preflight(&p, &scope, bits);
            assert!(candidate.is_none());
            assert!(stats.capped);
            assert_eq!(stats.cap_reason, "preflight_products");
            assert!(stats.preflight_products_upper > MAX_PRODUCTS);
            assert_eq!(stats.products, 0);
            assert_eq!(stats.cells, 0);
            // Correctness remains the original exhaustive DPLL fallback.
            let mut trail = TrailEngine::new(&p, bits, 128, 100_000);
            let answer = trail.count(scope, 0).unwrap();
            assert!(answer.sat);
            assert_eq!(answer.count.re, 15);
            assert_eq!(answer.count.im, 0);
        }
    }

    #[test]
    fn excessive_source_clause_width_fails_closed() {
        let mut cnf = String::from("p cnf 13 1\nc t wmc\n");
        for var in 1..=13 { cnf.push_str(&format!("{var} ")); }
        cnf.push_str("0\n");
        let p = parse_emitted(&cnf, 100).unwrap();
        let (answer, stats) = count(&p, &(1..=p.n).collect::<Vec<_>>(), 100);
        assert!(answer.is_none() && stats.capped);
    }
}
