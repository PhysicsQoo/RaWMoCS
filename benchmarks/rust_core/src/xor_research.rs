//! Research-only sound XOR refutation from general, unordered CNF.
//!
//! Four different ternary clauses over precisely the same three distinct
//! variables can exclude one entire parity half-space. Only then does their
//! conjunction imply a three-variable XOR equation. We derive such equations
//! without relying on clause adjacency, and use bounded GF(2) elimination
//! with a replayable contradiction witness.
//!
//! This oracle is not enabled in the production solver, not a generic SAT
//! algorithm and not a DRAT/LRAT proof system.

use rawmocs_core::Problem;
use std::collections::{BTreeMap, BTreeSet};

const MAX_VARIABLES: usize = 256;
const MAX_CLAUSES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XorEquation {
    /// Sorted, one-indexed CNF variables.
    pub variables: [usize; 3],
    /// x_a XOR x_b XOR x_c = rhs.
    pub rhs: u8,
    /// Zero-indexed positions of the four original CNF clauses.
    pub clauses: [usize; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XorRefutation {
    pub variables: usize,
    /// Only equations used by the contradiction are included.
    pub equations: Vec<XorEquation>,
}

/// A one-parity half-space requires all four unique forbidden masks.
/// Skip groups with duplicates, wrong arity, tautologies, or mixtures
/// of forbidden parity; do not assume all CNF clauses form XORs.
fn source_equations(variables: usize, clauses: &[Vec<i32>]) -> Vec<XorEquation> {
    let mut grouped: BTreeMap<[usize; 3], Vec<(u8, usize)>> = BTreeMap::new();
    for (index, clause) in clauses.iter().enumerate() {
        if clause.len() != 3 {
            continue;
        }
        let mut vars = [
            clause[0].unsigned_abs() as usize,
            clause[1].unsigned_abs() as usize,
            clause[2].unsigned_abs() as usize,
        ];
        vars.sort_unstable();
        if vars[0] == 0 || vars[2] > variables || vars[0] == vars[1] || vars[1] == vars[2] {
            continue;
        }
        let mut forbidden = 0u8;
        for &lit in clause {
            if lit < 0 {
                let bit = vars
                    .iter()
                    .position(|&v| v == lit.unsigned_abs() as usize)
                    .expect("validated three-variable triple");
                forbidden |= 1 << bit;
            }
        }
        grouped.entry(vars).or_default().push((forbidden, index));
    }
    let mut result = Vec::new();
    for (vars, candidates) in grouped {
        // A duplicated clause cannot be silently counted as a distinct mask.
        if candidates.len() != 4 {
            continue;
        }
        let masks: BTreeSet<u8> = candidates.iter().map(|item| item.0).collect();
        if masks.len() != 4 {
            continue;
        }
        let parity = (candidates[0].0.count_ones() & 1) as u8;
        if candidates
            .iter()
            .any(|item| (item.0.count_ones() & 1) as u8 != parity)
        {
            continue;
        }
        let mut indices = [0usize; 4];
        for (j, candidate) in candidates.iter().enumerate() {
            indices[j] = candidate.1;
        }
        result.push(XorEquation {
            variables: vars,
            rhs: parity ^ 1,
            clauses: indices,
        });
    }
    result
}

fn xor_symmetric_difference(left: &mut BTreeSet<usize>, right: &BTreeSet<usize>) {
    for &id in right {
        if !left.insert(id) {
            left.remove(&id);
        }
    }
}

/// Returns an explicit UNSAT witness only if a contradiction was derived.
/// None means "no proof found", NEVER SAT or an exact WMC answer.
///
/// Work is capped by both input dimensions. The basis has at most 256 rows;
/// no candidate is run on unbounded large arbitrary CNFs.
pub fn refute(variables: usize, clauses: &[Vec<i32>]) -> Option<XorRefutation> {
    if variables == 0 || variables > MAX_VARIABLES || clauses.len() > MAX_CLAUSES {
        return None;
    }
    let equations = source_equations(variables, clauses);
    let words = variables.div_ceil(64);
    // Pivot by first set bit. All words are local, densely bounded bitsets.
    let mut basis: BTreeMap<usize, (Vec<u64>, u8, BTreeSet<usize>)> = BTreeMap::new();
    for (id, eq) in equations.iter().enumerate() {
        let mut row = vec![0u64; words];
        for &v in &eq.variables {
            row[(v - 1) / 64] ^= 1u64 << ((v - 1) % 64);
        }
        let mut rhs = eq.rhs;
        let mut used = BTreeSet::from([id]);
        loop {
            let pivot = row
                .iter()
                .enumerate()
                .find(|(_, bits)| **bits != 0)
                .map(|(word, bits)| 64 * word + bits.trailing_zeros() as usize);
            match pivot {
                None if rhs == 1 => {
                    let proof = XorRefutation {
                        variables,
                        equations: used.iter().map(|&i| equations[i].clone()).collect(),
                    };
                    if verify(variables, clauses, &proof) {
                        return Some(proof);
                    }
                    return None;
                }
                None => break,
                Some(key) => {
                    if let Some((previous, previous_rhs, previous_used)) = basis.get(&key) {
                        for (item, other) in row.iter_mut().zip(previous) {
                            *item ^= *other;
                        }
                        rhs ^= *previous_rhs;
                        xor_symmetric_difference(&mut used, previous_used);
                    } else {
                        basis.insert(key, (row, rhs, used));
                        break;
                    }
                }
            }
        }
    }
    None
}

/// Independently recheck the certificate against the *supplied original CNF*,
/// including the four forbidden assignments of each claimed XOR equation.
/// The XOR of all witness equations must have zero LHS and RHS one.
pub fn verify(variables: usize, clauses: &[Vec<i32>], proof: &XorRefutation) -> bool {
    if variables == 0
        || variables > MAX_VARIABLES
        || clauses.len() > MAX_CLAUSES
        || proof.variables != variables
        || proof.equations.is_empty()
    {
        return false;
    }
    let mut occurrences = vec![0u8; variables + 1];
    let mut rhs = 0u8;
    let mut unique = BTreeSet::new();
    for eq in &proof.equations {
        if eq.rhs > 1
            || eq.variables[0] == 0
            || eq.variables[2] > variables
            || !(eq.variables[0] < eq.variables[1] && eq.variables[1] < eq.variables[2])
        {
            return false;
        }
        let mut masks = BTreeSet::new();
        for &index in &eq.clauses {
            if !unique.insert(index) {
                return false;
            }
            let Some(clause) = clauses.get(index) else {
                return false;
            };
            if clause.len() != 3 {
                return false;
            }
            let mut seen = BTreeSet::new();
            let mut mask = 0u8;
            for &lit in clause {
                let v = lit.unsigned_abs() as usize;
                let Some(pos) = eq.variables.iter().position(|&x| x == v) else {
                    return false;
                };
                if !seen.insert(v) {
                    return false;
                }
                if lit < 0 {
                    mask |= 1 << pos;
                }
            }
            if seen.len() != 3 || (mask.count_ones() as u8 & 1) == eq.rhs {
                return false;
            }
            masks.insert(mask);
        }
        if masks.len() != 4 {
            return false;
        }
        for &v in &eq.variables {
            occurrences[v] ^= 1;
        }
        rhs ^= eq.rhs;
    }
    rhs == 1 && occurrences.iter().all(|x| *x == 0)
}

pub fn refute_problem(problem: &Problem) -> Option<XorRefutation> {
    refute(problem.variables(), problem.clauses())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dubois_xor_certificate::verify_dubois20_unsat;
    use crate::satlib_provenance::{fixture, original_dubois20_clauses};
    use crate::irregular_holdout::irregular;

    fn brute_sat(variables: usize, clauses: &[Vec<i32>]) -> bool {
        assert!(variables <= 10);
        (0..(1usize << variables)).any(|a| {
            clauses.iter().all(|clause| {
                clause.iter().any(|&lit| {
                    let value = (a & (1usize << (lit.unsigned_abs() as usize - 1))) != 0;
                    if lit > 0 { value } else { !value }
                })
            })
        })
    }

    #[test]
    fn source_pinned_dubois20_has_replayable_generic_unsat_proof() {
        let reference = verify_dubois20_unsat().unwrap();
        assert_eq!(reference.xor_equations, 40);
        let (variables, clauses) = original_dubois20_clauses();
        let proof = refute(variables, &clauses).expect("independent general CNF xor refutation");
        assert!(verify(variables, &clauses, &proof));
        assert!(proof.equations.len() <= 40);
        let mut shuffled = clauses.clone();
        shuffled.reverse();
        shuffled.rotate_left(17);
        let reordered = refute(variables, &shuffled).expect("not dependent on source ordering");
        assert!(verify(variables, &shuffled, &reordered));
        assert!(!verify(variables, &shuffled, &proof), "witness source positions matter");
    }

    #[test]
    fn exact_sat_controls_and_irregular_sources_cannot_be_mislabeled_unsat() {
        for name in ["satlib_uf20_01", "satlib_uf20_02"] {
            let source = fixture(name);
            assert!(refute_problem(&source).is_none(), "original source is SAT");
        }
        for name in ["irregular8", "irregular10"] {
            let source = irregular(name);
            assert!(brute_sat(source.variables(), source.clauses()));
            assert!(refute_problem(&source).is_none());
        }
    }

    #[test]
    fn incomplete_duplicate_tautological_and_tampered_parity_are_not_proofs() {
        let (variables, original) = original_dubois20_clauses();
        // Remove, replace, duplicate a source clause from one complete group.
        for kind in 0..4 {
            let mut clauses = original.clone();
            match kind {
                0 => { clauses.remove(0); }
                1 => { clauses[0][0] = -clauses[0][0]; }
                2 => { clauses.push(clauses[0].clone()); }
                _ => { clauses[0] = vec![1, -1, 2]; }
            }
            assert!(refute(variables, &clauses).is_none(), "kind={kind}");
        }
    }

    #[test]
    fn independent_exhaustive_small_cnf_no_false_unsat() {
        let all = (1_i32..=4)
            .flat_map(|a| {
                (1_i32..=4).filter(move |&b| b != a).map(move |b| vec![a, -b, 4])
            })
            .collect::<Vec<_>>();
        // 200 deterministic subsets of a SAT formula. Every clause is
        // satisfied by assignment where all variables are True.
        for k in 0..200usize {
            let clauses = all
                .iter()
                .enumerate()
                .filter(|(i, _)| ((i * 3 + k * 5) % 13) < 6)
                .map(|(_, clause)| clause.clone())
                .collect::<Vec<_>>();
            assert!(brute_sat(4, &clauses));
            assert!(refute(4, &clauses).is_none());
        }
        // A complete 3-XOR contradiction plus a forced opposite parity:
        // 4 clause groups together forbid all 8 assignments of three vars.
        let mut eight = Vec::new();
        for a in 0_u8..8 {
            eight.push(vec![
                if a & 1 != 0 { -1 } else { 1 },
                if a & 2 != 0 { -2 } else { 2 },
                if a & 4 != 0 { -3 } else { 3 },
            ]);
        }
        assert!(!brute_sat(3, &eight));
        // Both contradictory parity groups share a triple; we conservatively
        // refuse pattern admission on groups >4. None is a valid outcome.
        assert!(refute(3, &eight).is_none());
    }
}
