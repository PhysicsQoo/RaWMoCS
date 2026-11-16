//! Strict, bounded XOR implication closure on the *existing* weighted CNF.
//!
//! A complete 2- or 3-variable parity truth-table encoding implies an XOR
//! equation. Gaussian elimination over GF(2) can then derive unary or binary
//! clauses logically entailed by the ORIGINAL formula. We only ADD these
//! consequences; no variable elimination and no literal-weight changes.
//! Therefore the entire model set (and Full Complex WMC) is preserved.
//! Research-only; absent evidence or any budget exhaustion: unchanged CNF.
use std::collections::{BTreeMap, BTreeSet};

use crate::Clause;

const MAX_VARS: usize = 1024;
const MAX_CLAUSES: usize = 5000;
const MAX_EQUATIONS: usize = 512;
const MAX_ROW_XORS: usize = 250_000;
const MAX_NEW_CLAUSES: usize = 1024;

#[derive(Clone, Debug, Default)]
pub(crate) struct XorClosureStats {
    pub(crate) equations: usize,
    pub(crate) rank: usize,
    pub(crate) forced_units: usize,
    pub(crate) binary_clauses: usize,
    pub(crate) contradiction: bool,
    pub(crate) row_xors: usize,
    pub(crate) censored: bool,
}

#[derive(Clone, Debug)]
struct Row {
    bits: Vec<u64>,
    rhs: bool,
}
impl Row {
    fn new(vars: &[usize], rhs: bool, words: usize) -> Self {
        let mut bits = vec![0_u64; words];
        for &v in vars {
            let zero_based = v - 1;
            bits[zero_based / 64] ^= 1_u64 << (zero_based % 64);
        }
        Self { bits, rhs }
    }
    fn bit(&self, v: usize) -> bool {
        let v = v - 1;
        self.bits[v / 64] & (1_u64 << (v % 64)) != 0
    }
    fn pivot(&self) -> Option<usize> {
        self.bits
            .iter()
            .enumerate()
            .find_map(|(i, &word)| (word != 0).then(|| i * 64 + word.trailing_zeros() as usize + 1))
    }
    fn variables(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (block, &original) in self.bits.iter().enumerate() {
            let mut word = original;
            while word != 0 {
                result.push(block * 64 + word.trailing_zeros() as usize + 1);
                word &= word - 1;
            }
        }
        result
    }
    fn xor(&mut self, other: &Self, stats: &mut XorClosureStats) -> Result<(), ()> {
        if stats.row_xors >= MAX_ROW_XORS {
            return Err(());
        }
        stats.row_xors += 1;
        for (a, b) in self.bits.iter_mut().zip(&other.bits) {
            *a ^= *b;
        }
        self.rhs ^= other.rhs;
        Ok(())
    }
}

/// A literal's sign determines the truth assignment forbidden by its clause.
/// A set of all 2^(k-1) forbidden assignments of one parity (k=2 or 3)
/// logically implies the opposite parity. Partial groups are NEVER admitted.
fn source_equations(clauses: &[Clause], n: usize) -> Vec<Row> {
    let mut groups: BTreeMap<(usize, [usize; 3]), u8> = BTreeMap::new();
    for clause in clauses {
        let k = clause.len();
        if !(2..=3).contains(&k) {
            continue;
        }
        let mut vars = [0_usize; 3];
        for (v, &lit) in vars.iter_mut().zip(clause) {
            *v = lit.unsigned_abs() as usize;
        }
        vars[..k].sort_unstable();
        if vars[0] == 0 || vars[k - 1] > n || vars[..k].windows(2).any(|pair| pair[0] == pair[1]) {
            continue;
        }
        let mut forbidden = 0_u8;
        for &lit in clause {
            if lit < 0 {
                let slot = vars[..k]
                    .iter()
                    .position(|&v| v == lit.unsigned_abs() as usize)
                    .expect("all literals were sorted into the validated variable set");
                forbidden |= 1 << slot;
            }
        }
        groups
            .entry((k, vars))
            .and_modify(|mask| *mask |= 1 << forbidden)
            .or_insert(1 << forbidden);
    }
    let mut equations = Vec::new();
    let words = n.div_ceil(64);
    for ((k, vars), mask) in groups {
        for parity in [0_u8, 1_u8] {
            let mut admitted = true;
            for assignment in 0..(1 << k) {
                if (assignment as u8).count_ones() as u8 % 2 == parity
                    && mask & (1 << assignment) == 0
                {
                    admitted = false;
                    break;
                }
            }
            if admitted {
                equations.push(Row::new(&vars[..k], parity == 0, words));
            }
        }
    }
    equations
}

