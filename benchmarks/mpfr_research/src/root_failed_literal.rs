//! Bounded failed-literal root preprocessing for signed Complex WMC.
//!
//! The ONLY transformation is adding a CNF unit clause F |= literal,
//! established by Boolean unit-propagation refutation of F AND !literal.
//! This preserves the *entire satisfying assignment set*, so no weighted
//! factor or free-variable sum is changed. SAT must not be inferred from
//! a zero complex WMC. The procedure is research-only, exact and opt-in.
use crate::{Clause, Problem};
use std::cmp::Reverse;

const MAX_VARIABLES: usize = 4096;
const MAX_CLAUSES: usize = 50_000;
const MAX_LITERALS: usize = 200_000;
const MAX_CANDIDATES: usize = 32;
const MAX_WORK: usize = 250_000;

#[derive(Default, Debug, Clone)]
pub(crate) struct RootFailedStats {
    pub(crate) eligible: usize,
    pub(crate) candidates: usize,
    pub(crate) probes: usize,
    pub(crate) implied_units: usize,
    pub(crate) work: usize,
    pub(crate) refutations: usize,
    pub(crate) contradicted: bool,
    pub(crate) capped: bool,
}

fn boolean_refutes(
    clauses: &[Clause], variable: usize, positive: bool,
    n: usize, work: &mut usize,
) -> Option<bool> {
    // None: budget exhausted. Some(true): actual Boolean conflict.
    // Some(false): no conflict found by Boolean unit propagation.
    let mut assignment = vec![0_i8; n + 1];
    assignment[variable] = if positive { 1 } else { -1 };
    loop {
        let mut changed = false;
        for clause in clauses {
            let mut open_count = 0_usize;
            let mut last_open = 0_i32;
            let mut satisfied = false;
            for &literal in clause {
                if *work >= MAX_WORK {
                    return None;
                }
                *work += 1;
                let var = literal.unsigned_abs() as usize;
                let sign = if literal > 0 { 1_i8 } else { -1_i8 };
                match assignment[var] {
                    0 => {
                        open_count += 1;
                        last_open = literal;
                    }
                    value if value == sign => {
                        satisfied = true;
                        break;
                    }
                    _ => {}
                }
            }
            if satisfied {
                continue;
            }
            if open_count == 0 {
                return Some(true);
            }
            if open_count == 1 {
                let forced = last_open.unsigned_abs() as usize;
                let sign = if last_open > 0 { 1_i8 } else { -1_i8 };
                if assignment[forced] == -sign {
                    return Some(true);
                }
                if assignment[forced] == 0 {
                    assignment[forced] = sign;
                    changed = true;
                }
            }
        }
        if !changed {
            return Some(false);
        }
    }
}

