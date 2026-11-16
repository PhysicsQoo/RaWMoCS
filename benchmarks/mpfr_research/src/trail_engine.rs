//! Research-only reversible source-clause weighted DPLL.
//!
//! A literal assignment updates only occurrence-adjacent clause counters. Undo
//! restores the counters in reverse trail order. Unlike the standard MPFR DPLL,
//! neither Shannon branch materializes a Vec<Vec<i32>> cofactor. Canonical
//! reduced clauses are streamed only at the component-cache boundary.
//!
//! This is NOT CDCL and does not perform weight-unsafe SAT simplification.
use crate::{normalize, Clause, Outcome, PackedKey, Problem, Stats, C};
use hashbrown::HashMap;
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap};

pub(crate) struct TrailEngine<'a> {
    problem: &'a Problem,
    bits: u32,
    clauses: Vec<Clause>,
    occurrence: Vec<Vec<usize>>,
    assigned: Vec<i8>,
    trail: Vec<usize>,
    live: Vec<usize>,
    satisfied: Vec<usize>,
    rank: Vec<usize>,
    generation: Vec<u64>,
    sequence: u64,
    frequency: Vec<usize>,
    cache: HashMap<PackedKey, Outcome>,
    cache_limit: usize,
    node_budget: u64,
    pub(crate) stats: Stats,
    pub(crate) assignments: u64,
    pub(crate) cache_keys: u64,
    // Descendants may examine only their parent's still-live clause IDs.
    // No assignment can awaken a clause previously satisfied along this trail.
    pub(crate) use_parent_ids: bool,
}

fn slot(lit: i32) -> usize {
    2 * (lit.unsigned_abs() as usize) + usize::from(lit < 0)
}
fn root(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}
impl<'a> TrailEngine<'a> {
    pub(crate) fn new(
        problem: &'a Problem,
        bits: u32,
        cache_limit: usize,
        node_budget: u64,
    ) -> Self {
        let clauses = normalize(problem.clauses.clone());
        let mut occurrence = vec![Vec::new(); 2 * (problem.n + 1)];
        let mut live = Vec::with_capacity(clauses.len());
        for (cid, clause) in clauses.iter().enumerate() {
            live.push(clause.len());
            for &lit in clause {
                occurrence[slot(lit)].push(cid);
            }
        }
        Self {
            problem,
            bits,
            rank: vec![0; clauses.len()],
            generation: vec![0; clauses.len()],
            sequence: 0,
            satisfied: vec![0; clauses.len()],
            clauses,
            occurrence,
            assigned: vec![0; problem.n + 1],
            trail: Vec::with_capacity(problem.n),
            live,
            frequency: vec![0; problem.n + 1],
            cache: HashMap::new(),
            cache_limit,
            node_budget,
            stats: Stats::default(),
            assignments: 0,
            cache_keys: 0,
            use_parent_ids: false,
        }
    }

    /// One reversible variable assignment. Even if a clause becomes
    /// contradictory, finish the incidence updates so undo is exact.
    fn apply(&mut self, lit: i32, heap: Option<(&mut BinaryHeap<Reverse<usize>>, u64)>) -> bool {
        let var = lit.unsigned_abs() as usize;
        debug_assert_eq!(self.assigned[var], 0);
        self.assigned[var] = if lit > 0 { 1 } else { -1 };
        self.trail.push(var);
        self.assignments += 1;
        for &cid in &self.occurrence[slot(lit)] {
            self.satisfied[cid] += 1;
            self.live[cid] -= 1;
        }
        let mut conflict = false;
        let mut heap = heap;
        for &cid in &self.occurrence[slot(-lit)] {
            self.live[cid] -= 1;
            if self.satisfied[cid] == 0 && self.live[cid] == 0 {
                conflict = true;
            }
            if self.satisfied[cid] == 0 && self.live[cid] == 1 {
                if let Some((queue, generation)) = heap.as_mut() {
                    if self.generation[cid] == *generation {
                        queue.push(Reverse(self.rank[cid]));
                    }
                }
            }
        }
        !conflict
    }

    fn undo(&mut self, checkpoint: usize) {
        while self.trail.len() > checkpoint {
            let var = self.trail.pop().expect("nonempty trail");
            let lit = if self.assigned[var] > 0 {
                var as i32
            } else {
                -(var as i32)
            };
            for &cid in &self.occurrence[slot(lit)] {
                self.satisfied[cid] -= 1;
                self.live[cid] += 1;
            }
            for &cid in &self.occurrence[slot(-lit)] {
                self.live[cid] += 1;
            }
            self.assigned[var] = 0;
        }
    }