/// Returns the original input unchanged when the workload exceeds any cap.
/// Only a Boolean contradictory subset can cause an empty clause, and that
/// is a proof of UNSAT, not a numerical zero in a SAT weighted problem.
pub(crate) fn implied_closure(clauses: &[Clause], n: usize) -> (Vec<Clause>, XorClosureStats) {
    let mut stats = XorClosureStats::default();
    if n == 0 || n > MAX_VARS || clauses.len() > MAX_CLAUSES {
        stats.censored = true;
        return (clauses.to_vec(), stats);
    }
    let equations = source_equations(clauses, n);
    stats.equations = equations.len();
    if equations.len() > MAX_EQUATIONS {
        stats.censored = true;
        return (clauses.to_vec(), stats);
    }
    if equations.is_empty() {
        return (clauses.to_vec(), stats);
    }
    let mut basis: BTreeMap<usize, Row> = BTreeMap::new();
    for mut row in equations {
        loop {
            let Some(pivot) = row.pivot() else {
                if row.rhs {
                    stats.contradiction = true;
                    let mut result = clauses.to_vec();
                    result.push(Vec::new());
                    return (result, stats);
                }
                break;
            };
            if let Some(prior) = basis.get(&pivot) {
                if row.xor(prior, &mut stats).is_err() {
                    stats.censored = true;
                    return (clauses.to_vec(), stats);
                }
            } else {
                basis.insert(pivot, row);
                break;
            }
        }
    }
    // Convert to RREF so all logically implied low-support combinations of
    // the extracted XOR equations become visible (including new unary rows).
    let pivots: Vec<usize> = basis.keys().copied().collect();
    for &pivot in pivots.iter().rev() {
        let lower = basis.get(&pivot).expect("source pivot").clone();
        for &earlier in &pivots {
            if earlier >= pivot {
                break;
            }
            if basis.get(&earlier).expect("earlier row").bit(pivot)
                && basis
                    .get_mut(&earlier)
                    .expect("earlier row")
                    .xor(&lower, &mut stats)
                    .is_err()
            {
                stats.censored = true;
                return (clauses.to_vec(), stats);
            }
        }
    }
    stats.rank = basis.len();
    let mut known: BTreeSet<Clause> = clauses
        .iter()
        .filter(|c| c.len() <= 2)
        .map(|c| {
            let mut c = c.clone();
            c.sort_unstable();
            c
        })
        .collect();
    let mut additions = Vec::new();
    for row in basis.values() {
        let vars = row.variables();
        let new_clauses: Vec<Clause> = match vars.as_slice() {
            [a] => vec![vec![if row.rhs { *a as i32 } else { -(*a as i32) }]],
            [a, b] if row.rhs => vec![vec![*a as i32, *b as i32], vec![-(*a as i32), -(*b as i32)]],
            [a, b] => vec![vec![-(*a as i32), *b as i32], vec![*a as i32, -(*b as i32)]],
            _ => continue,
        };
        for mut clause in new_clauses {
            clause.sort_unstable();
            if known.insert(clause.clone()) {
                if additions.len() >= MAX_NEW_CLAUSES {
                    stats.censored = true;
                    return (clauses.to_vec(), stats);
                }
                if clause.len() == 1 {
                    stats.forced_units += 1;
                } else {
                    stats.binary_clauses += 1;
                }
                additions.push(clause);
            }
        }
    }
    let mut result = clauses.to_vec();
    result.extend(additions);
    (result, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, CachePolicy, Engine};

    fn parity_clauses(vars: &[i32], rhs: bool) -> Vec<Clause> {
        let mut result = Vec::new();
        for mask in 0..(1 << vars.len()) {
            if ((mask as u32).count_ones() % 2 == 1) == rhs {
                continue;
            }
            let clause = vars
                .iter()
                .enumerate()
                .map(|(i, &v)| if mask & (1 << i) != 0 { -v } else { v })
                .collect();
            result.push(clause);
        }
        result
    }

    fn sat(clauses: &[Clause], mask: usize) -> bool {
        clauses.iter().all(|clause| {
            clause.iter().any(|&lit| {
                let bit = mask & (1 << (lit.unsigned_abs() as usize - 1)) != 0;
                bit == (lit > 0)
            })
        })
    }

    #[test]
    fn complete_parities_and_gaussian_cancellation_produce_new_units() {
        let mut clauses = parity_clauses(&[1, 2, 3], false);
        clauses.extend(parity_clauses(&[1, 2, 4], true));
        clauses.extend(parity_clauses(&[3, 4], false));
        let (enhanced, stats) = implied_closure(&clauses, 4);
        assert!(!stats.censored);
        // x1 ^ x2 ^ x3 = 0; x1 ^ x2 ^ x4 = 1; x3 ^ x4 = 0
        // is inconsistent though each individual equation is feasible.
        assert!(stats.contradiction);
        assert!(enhanced.iter().any(Vec::is_empty));
        assert!(!(0..16).any(|x| sat(&clauses, x)));
    }

    #[test]
    fn rref_keeps_free_variables_and_discovers_two_variable_implications() {
        let mut clauses = parity_clauses(&[1, 2, 3], false);
        clauses.extend(parity_clauses(&[1, 2, 4], false));
        // cancellation derives x3 ^ x4 = 0; no variable is deleted.
        let (enhanced, stats) = implied_closure(&clauses, 4);
        assert!(!stats.contradiction);
        assert!(stats.binary_clauses >= 2);
        for x in 0..16 {
            assert_eq!(sat(&clauses, x), sat(&enhanced, x));
        }
    }

    #[test]
    fn partial_xor_patterns_and_non_xor_background_never_justify_inference() {
        let mut clauses = parity_clauses(&[1, 2, 3], true);
        clauses.pop();
        clauses.push(vec![-3, 4]);
        let (enhanced, stats) = implied_closure(&clauses, 4);
        assert_eq!(stats.equations, 0);
        assert_eq!(enhanced, clauses);
    }

    #[test]
    fn preserve_all_small_weighted_cnf_assignment_sets_and_mpfr_counts() {
        for bits in [100, 333] {
            for seed in 0..30_usize {
                let mut clauses = parity_clauses(&[1, 2, 3], seed & 1 != 0);
                clauses.extend(parity_clauses(&[1, 2, 4], seed & 2 != 0));
                if seed & 4 != 0 {
                    clauses.extend(parity_clauses(&[3, 4], seed & 8 != 0));
                }
                // Include non-parity, unit, duplicated and irregular clauses.
                if seed & 1 != 0 {
                    clauses.push(vec![2, -5]);
                }
                if seed & 2 != 0 {
                    clauses.push(vec![5]);
                }
                if seed & 8 != 0 {
                    clauses.push(vec![-3, 6]);
                }
                if seed & 16 != 0 {
                    clauses.push(clauses[0].clone());
                }
                let (enhanced, stats) = implied_closure(&clauses, 6);
                assert!(!stats.censored, "seed={seed}");
                for x in 0..64 {
                    assert_eq!(
                        sat(&clauses, x),
                        sat(&enhanced, x),
                        "seed={seed}, assignment={x}"
                    );
                }
                let mut raw = format!("p cnf 6 {}\nc t wmc\n", clauses.len());
                for v in 1..=6 {
                    raw.push_str(&format!("c p weight {v} 0.25 0.125 0\n"));
                    raw.push_str(&format!("c p weight -{v} -0.125 0.25 0\n"));
                }
                for clause in &clauses {
                    for lit in clause {
                        raw.push_str(&format!("{lit} "));
                    }
                    raw.push_str("0\n");
                }
                let problem = parse_emitted(&raw, bits).unwrap();
                let mut original = Engine::new(&problem, bits, CachePolicy::StdOwned);
                let a = original
                    .count(clauses.clone(), (1..=6).collect(), 0)
                    .unwrap();
                let mut proposed = Engine::new(&problem, bits, CachePolicy::StdOwned);
                let b = proposed.count(enhanced, (1..=6).collect(), 0).unwrap();
                assert_eq!(
                    (a.sat, a.count.re, a.count.im),
                    (b.sat, b.count.re, b.count.im),
                    "seed={seed}, precision={bits}"
                );
            }
        }
    }

    #[test]
    fn strict_budget_and_negative_controls_do_not_modify_source() {
        let clauses = parity_clauses(&[1, 2, 3], false);
        let (same, stats) = implied_closure(&clauses, 1025);
        assert_eq!(same, clauses);
        assert!(stats.censored);
        let (same, stats) = implied_closure(&vec![vec![1, 2]; 5001], 2);
        assert_eq!(same.len(), 5001);
        assert!(stats.censored);
    }
}
