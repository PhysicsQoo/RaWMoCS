//! Research-only reusable CSR occurrence workspace for weighted-CNF unit propagation.
//! The original CSR implementation remains the independent comparison arm.
//! This preserves source clause order, first-unit priority, all complex MPFR
//! multiplication order and Boolean conflict semantics; it only reuses buffers.
use crate::{occurrence_slot, Clause, Weights, C};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Default)]
pub(crate) struct Workspace {
    counts: Vec<usize>,
    lengths: Vec<usize>,
    alive: Vec<bool>,
    unit: BinaryHeap<Reverse<usize>>,
    assignments: Vec<i8>,
    offsets: Vec<usize>,
    cursors: Vec<usize>,
    indices: Vec<usize>,
}

impl Workspace {
    pub(crate) fn capacities(&self) -> (usize, usize, usize) {
        (self.counts.capacity(), self.indices.capacity(), self.lengths.capacity())
    }

    fn reset(&mut self, variables: usize, clauses: usize) {
        let slots = 2 * (variables + 1);
        self.counts.resize(slots, 0);
        self.counts.fill(0);
        self.lengths.clear();
        self.alive.resize(clauses, true);
        self.alive.fill(true);
        self.unit.clear();
        self.assignments.resize(variables + 1, 0);
        self.assignments.fill(0);
        self.offsets.clear();
        self.cursors.clear();
        // Entries in indices will be fully overwritten by the second
        // occurrence traversal; allocating/clearing them here is redundant.
    }
}