/// Add only *entailed* unit clauses to the original already-weighted CNF.
///
/// None means exactly unchanged input; no source clone or extra scope/factors.
/// Once the work cap is reached, keep only units independently proven
/// before that point, marking this as a bounded partial experiment.
pub(crate) fn augment(
    input: &Problem,
    scope: &[usize],
) -> (Option<Problem>, RootFailedStats) {
    let mut stats = RootFailedStats::default();
    if input.n == 0 || input.n > MAX_VARIABLES || input.clauses.len() > MAX_CLAUSES {
        stats.capped = true;
        return (None, stats);
    }
    let mut freq = vec![0_usize; input.n + 1];
    let mut incidences = 0_usize;
    for clause in &input.clauses {
        incidences += clause.len();
        if incidences > MAX_LITERALS {
            stats.capped = true;
            return (None, stats);
        }
        for &literal in clause {
            let var = literal.unsigned_abs() as usize;
            if var == 0 || var > input.n {
                // Invalid source should be rejected upstream; never use it.
                stats.capped = true;
                return (None, stats);
            }
            freq[var] += 1;
        }
    }
    let mut candidate_vars: Vec<usize> = scope.iter().copied().filter(
        |&var| var > 0 && var <= input.n && freq[var] > 0
    ).collect();
    candidate_vars.sort_unstable_by_key(|&var| (Reverse(freq[var]), var));
    candidate_vars.dedup();
    stats.eligible = candidate_vars.len();
    let mut implied: Vec<Clause> = Vec::new();
    for var in candidate_vars.into_iter().take(MAX_CANDIDATES) {
        stats.candidates += 1;
        stats.probes += 1;
        let positive_conflict = boolean_refutes(
            &input.clauses, var, true, input.n, &mut stats.work
        );
        let Some(positive_conflict) = positive_conflict else {
            stats.capped = true;
            break;
        };
        stats.probes += 1;
        let negative_conflict = boolean_refutes(
            &input.clauses, var, false, input.n, &mut stats.work
        );
        let Some(negative_conflict) = negative_conflict else {
            stats.capped = true;
            break;
        };
        stats.refutations += usize::from(positive_conflict)
            + usize::from(negative_conflict);
        if positive_conflict && negative_conflict {
            // Both Boolean assumptions are refuted; F is UNSAT, not
            // merely a zero weighted sum. F entails the empty clause.
            implied.push(Vec::new());
            stats.contradicted = true;
            break;
        }
        if positive_conflict {
            implied.push(vec![-(var as i32)]);
            stats.implied_units += 1;
        } else if negative_conflict {
            implied.push(vec![var as i32]);
            stats.implied_units += 1;
        }
    }
    if implied.is_empty() {
        return (None, stats);
    }
    let mut clauses = input.clauses.clone();
    clauses.extend(implied);
    (
        Some(Problem {
            n: input.n,
            clauses,
            weights: input.weights.clone(),
        }),
        stats,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, CachePolicy, Engine, Outcome, C};

    fn source(clauses: &[Clause], bits: u32, first_weights: (&str, &str)) -> Problem {
        let mut input = format!("p cnf 4 {}\nc t wmc\n", clauses.len());
        input.push_str(&format!("c p weight 1 {} 0.125 0\n", first_weights.0));
        input.push_str(&format!("c p weight -1 {} -0.25 0\n", first_weights.1));
        for v in 2..=4 {
            input.push_str(&format!("c p weight {v} 0.5 0.25 0\n"));
            input.push_str(&format!("c p weight -{v} -0.25 0.5 0\n"));
        }
        for clause in clauses {
            for literal in clause {
                input.push_str(&format!("{literal} "));
            }
            input.push_str("0\n");
        }
        parse_emitted(&input, bits).expect("valid exact dyadic complex WMC")
    }

    fn brute(p: &Problem, bits: u32) -> Outcome {
        let mut sat = false;
        let mut result = C::zero(bits);
        for mask in 0..(1_usize << p.n) {
            if !p.clauses.iter().all(|clause| clause.iter().any(|&lit| {
                let var = lit.unsigned_abs() as usize - 1;
                ((mask >> var) & 1 != 0) == (lit > 0)
            })) {
                continue;
            }
            sat = true;
            let mut term = C::one(bits);
            for var in 1..=p.n {
                let weight = &p.weights[var - 1];
                term = term.mul(
                    if mask & (1 << (var - 1)) != 0 {
                        &weight.positive
                    } else {
                        &weight.negative
                    },
                    bits,
                );
            }
            result = result.add(&term, bits);
        }
        Outcome { sat, count: result }
    }

    fn verify(clauses: &[Clause], weights: (&str, &str), expected: Option<i32>) {
        for bits in [100, 333] {
            let p = source(clauses, bits, weights);
            let expected_brute = brute(&p, bits);
            let mut old = Engine::new(&p, bits, CachePolicy::StdOwned);
            let original = old.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
            let (added, stats) = augment(&p, &(1..=p.n).collect::<Vec<_>>());
            let converted = added.as_ref().unwrap_or(&p);
            let mut new = Engine::new(converted, bits, CachePolicy::StdOwned);
            let actual = new.count(
                converted.clauses.clone(), (1..=p.n).collect(), 0,
            ).unwrap();
            assert_eq!(original.sat, expected_brute.sat);
            assert_eq!(actual.sat, expected_brute.sat);
            assert_eq!(original.count.re, expected_brute.count.re);
            assert_eq!(original.count.im, expected_brute.count.im);
            assert_eq!(actual.count.re, expected_brute.count.re);
            assert_eq!(actual.count.im, expected_brute.count.im);
            if let Some(implied) = expected {
                assert!(added.is_some());
                assert!(converted.clauses.contains(&vec![implied]));
                assert!(stats.implied_units >= 1);
            }
        }
    }

    #[test]
    fn positive_failed_literal_adds_only_entailed_unit_with_complex_weights() {
        verify(&[vec![1, 2], vec![1, -2], vec![3, 4]], ("0.5", "-0.25"), Some(1));
    }

    #[test]
    fn negative_failed_literal_adds_only_entailed_unit_with_complex_weights() {
        verify(&[vec![-1, 2], vec![-1, -2], vec![3, 4]], ("0.5", "-0.25"), Some(-1));
    }

    #[test]
    fn sat_with_zero_complex_weight_is_not_mistaken_for_unsat() {
        // The formula forces 1=true, but its positive literal weight is 0.
        verify(&[vec![1, 2], vec![1, -2]], ("0", "1"), Some(1));
    }

    #[test]
    fn a_logical_contradiction_adds_empty_clause_not_weight_cancellation() {
        verify(
            &[vec![1, 2], vec![1, -2], vec![-1, 3], vec![-1, -3]],
            ("0.5", "-0.25"), None,
        );
        for bits in [100, 333] {
            let p = source(
                &[vec![1, 2], vec![1, -2], vec![-1, 3], vec![-1, -3]],
                bits, ("0.5", "-0.25"),
            );
            let (added, stats) = augment(&p, &(1..=p.n).collect::<Vec<_>>());
            assert!(stats.contradicted);
            assert!(added.unwrap().clauses.contains(&Vec::new()));
        }
    }

    #[test]
    fn no_proof_returns_exact_unmodified_source() {
        let p = source(&[vec![1, 2, 3], vec![-1, 2, 4]], 100, ("0.5", "0.25"));
        let (added, stats) = augment(&p, &(1..=p.n).collect::<Vec<_>>());
        assert!(added.is_none());
        assert_eq!(stats.implied_units, 0);
        assert!(!stats.contradicted);
    }

    #[test]
    fn source_bound_fails_closed_without_any_weight_mutation() {
        let mut p = source(&[vec![1, 2], vec![1, -2]], 100, ("0.5", "0.25"));
        p.clauses.extend(vec![vec![2, 3]; MAX_CLAUSES]);
        let (added, stats) = augment(&p, &(1..=p.n).collect::<Vec<_>>());
        assert!(added.is_none());
        assert!(stats.capped);
    }
}