    /// Lexicographic order of the current residual literals, without
    /// allocating any child clauses. This reproduces normalize_child's
    /// canonical clause order even after removing several assigned literals.
    fn cmp_residual(&self, a: usize, b: usize) -> Ordering {
        let mut left = self.clauses[a]
            .iter()
            .filter(|&&lit| self.assigned[lit.unsigned_abs() as usize] == 0);
        let mut right = self.clauses[b]
            .iter()
            .filter(|&&lit| self.assigned[lit.unsigned_abs() as usize] == 0);
        loop {
            match (left.next(), right.next()) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(x), Some(y)) => {
                    let cmp = x.cmp(y);
                    if cmp != Ordering::Equal {
                        return cmp;
                    }
                }
            }
        }
    }

    /// Canonical ids are sorted and deduplicated by *reduced* literal bytes.
    /// Under a disconnected sub-scope, clauses from another component are
    /// deliberately excluded, just as the original split_components does.
    fn canonical_ids(&self, scope: &[usize], parent_ids: Option<&[usize]>) -> Option<Vec<usize>> {
        let mut ids = Vec::new();
        let mut visit = |cid: usize| {
            if self.satisfied[cid] != 0 {
                return true;
            }
            if self.live[cid] == 0 {
                return false;
            }
            let first = self.clauses[cid]
                .iter()
                .find(|&&lit| self.assigned[lit.unsigned_abs() as usize] == 0)
                .expect("live clause has an unassigned literal");
            if scope
                .binary_search(&(first.unsigned_abs() as usize))
                .is_ok()
            {
                ids.push(cid);
            }
            true
        };
        if let Some(parent) = parent_ids.filter(|_| self.use_parent_ids) {
            for &cid in parent {
                if !visit(cid) {
                    return None;
                }
            }
        } else {
            for cid in 0..self.clauses.len() {
                if !visit(cid) {
                    return None;
                }
            }
        }
        ids.sort_unstable_by(|&a, &b| self.cmp_residual(a, b));
        ids.dedup_by(|a, b| self.cmp_residual(*a, *b) == Ordering::Equal);
        Some(ids)
    }

    fn key(&mut self, scope: Vec<usize>, ids: &[usize]) -> PackedKey {
        self.cache_keys += 1;
        let mut literals = Vec::new();
        for &cid in ids {
            for &lit in &self.clauses[cid] {
                if self.assigned[lit.unsigned_abs() as usize] == 0 {
                    literals.push(lit);
                }
            }
            literals.push(0);
        }
        PackedKey { scope, literals }
    }
    fn save(&mut self, key: PackedKey, outcome: Outcome) -> Outcome {
        if self.cache.len() < self.cache_limit {
            self.cache.insert(key, outcome.clone());
        }
        outcome
    }

    fn components(&self, ids: &[usize], active: &[usize]) -> Vec<Vec<usize>> {
        let mut parents: Vec<_> = (0..=self.problem.n).collect();
        for &cid in ids {
            if self.satisfied[cid] != 0 {
                continue;
            }
            let mut first = None;
            for &lit in &self.clauses[cid] {
                let v = lit.unsigned_abs() as usize;
                if self.assigned[v] != 0 {
                    continue;
                }
                if let Some(first_var) = first {
                    let a = root(&mut parents, first_var);
                    let b = root(&mut parents, v);
                    parents[b] = a;
                } else {
                    first = Some(v);
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for &var in active {
            groups.entry(root(&mut parents, var)).or_default().push(var);
        }
        groups.into_values().collect()
    }

    pub(crate) fn count(&mut self, scope: Vec<usize>, depth: usize) -> Result<Outcome, String> {
        self.count_relevant(scope, depth, None)
    }

    fn count_relevant(
        &mut self,
        scope: Vec<usize>,
        depth: usize,
        parent_ids: Option<&[usize]>,
    ) -> Result<Outcome, String> {
        let checkpoint = self.trail.len();
        let answer = self.count_inner(scope, depth, parent_ids);
        self.undo(checkpoint);
        answer
    }

    fn count_inner(
        &mut self,
        scope: Vec<usize>,
        depth: usize,
        parent_ids: Option<&[usize]>,
    ) -> Result<Outcome, String> {
        if depth >= 512 {
            return Err("trail depth limit".into());
        }
        if self.stats.nodes >= self.node_budget {
            return Err("node limit".into());
        }
        self.stats.nodes += 1;
        let Some(ids) = self.canonical_ids(&scope, parent_ids) else {
            return Ok(Outcome::unsat(self.bits));
        };
        let key = self.key(scope.clone(), &ids);
        if let Some(hit) = self.cache.get(&key) {
            self.stats.cache_hits += 1;
            return Ok(hit.clone());
        }
        self.sequence += 1;
        let generation = self.sequence;
        let mut heap = BinaryHeap::<Reverse<usize>>::new();
        for (index, &cid) in ids.iter().enumerate() {
            self.rank[cid] = index;
            self.generation[cid] = generation;
            if self.live[cid] == 1 {
                heap.push(Reverse(index));
            }
        }
        let mut prefix = C::one(self.bits);
        while let Some(Reverse(index)) = heap.pop() {
            let cid = ids[index];
            if self.satisfied[cid] != 0 || self.live[cid] != 1 {
                continue;
            }
            let lit = *self.clauses[cid]
                .iter()
                .find(|&&lit| self.assigned[lit.unsigned_abs() as usize] == 0)
                .expect("live unit has one unassigned literal");
            let var = lit.unsigned_abs() as usize;
            let weights = &self.problem.weights[var - 1];
            let factor = if lit > 0 {
                &weights.positive
            } else {
                &weights.negative
            };
            prefix = prefix.mul(factor, self.bits);
            if !self.apply(lit, Some((&mut heap, generation))) {
                return Ok(self.save(key, Outcome::unsat(self.bits)));
            }
        }

        let mut frequency = std::mem::take(&mut self.frequency);
        if frequency.len() != self.problem.n + 1 {
            frequency.resize(self.problem.n + 1, 0);
        }
        frequency.fill(0);
        for &cid in &ids {
            if self.satisfied[cid] != 0 {
                continue;
            }
            for &lit in &self.clauses[cid] {
                if self.assigned[lit.unsigned_abs() as usize] == 0 {
                    frequency[lit.unsigned_abs() as usize] += 1;
                }
            }
        }
        let mut active = Vec::new();
        for &var in &scope {
            if self.assigned[var] != 0 {
                continue;
            }
            if frequency[var] > 0 {
                active.push(var);
            } else {
                let w = &self.problem.weights[var - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative, self.bits), self.bits);
            }
        }
        let pivot = active.iter().max_by_key(|&&var| frequency[var]).copied();
        self.frequency = frequency;
        if active.is_empty() {
            return Ok(self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix,
                },
            ));
        }
        let pieces = self.components(&ids, &active);
        if pieces.len() > 1 {
            self.stats.decompositions += 1;
            let mut value = C::one(self.bits);
            for part in pieces {
                let result = self.count_relevant(part, depth + 1, Some(&ids))?;
                if !result.sat {
                    return Ok(self.save(key, Outcome::unsat(self.bits)));
                }
                value = value.mul(&result.count, self.bits);
            }
            return Ok(self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix.mul(&value, self.bits),
                },
            ));
        }
        let pivot = pivot.ok_or("missing trail pivot")?;
        let child: Vec<usize> = active.into_iter().filter(|&v| v != pivot).collect();
        let checkpoint = self.trail.len();

        // Branch assignments mutate only occurrence-adjacent clause counters.
        // Neither branch creates a materialized child Vec<Vec<i32>>.
        let yes_ok = self.apply(pivot as i32, None);
        let yes_result = if yes_ok {
            self.count_relevant(child.clone(), depth + 1, Some(&ids))
        } else {
            Ok(Outcome::unsat(self.bits))
        };
        self.undo(checkpoint);
        let yes = yes_result?;

        let no_ok = self.apply(-(pivot as i32), None);
        let no_result = if no_ok {
            self.count_relevant(child, depth + 1, Some(&ids))
        } else {
            Ok(Outcome::unsat(self.bits))
        };
        self.undo(checkpoint);
        let no = no_result?;

        let weights = &self.problem.weights[pivot - 1];
        let yes_weighted = weights.positive.mul(&yes.count, self.bits);
        let no_weighted = weights.negative.mul(&no.count, self.bits);
        let value = prefix.mul(&yes_weighted.add(&no_weighted, self.bits), self.bits);
        Ok(self.save(
            key,
            Outcome {
                sat: yes.sat || no.sat,
                count: value,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, CachePolicy, Engine};

    fn verify(source: &str) {
        for bits in [100_u32, 333_u32] {
            let problem = parse_emitted(source, bits).unwrap();
            let mut control = Engine::new(&problem, bits, CachePolicy::HbBorrowed);
            control.cache_limit = 128;
            control.fast_assign = true;
            let expected = control
                .count(problem.clauses.clone(), (1..=problem.n).collect(), 0)
                .unwrap();
            let mut trail = TrailEngine::new(&problem, bits, 128, 100_000);
            let actual = trail.count((1..=problem.n).collect(), 0).unwrap();
            assert_eq!(actual.sat, expected.sat);
            assert_eq!(actual.count.re, expected.count.re, "real @ {bits}");
            assert_eq!(actual.count.im, expected.count.im, "imag @ {bits}");
            let mut parent_indexed = TrailEngine::new(&problem, bits, 128, 100_000);
            parent_indexed.use_parent_ids = true;
            let indexed = parent_indexed.count((1..=problem.n).collect(), 0).unwrap();
            assert_eq!(indexed.sat, actual.sat);
            assert_eq!(indexed.count.re, actual.count.re);
            assert_eq!(indexed.count.im, actual.count.im);
            assert_eq!(parent_indexed.stats.nodes, trail.stats.nodes);
            assert_eq!(parent_indexed.stats.cache_hits, trail.stats.cache_hits);
            assert_eq!(
                parent_indexed.stats.decompositions,
                trail.stats.decompositions
            );
            assert!(parent_indexed.assigned.iter().all(|&x| x == 0));
            assert_eq!(trail.trail.len(), 0);
            assert!(trail.assigned.iter().all(|&x| x == 0));
            for (cid, clause) in trail.clauses.iter().enumerate() {
                assert_eq!(trail.live[cid], clause.len());
                assert_eq!(trail.satisfied[cid], 0);
            }
        }
    }

    #[test]
    fn weighted_signed_complex_unit_chains_and_components() {
        verify(concat!(
            "p cnf 7 8\nc t wmc\n",
            "c p weight 1 0.5 0.25 0\n",
            "c p weight -1 -0.25 0.125 0\n",
            "c p weight 3 0.25 -0.5 0\n",
            "c p weight -3 -0.25 0 0\n",
            "1 -2 0\n-1 2 0\n-2 3 0\n",
            "4 -5 0\n-4 5 0\n6 7 0\n-6 -7 0\n",
            "2 3 0\n"
        ));
    }
    #[test]
    fn signed_sat_zero_and_boolean_unsat_remain_distinct() {
        verify("p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n");
        verify("p cnf 3 2\nc t wmc\n1 0\n-1 0\n");
        verify("p cnf 4 2\nc t wmc\n1 2 0\n-1 -2 0\n");
    }
    #[test]
    fn deterministic_small_dyadic_full_wmc_crosscheck() {
        let mut seed = 0x9e3779b97f4a7c15_u64;
        for case in 0..40 {
            let mut clauses = Vec::new();
            for _ in 0..(4 + case % 10) {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let n = 1 + (seed as usize % 4);
                let mut clause = Vec::new();
                for _ in 0..n {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let var = 1 + (seed as usize % 5);
                    clause.push(if seed & 4 == 0 {
                        var as i32
                    } else {
                        -(var as i32)
                    });
                }
                clauses.push(clause);
            }
            for bits in [100_u32, 333_u32] {
                let problem = Problem {
                    n: 5,
                    clauses: clauses.clone(),
                    weights: (0..5)
                        .map(|i| crate::Weights {
                            positive: C::decimal(bits, "0.5", "-0.25").unwrap(),
                            negative: C::decimal(
                                bits,
                                if i % 2 == 0 { "-0.25" } else { "0.125" },
                                "0.5",
                            )
                            .unwrap(),
                        })
                        .collect(),
                };
                let mut baseline = Engine::new(&problem, bits, CachePolicy::HbBorrowed);
                baseline.fast_assign = true;
                let old = baseline
                    .count(clauses.clone(), (1..=5).collect(), 0)
                    .unwrap();
                let mut trail = TrailEngine::new(&problem, bits, 256, 100_000);
                let new = trail.count((1..=5).collect(), 0).unwrap();
                assert_eq!(
                    (old.sat, &old.count.re, &old.count.im),
                    (new.sat, &new.count.re, &new.count.im),
                    "case {case} bits {bits}"
                );
                let mut incremental = TrailEngine::new(&problem, bits, 256, 100_000);
                incremental.use_parent_ids = true;
                let seen = incremental.count((1..=5).collect(), 0).unwrap();
                assert_eq!(
                    (old.sat, &old.count.re, &old.count.im),
                    (seen.sat, &seen.count.re, &seen.count.im),
                    "incremental clause ids case {case} bits {bits}"
                );
                assert_eq!(incremental.stats.nodes, trail.stats.nodes);
                assert_eq!(incremental.stats.cache_hits, trail.stats.cache_hits);
                assert_eq!(incremental.stats.decompositions, trail.stats.decompositions);
            }
        }
    }
}
