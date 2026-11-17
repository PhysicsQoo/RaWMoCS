//! Exact bounded signed-complex sum-product variable elimination.
//! Retains SAT separately from WMC: zero complex weight is not UNSAT.
//! Input clauses, weights and scope are never mutated. On any bound
//! failure the caller uses the unchanged original Trail solver.
use crate::{C, Outcome, Problem};

const MAX_VARS: usize = 4096;
const MAX_SCOPE: usize = 256;
const MAX_CLAUSES: usize = 3000;
const MAX_CLAUSE_WIDTH: usize = 12;
const MAX_BAG: usize = 11;
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
}

fn reject(mut stats: VeStats) -> (Option<Outcome>, VeStats) {
    stats.capped = true;
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
    let mut selected: Option<(usize, usize, usize, Vec<usize>, Vec<usize>)> = None;
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
    let mut stats = VeStats::default();
    if p.n == 0 || p.n > MAX_VARS || scope.len() > MAX_SCOPE
        || p.clauses.len() > MAX_CLAUSES || p.weights.len() < p.n
    {
        return reject(stats);
    }
    let mut present = vec![false; p.n + 1];
    for &v in scope {
        if v == 0 || v > p.n || present[v] { return reject(stats); }
        present[v] = true;
    }
    let mut factors = Vec::with_capacity(p.clauses.len() + scope.len());
    for clause in &p.clauses {
        let Some(factor) = clause_factor(clause, &present, bits, &mut stats) else {
            return reject(stats);
        };
        factors.push(factor);
    }
    for &v in scope {
        if stats.cells.saturating_add(2) > MAX_CELLS { return reject(stats); }
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
    let mut remaining = scope.to_vec();
    remaining.sort_unstable();
    while !remaining.is_empty() {
        let (variable, incident, union) = selected_bucket(&factors, &remaining);
        stats.width = stats.width.max(union.len());
        if union.len() > MAX_BAG || union.is_empty() {
            return reject(stats);
        }
        let rest: Vec<usize> = union.into_iter().filter(|&v| v != variable).collect();
        let n_cells = 1_usize << rest.len();
        if stats.cells.saturating_add(n_cells) > MAX_CELLS { return reject(stats); }
        stats.cells += n_cells;
        let affected: Vec<&Factor> = incident.iter().map(|&i| &factors[i]).collect();
        let mut new_values = Vec::with_capacity(n_cells);
        for assignment in 0..n_cells {
            let Some(negative) = value_at(
                &affected, &rest, variable, false, assignment, bits, &mut stats
            ) else { return reject(stats) };
            let Some(positive) = value_at(
                &affected, &rest, variable, true, assignment, bits, &mut stats
            ) else { return reject(stats) };
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
            return reject(stats);
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
    fn excessive_source_clause_width_fails_closed() {
        let mut cnf = String::from("p cnf 13 1\nc t wmc\n");
        for var in 1..=13 { cnf.push_str(&format!("{var} ")); }
        cnf.push_str("0\n");
        let p = parse_emitted(&cnf, 100).unwrap();
        let (answer, stats) = count(&p, &(1..=p.n).collect::<Vec<_>>(), 100);
        assert!(answer.is_none() && stats.capped);
    }
}