/// Execute exactly the control CSR propagation using stable, reusable arenas.
/// The Workspace must be returned to the Engine before any child recursion.
/// The original residual, weights and scope are never stored in this arena.
pub(crate) fn propagate(
    clauses: &[Clause],
    variables: usize,
    weights: &[Weights],
    bits: u32,
    remaining: &mut Vec<usize>,
    prefix: &mut C,
    arena: &mut Workspace,
) -> Option<Vec<Clause>> {
    let slots = 2 * (variables + 1);
    arena.reset(variables, clauses.len());
    for (cid, clause) in clauses.iter().enumerate() {
        if clause.is_empty() {
            return None;
        }
        arena.lengths.push(clause.len());
        if clause.len() == 1 {
            arena.unit.push(Reverse(cid));
        }
        for &lit in clause {
            arena.counts[occurrence_slot(lit)] += 1;
        }
    }

    arena.offsets.push(0);
    for &count in &arena.counts {
        let end = arena.offsets.last().copied().unwrap() + count;
        arena.offsets.push(end);
    }
    arena.cursors.extend_from_slice(&arena.offsets[..slots]);
    let total = arena.offsets[slots];
    arena.indices.resize(total, 0);
    for (cid, clause) in clauses.iter().enumerate() {
        for &lit in clause {
            let slot = occurrence_slot(lit);
            arena.indices[arena.cursors[slot]] = cid;
            arena.cursors[slot] += 1;
        }
    }

    while let Some(Reverse(cid)) = arena.unit.pop() {
        if !arena.alive[cid] || arena.lengths[cid] != 1 {
            continue;
        }
        let lit = *clauses[cid]
            .iter()
            .find(|&&lit| arena.assignments[lit.unsigned_abs() as usize] == 0)
            .expect("active unit has one unassigned literal");
        let var = lit.unsigned_abs() as usize;
        let side = usize::from(lit < 0);
        let w = &weights[var - 1];
        *prefix = prefix.mul(if lit > 0 { &w.positive } else { &w.negative }, bits);
        remaining.retain(|&v| v != var);
        arena.assignments[var] = if lit > 0 { 1 } else { -1 };

        let satisfied_slot = 2 * var + side;
        for &index in
            &arena.indices[arena.offsets[satisfied_slot]..arena.offsets[satisfied_slot + 1]]
        {
            arena.alive[index] = false;
        }
        let falsified_slot = 2 * var + (1 - side);
        for &index in
            &arena.indices[arena.offsets[falsified_slot]..arena.offsets[falsified_slot + 1]]
        {
            if !arena.alive[index] {
                continue;
            }
            arena.lengths[index] -= 1;
            if arena.lengths[index] == 0 {
                return None;
            }
            if arena.lengths[index] == 1 {
                arena.unit.push(Reverse(index));
            }
        }
    }

    let mut reduced = Vec::with_capacity(clauses.len());
    for (cid, clause) in clauses.iter().enumerate() {
        if !arena.alive[cid] {
            continue;
        }
        let mut next = Vec::with_capacity(arena.lengths[cid]);
        for &lit in clause {
            if arena.assignments[lit.unsigned_abs() as usize] == 0 {
                next.push(lit);
            }
        }
        debug_assert_eq!(next.len(), arena.lengths[cid]);
        reduced.push(next);
    }
    Some(reduced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, unit_propagate_indexed_csr, CachePolicy, Engine};

    #[test]
    fn preserves_first_unit_order_and_complex_prefix_at_two_precisions() {
        let samples = [
            "p cnf 7 7\nc t wmc\nc p weight 1 0.11 0.21 0\nc p weight -1 0.4 0 0\nc p weight 2 0.33 -0.3 0\nc p weight -2 -0.4 0.1 0\n1 2 3 0\n-1 4 0\n-4 5 0\n-5 6 0\n-6 7 0\n-7 0\n2 3 0\n",
            "p cnf 7 3\nc t wmc\n1 0\n-1 0\n2 3 0\n",
            "p cnf 7 3\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n2 -3 0\n-2 4 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            let mut arena = Workspace::default();
            for sample in samples {
                let p = parse_emitted(sample, bits).unwrap();
                let mut old_scope: Vec<usize> = (1..=p.n).collect();
                let mut new_scope = old_scope.clone();
                let mut old_factor = C::one(bits);
                let mut new_factor = C::one(bits);
                let original = unit_propagate_indexed_csr(
                    &p.clauses, p.n, &p.weights, bits, &mut old_scope, &mut old_factor,
                );
                let candidate = propagate(
                    &p.clauses, p.n, &p.weights, bits, &mut new_scope, &mut new_factor, &mut arena,
                );
                assert_eq!(original, candidate);
                assert_eq!(old_scope, new_scope);
                assert_eq!(old_factor.re, new_factor.re);
                assert_eq!(old_factor.im, new_factor.im);
            }
            assert!(arena.capacities().0 >= 2 * (7 + 1));
        }
    }

    #[test]
    fn engine_repeated_recursion_caches_and_sat_zero_are_exact() {
        let fixtures = [
            "p cnf 6 8\nc t wmc\n1 2 3 0\n-1 4 0\n-2 -3 0\n-4 5 0\n-5 6 0\n-6 1 0\n2 -6 0\n-2 4 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            for input in fixtures {
                let p = parse_emitted(input, bits).unwrap();
                for cache in [0_usize, 1, 10, 10_000] {
                    let mut baseline = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    baseline.indexed_unit_propagation = true;
                    baseline.indexed_unit_csr = true;
                    baseline.fast_assign = true;
                    baseline.cache_limit = cache;
                    let expected = baseline.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                    let mut candidate = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    candidate.indexed_unit_propagation = true;
                    candidate.indexed_unit_csr = true;
                    candidate.reuse_csr_workspace = true;
                    candidate.fast_assign = true;
                    candidate.cache_limit = cache;
                    let actual = candidate.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                    assert_eq!(
                        (expected.sat, expected.count.re, expected.count.im),
                        (actual.sat, actual.count.re, actual.count.im),
                    );
                    assert_eq!(
                        (baseline.stats.nodes, baseline.stats.cache_hits, baseline.stats.decompositions),
                        (candidate.stats.nodes, candidate.stats.cache_hits, candidate.stats.decompositions),
                    );
                }
            }
        }
    }
}
