//! Research-only reversible source-clause weighted DPLL.
//!
//! A literal assignment updates only occurrence-adjacent clause counters. Undo
//! restores the counters in reverse trail order. Unlike the standard MPFR DPLL,
//! neither Shannon branch materializes a Vec<Vec<i32>> cofactor. Canonical
//! reduced clauses are streamed only at the component-cache boundary.
//!
//! This is NOT CDCL and does not perform weight-unsafe SAT simplification.
use crate::{normalize, Clause, Outcome, PackedKey, Problem, Stats, C};
use hashbrown::{hash_map::RawEntryMut, HashMap};
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap};
use std::hash::BuildHasher;

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
    // Research option: filter each reduced literal once into a dense arena,
    // then sort borrowed views and construct the same exact PackedKey.
    pub(crate) use_compact_residual: bool,
    // Cached exact-key hash is an optimization only; collisions still compare keys.
    pub(crate) use_prehashed_cache: bool,
    // On compact keys, fuse scope admission and literal compaction into
    // one source-clause traversal rather than scanning twice per node.
    pub(crate) use_fused_compact: bool,
    // Exact, order-preserving variable relabeling for identical weighted
    // components. A cache hit is permitted only when all positive AND
    // negative Complex MPFR weight values agree at the same precision.
    pub(crate) use_weight_class_cache: bool,
    // None: original all-scopes weight-class cache. Some(k): only
    // scopes with <=k variables pay the normalization cost. A zero
    // sentinel separates localized cache keys from full source keys.
    pub(crate) weight_cache_scope_max: Option<usize>,
    pub(crate) normalized_cache_keys: u64,
    pub(crate) source_cache_keys: u64,
    // Opt-in: compact signed residual literals directly in canonical local IDs.
    pub(crate) use_inline_weight_relabel: bool,
    // Exact weighted residual memo AFTER Boolean unit-closure, opt-in only.
    pub(crate) use_post_unit_cache: bool,
    pub(crate) post_unit_hits: u64,
    pub(crate) post_unit_visited: u64,
    pub(crate) post_unit_forced: u64,
    weight_classes: Vec<usize>,
    local_variable: Vec<i32>,
    local_generation: Vec<u64>,
    local_epoch: u64,
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
            use_compact_residual: false,
            use_prehashed_cache: false,
            use_fused_compact: false,
            use_weight_class_cache: false,
            weight_cache_scope_max: None,
            normalized_cache_keys: 0,
            source_cache_keys: 0,
            use_inline_weight_relabel: false,
            use_post_unit_cache: false,
            post_unit_hits: 0,
            post_unit_visited: 0,
            post_unit_forced: 0,
            weight_classes: Vec::new(),
            local_variable: vec![0; problem.n + 1],
            local_generation: vec![0; problem.n + 1],
            local_epoch: 0,
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

    // The standard path re-filters original clauses in each comparison and
    // again when it serializes the exact memoization key. This opt-in path
    // materializes only one contiguous literal arena (never child CNFs),
    // sorting small immutable spans of that arena instead.
    fn canonical_compact(
        &self,
        scope: &[usize],
        parent_ids: Option<&[usize]>,
        inline_weight_relabel: bool,
    ) -> Option<(Vec<usize>, Vec<i32>)> {
        #[derive(Clone, Copy)]
        struct View {
            cid: usize,
            start: usize,
            end: usize,
        }
        let mut flat = Vec::<i32>::new();
        let mut views = Vec::<View>::new();
        let mut visit = |cid: usize| {
            if self.satisfied[cid] != 0 {
                return true;
            }
            if self.live[cid] == 0 {
                return false;
            }
            let start = flat.len();
            if self.use_fused_compact {
                // A branch's first still-unassigned literal establishes
                // the same component/scope admission as the control.
                // Read each active source clause at most once and stop
                // immediately for a clause outside the current scope.
                let mut first_unassigned = true;
                for &lit in &self.clauses[cid] {
                    let variable = lit.unsigned_abs() as usize;
                    if self.assigned[variable] == 0 {
                        if first_unassigned {
                            if scope.binary_search(&variable).is_err() {
                                return true;
                            }
                            first_unassigned = false;
                        }
                        if inline_weight_relabel {
                            // The component scope is pre-indexed. All local
                            // ordinal IDs are positive and order-preserving,
                            // so canonical sorting/dedup remains unchanged.
                            let rank = self.local_variable[variable];
                            flat.push(if lit < 0 { -rank } else { rank });
                        } else {
                            flat.push(lit);
                        }
                    }
                }
                // A live, unsatisfied clause always has an unassigned
                // literal; a malformed counter must never be ignored.
                assert!(!first_unassigned, "live clause has no unassigned literal");
            } else {
                let first = self.clauses[cid]
                    .iter()
                    .find(|&&lit| self.assigned[lit.unsigned_abs() as usize] == 0)
                    .expect("live clause has an unassigned literal");
                if scope
                    .binary_search(&(first.unsigned_abs() as usize))
                    .is_err()
                {
                    return true;
                }
                for &lit in &self.clauses[cid] {
                    if self.assigned[lit.unsigned_abs() as usize] == 0 {
                        flat.push(lit);
                    }
                }
            }
            debug_assert_eq!(flat.len() - start, self.live[cid]);
            views.push(View {
                cid,
                start,
                end: flat.len(),
            });
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
        views.sort_unstable_by(|a, b| flat[a.start..a.end].cmp(&flat[b.start..b.end]));
        views.dedup_by(|a, b| flat[a.start..a.end] == flat[b.start..b.end]);
        let mut ids = Vec::with_capacity(views.len());
        let mut literals = Vec::with_capacity(flat.len() + views.len());
        for view in views {
            ids.push(view.cid);
            literals.extend_from_slice(&flat[view.start..view.end]);
            literals.push(0);
        }
        Some((ids, literals))
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
    fn save(&mut self, key: PackedKey, key_hash: u64, outcome: Outcome) -> Outcome {
        if self.cache.len() < self.cache_limit {
            if self.use_prehashed_cache {
                // Hash matches the HashMap's own hash builder; even in
                // a hash collision, the complete PackedKey is compared.
                match self
                    .cache
                    .raw_entry_mut()
                    .from_key_hashed_nocheck(key_hash, &key)
                {
                    RawEntryMut::Occupied(_) => {}
                    RawEntryMut::Vacant(entry) => {
                        entry.insert_hashed_nocheck(key_hash, key, outcome.clone());
                    }
                }
            } else {
                self.cache.insert(key, outcome.clone());
            }
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

    // The fingerprint is computed once per experiment (not for the Fused
    // control). A full binary MPFR mantissa, exponent, sign and signed zero
    // are preserved by radix-16 formatting with >= ceil(bits/4)+2 digits.
    // This is a conservative weight-class *identity*, not an approximate
    // numerical/tolerance clustering rule.
    fn initialize_weight_classes(&mut self) {
        if !self.weight_classes.is_empty() {
            return;
        }
        let digits = (self.bits.div_ceil(4) + 3) as usize;
        let mut known = BTreeMap::new();
        self.weight_classes = vec![0; self.problem.n + 1];
        for (index, weight) in self.problem.weights.iter().enumerate() {
            let signature = (
                weight.positive.re.to_string_radix(16, Some(digits)),
                weight.positive.im.to_string_radix(16, Some(digits)),
                weight.negative.re.to_string_radix(16, Some(digits)),
                weight.negative.im.to_string_radix(16, Some(digits)),
            );
            let id = if let Some(&existing) = known.get(&signature) {
                existing
            } else {
                let next = known.len() + 1;
                known.insert(signature, next);
                next
            };
            self.weight_classes[index + 1] = id;
        }
    }

    // Different source variable IDs may share a PackedKey only when the
    // order-preserving positional relabeling produces the same literal
    // topology AND every corresponding variable has the same exact signed
    // Complex MPFR weights. Free scope variables stay in the signature.
    // No automorphism search; arbitrary reordering is NOT claimed safe.
    fn prepare_weighted_scope(&mut self, scope: &[usize]) -> Result<Vec<usize>, String> {
        self.local_epoch += 1;
        let stamp = self.local_epoch;
        let mut classes = Vec::with_capacity(scope.len());
        for (offset, &variable) in scope.iter().enumerate() {
            if variable == 0 || variable > self.problem.n || offset >= i32::MAX as usize {
                return Err("invalid weighted component scope".into());
            }
            self.local_variable[variable] = (offset + 1) as i32;
            self.local_generation[variable] = stamp;
            classes.push(self.weight_classes[variable]);
        }
        Ok(classes)
    }

    fn relabel_weighted_key(&mut self, scope: &[usize], key: &mut PackedKey) -> Result<(), String> {
        let classes = self.prepare_weighted_scope(scope)?;
        let stamp = self.local_epoch;
        for literal in &mut key.literals {
            if *literal == 0 {
                continue;
            }
            let original = literal.unsigned_abs() as usize;
            if original == 0
                || original > self.problem.n
                || self.local_generation[original] != stamp
            {
                return Err("residual outside weighted component scope".into());
            }
            let rank = self.local_variable[original];
            *literal = if *literal < 0 { -rank } else { rank };
        }
        key.scope = classes;
        Ok(())
    }

    pub(crate) fn count(&mut self, scope: Vec<usize>, depth: usize) -> Result<Outcome, String> {
        if self.use_weight_class_cache {
            self.initialize_weight_classes();
        }
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
        if self.use_post_unit_cache {
            return self.count_inner_post_unit(scope, depth, parent_ids);
        }
        if depth >= 512 {
            return Err("trail depth limit".into());
        }
        if self.stats.nodes >= self.node_budget {
            return Err("node limit".into());
        }
        self.stats.nodes += 1;
        let localized = self.use_weight_class_cache
            && self
                .weight_cache_scope_max
                .is_none_or(|max| scope.len() <= max);
        let inline_relabel = localized && self.use_inline_weight_relabel;
        let class_scope = if inline_relabel {
            Some(self.prepare_weighted_scope(&scope)?)
        } else {
            None
        };
        let (ids, mut key) = if self.use_compact_residual {
            let Some((ids, literals)) = self.canonical_compact(&scope, parent_ids, inline_relabel)
            else {
                return Ok(Outcome::unsat(self.bits));
            };
            self.cache_keys += 1;
            (
                ids,
                PackedKey {
                    scope: scope.clone(),
                    literals,
                },
            )
        } else {
            let Some(ids) = self.canonical_ids(&scope, parent_ids) else {
                return Ok(Outcome::unsat(self.bits));
            };
            let key = self.key(scope.clone(), &ids);
            (ids, key)
        };
        if localized {
            if let Some(classes) = class_scope {
                // The compact arena was already emitted with local ordinal
                // signed literals, so no second full PackedKey traversal.
                key.scope = classes;
            } else {
                self.relabel_weighted_key(&scope, &mut key)?;
            }
            if self.weight_cache_scope_max.is_some() {
                // Original source scopes contain only variable IDs 1..n.
                // This tag cannot equal any unmodified source scope,
                // even if weight-class IDs happen to equal variable IDs.
                key.scope.insert(0, 0);
            }
            self.normalized_cache_keys += 1;
        } else {
            self.source_cache_keys += 1;
        }
        let key_hash = if self.use_prehashed_cache {
            self.cache.hasher().hash_one(&key)
        } else {
            0
        };
        let hit = if self.use_prehashed_cache {
            self.cache
                .raw_entry()
                .from_key_hashed_nocheck(key_hash, &key)
                .map(|(_, value)| value)
        } else {
            self.cache.get(&key)
        };
        if let Some(hit) = hit {
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
                return Ok(self.save(key, key_hash, Outcome::unsat(self.bits)));
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
                key_hash,
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
                    return Ok(self.save(key, key_hash, Outcome::unsat(self.bits)));
                }
                value = value.mul(&result.count, self.bits);
            }
            return Ok(self.save(
                key,
                key_hash,
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
            key_hash,
            Outcome {
                sat: yes.sat || no.sat,
                count: value,
            },
        ))
    }
    fn finish_post_unit(
        &mut self,
        unit_prefix: &C,
        key: PackedKey,
        key_hash: u64,
        post: Outcome,
    ) -> Outcome {
        let saved = self.save(key, key_hash, post);
        Outcome {
            sat: saved.sat,
            count: unit_prefix.mul(&saved.count, self.bits),
        }
    }

    // Independent research counting path: memoize exact signed-complex
    // residuals AFTER unit closure. The old pre-unit cache remains unchanged.
    fn count_inner_post_unit(
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
        // Preserve the same canonical source order for the unit heap.
        // No pre-closure memo key is looked up or inserted in this arm.
        let Some((ids, _)) = self.canonical_compact(&scope, parent_ids, false) else {
            return Ok(Outcome::unsat(self.bits));
        };
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
            self.post_unit_forced += 1;
            if !self.apply(lit, Some((&mut heap, generation))) {
                return Ok(Outcome::unsat(self.bits));
            }
        }

        // Prefix of implied unit assignments belongs to this path only.
        // Never store it under the canonical post-closure residual key.
        let unit_prefix = prefix;
        let mut prefix = C::one(self.bits);
        let post_scope: Vec<usize> = scope
            .iter()
            .copied()
            .filter(|&var| self.assigned[var] == 0)
            .collect();
        let localized = self.use_weight_class_cache
            && self
                .weight_cache_scope_max
                .is_none_or(|max| post_scope.len() <= max);
        let inline_relabel = localized && self.use_inline_weight_relabel;
        let class_scope = if inline_relabel {
            Some(self.prepare_weighted_scope(&post_scope)?)
        } else {
            None
        };
        let Some((_, literals)) = self.canonical_compact(&post_scope, Some(&ids), inline_relabel)
        else {
            return Ok(Outcome::unsat(self.bits));
        };
        let mut key = PackedKey {
            scope: post_scope.clone(),
            literals,
        };
        self.cache_keys += 1;
        self.post_unit_visited += 1;
        if localized {
            if let Some(classes) = class_scope {
                key.scope = classes;
            } else {
                self.relabel_weighted_key(&post_scope, &mut key)?;
            }
            if self.weight_cache_scope_max.is_some() {
                key.scope.insert(0, 0);
            }
            self.normalized_cache_keys += 1;
        } else {
            self.source_cache_keys += 1;
        }
        let key_hash = self.cache.hasher().hash_one(&key);
        if let Some(hit) = self
            .cache
            .raw_entry()
            .from_key_hashed_nocheck(key_hash, &key)
            .map(|(_, value)| value)
        {
            self.stats.cache_hits += 1;
            self.post_unit_hits += 1;
            return Ok(Outcome {
                sat: hit.sat,
                count: unit_prefix.mul(&hit.count, self.bits),
            });
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
            return Ok(self.finish_post_unit(
                &unit_prefix,
                key,
                key_hash,
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
                    return Ok(self.finish_post_unit(
                        &unit_prefix,
                        key,
                        key_hash,
                        Outcome::unsat(self.bits),
                    ));
                }
                value = value.mul(&result.count, self.bits);
            }
            return Ok(self.finish_post_unit(
                &unit_prefix,
                key,
                key_hash,
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
        Ok(self.finish_post_unit(
            &unit_prefix,
            key,
            key_hash,
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
            let mut compact = TrailEngine::new(&problem, bits, 128, 100_000);
            compact.use_parent_ids = true;
            compact.use_compact_residual = true;
            let compressed = compact.count((1..=problem.n).collect(), 0).unwrap();
            assert_eq!(compressed.sat, indexed.sat);
            assert_eq!(&compressed.count.re, &indexed.count.re);
            assert_eq!(&compressed.count.im, &indexed.count.im);
            assert_eq!(compact.stats.nodes, parent_indexed.stats.nodes);
            assert_eq!(compact.stats.cache_hits, parent_indexed.stats.cache_hits);
            assert_eq!(
                compact.stats.decompositions,
                parent_indexed.stats.decompositions
            );
            assert!(compact.assigned.iter().all(|&x| x == 0));
            // The same complete PackedKey must still guard every lookup.
            // Change cache limits to exercise vacant, occupied and
            // fully disabled memoization at two MPFR precisions.
            for capacity in [0_usize, 1, 128] {
                let mut old_cache = TrailEngine::new(&problem, bits, capacity, 100_000);
                old_cache.use_parent_ids = true;
                old_cache.use_compact_residual = true;
                let original = old_cache.count((1..=problem.n).collect(), 0).unwrap();
                let mut once = TrailEngine::new(&problem, bits, capacity, 100_000);
                once.use_parent_ids = true;
                once.use_compact_residual = true;
                once.use_prehashed_cache = true;
                let hashed = once.count((1..=problem.n).collect(), 0).unwrap();
                assert_eq!(hashed.sat, original.sat);
                assert_eq!(&hashed.count.re, &original.count.re);
                assert_eq!(&hashed.count.im, &original.count.im);
                assert_eq!(once.stats.nodes, old_cache.stats.nodes);
                assert_eq!(once.stats.cache_hits, old_cache.stats.cache_hits);
                assert_eq!(once.stats.decompositions, old_cache.stats.decompositions);
                assert!(once.assigned.iter().all(|&x| x == 0));
                // Same full WMC and traversal when compact scope admission
                // and reduced-literal filtering are fused into one pass.
                let mut fused = TrailEngine::new(&problem, bits, capacity, 100_000);
                fused.use_parent_ids = true;
                fused.use_compact_residual = true;
                fused.use_prehashed_cache = true;
                fused.use_fused_compact = true;
                let candidate = fused.count((1..=problem.n).collect(), 0).unwrap();
                assert_eq!(candidate.sat, hashed.sat);
                assert_eq!(&candidate.count.re, &hashed.count.re);
                assert_eq!(&candidate.count.im, &hashed.count.im);
                assert_eq!(fused.stats.nodes, once.stats.nodes);
                assert_eq!(fused.stats.cache_hits, once.stats.cache_hits);
                assert_eq!(fused.stats.decompositions, once.stats.decompositions);
                assert!(fused.assigned.iter().all(|&x| x == 0));
            }
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
                let mut compact = TrailEngine::new(&problem, bits, 256, 100_000);
                compact.use_parent_ids = true;
                compact.use_compact_residual = true;
                let compressed = compact.count((1..=5).collect(), 0).unwrap();
                assert_eq!(compressed.sat, seen.sat);
                assert_eq!(&compressed.count.re, &seen.count.re);
                assert_eq!(&compressed.count.im, &seen.count.im);
                let mut once = TrailEngine::new(&problem, bits, 256, 100_000);
                once.use_parent_ids = true;
                once.use_compact_residual = true;
                once.use_prehashed_cache = true;
                let hashed = once.count((1..=5).collect(), 0).unwrap();
                assert_eq!(
                    (hashed.sat, &hashed.count.re, &hashed.count.im),
                    (compressed.sat, &compressed.count.re, &compressed.count.im)
                );
                assert_eq!(once.stats.nodes, compact.stats.nodes);
                assert_eq!(once.stats.cache_hits, compact.stats.cache_hits);
                assert_eq!(once.stats.decompositions, compact.stats.decompositions);
                let mut fused = TrailEngine::new(&problem, bits, 256, 100_000);
                fused.use_parent_ids = true;
                fused.use_compact_residual = true;
                fused.use_prehashed_cache = true;
                fused.use_fused_compact = true;
                let candidate = fused.count((1..=5).collect(), 0).unwrap();
                assert_eq!(
                    (candidate.sat, &candidate.count.re, &candidate.count.im),
                    (hashed.sat, &hashed.count.re, &hashed.count.im)
                );
                assert_eq!(fused.stats.nodes, once.stats.nodes);
                assert_eq!(fused.stats.cache_hits, once.stats.cache_hits);
                assert_eq!(fused.stats.decompositions, once.stats.decompositions);
                assert_eq!(compact.stats.nodes, incremental.stats.nodes);
                assert_eq!(compact.stats.cache_hits, incremental.stats.cache_hits);
                assert_eq!(
                    compact.stats.decompositions,
                    incremental.stats.decompositions
                );
                assert_eq!(incremental.stats.nodes, trail.stats.nodes);
                assert_eq!(incremental.stats.cache_hits, trail.stats.cache_hits);
                assert_eq!(incremental.stats.decompositions, trail.stats.decompositions);
            }
        }
    }
    fn weighted_component_source(second_positive_real: &str) -> String {
        let mut result = String::from("p cnf 6 6\nc t wmc\n");
        for var in 1..=6 {
            let pair = (var - 1) % 3;
            let p_real = if var == 4 {
                second_positive_real
            } else {
                match pair {
                    0 => "0.5",
                    1 => "0.25",
                    _ => "-0.125",
                }
            };
            let p_im = match pair {
                0 => "0.25",
                1 => "-0.125",
                _ => "0.5",
            };
            let n_real = match pair {
                0 => "-0.25",
                1 => "0.125",
                _ => "0.5",
            };
            let n_im = match pair {
                0 => "0.5",
                1 => "0.25",
                _ => "-0.125",
            };
            result.push_str(&format!("c p weight {var} {p_real} {p_im} 0\n"));
            result.push_str(&format!("c p weight -{var} {n_real} {n_im} 0\n"));
        }
        result.push_str("1 2 0\n-1 3 0\n-2 -3 0\n");
        result.push_str("4 5 0\n-4 6 0\n-5 -6 0\n");
        result
    }

    #[test]
    fn weight_class_remap_reuses_disjoint_identical_complex_components() {
        for bits in [100_u32, 333_u32] {
            let source = weighted_component_source("0.5");
            let problem = parse_emitted(&source, bits).unwrap();
            for cache_limit in [0_usize, 1, 128] {
                let mut independent = Engine::new(&problem, bits, CachePolicy::HbBorrowed);
                independent.cache_limit = cache_limit;
                independent.fast_assign = true;
                let expected = independent
                    .count(problem.clauses.clone(), (1..=problem.n).collect(), 0)
                    .unwrap();
                let mut baseline = TrailEngine::new(&problem, bits, cache_limit, 100_000);
                baseline.use_parent_ids = true;
                baseline.use_compact_residual = true;
                baseline.use_prehashed_cache = true;
                baseline.use_fused_compact = true;
                let old = baseline.count((1..=problem.n).collect(), 0).unwrap();
                let mut candidate = TrailEngine::new(&problem, bits, cache_limit, 100_000);
                candidate.use_parent_ids = true;
                candidate.use_compact_residual = true;
                candidate.use_prehashed_cache = true;
                candidate.use_fused_compact = true;
                candidate.use_weight_class_cache = true;
                let actual = candidate.count((1..=problem.n).collect(), 0).unwrap();
                assert_eq!(
                    (actual.sat, &actual.count.re, &actual.count.im),
                    (expected.sat, &expected.count.re, &expected.count.im)
                );
                assert_eq!(
                    (actual.sat, &actual.count.re, &actual.count.im),
                    (old.sat, &old.count.re, &old.count.im)
                );
                assert!(candidate.assigned.iter().all(|&x| x == 0));
                assert!(candidate.trail.is_empty());
                assert_eq!(candidate.weight_classes[1], candidate.weight_classes[4]);
                assert_eq!(candidate.weight_classes[2], candidate.weight_classes[5]);
                assert_eq!(candidate.weight_classes[3], candidate.weight_classes[6]);
                if cache_limit == 128 {
                    assert!(candidate.stats.cache_hits > baseline.stats.cache_hits);
                    assert!(candidate.stats.nodes < baseline.stats.nodes);
                }
            }
        }
    }

    #[test]
    fn weighted_component_remap_rejects_different_signed_mpfr_weights() {
        for bits in [100_u32, 333_u32] {
            // The two disconnected components share precisely the clause
            // graph but variable 4 has a different positive Complex weight.
            // They must NOT be merged in the cache.
            let source = weighted_component_source("0.125");
            let problem = parse_emitted(&source, bits).unwrap();
            let mut baseline = TrailEngine::new(&problem, bits, 128, 100_000);
            baseline.use_parent_ids = true;
            baseline.use_compact_residual = true;
            baseline.use_prehashed_cache = true;
            baseline.use_fused_compact = true;
            let old = baseline.count((1..=6).collect(), 0).unwrap();
            let mut candidate = TrailEngine::new(&problem, bits, 128, 100_000);
            candidate.use_parent_ids = true;
            candidate.use_compact_residual = true;
            candidate.use_prehashed_cache = true;
            candidate.use_fused_compact = true;
            candidate.use_weight_class_cache = true;
            let actual = candidate.count((1..=6).collect(), 0).unwrap();
            assert_ne!(candidate.weight_classes[1], candidate.weight_classes[4]);
            assert_eq!(actual.sat, old.sat);
            assert_eq!(actual.count.re, old.count.re);
            assert_eq!(actual.count.im, old.count.im);
        }
    }

    #[test]
    fn weighted_component_remap_matches_signed_random_dyadic_cnf() {
        let mut seed = 0x9e3779b97f4a7c15_u64;
        for bits in [100_u32, 333_u32] {
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
                let problem = Problem {
                    n: 5,
                    clauses,
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
                for cap in [0_usize, 128] {
                    let mut baseline = TrailEngine::new(&problem, bits, cap, 100_000);
                    baseline.use_parent_ids = true;
                    baseline.use_compact_residual = true;
                    baseline.use_prehashed_cache = true;
                    baseline.use_fused_compact = true;
                    let old = baseline.count((1..=5).collect(), 0).unwrap();
                    let mut candidate = TrailEngine::new(&problem, bits, cap, 100_000);
                    candidate.use_parent_ids = true;
                    candidate.use_compact_residual = true;
                    candidate.use_prehashed_cache = true;
                    candidate.use_fused_compact = true;
                    candidate.use_weight_class_cache = true;
                    let actual = candidate.count((1..=5).collect(), 0).unwrap();
                    assert_eq!(
                        (actual.sat, actual.count.re, actual.count.im),
                        (old.sat, old.count.re, old.count.im),
                        "source {case}, bits {bits}, capacity {cap}"
                    );
                    assert!(candidate.assigned.iter().all(|&x| x == 0));
                }
            }
        }
    }
    #[test]
    fn small_component_admission_reuses_matching_weighted_components() {
        for bits in [100_u32, 333_u32] {
            let source = weighted_component_source("0.5");
            let problem = parse_emitted(&source, bits).unwrap();
            let mut previous = TrailEngine::new(&problem, bits, 128, 100_000);
            previous.use_parent_ids = true;
            previous.use_compact_residual = true;
            previous.use_prehashed_cache = true;
            previous.use_fused_compact = true;
            let expected = previous.count((1..=6).collect(), 0).unwrap();
            for max_vars in [3_usize, 16, 64] {
                let mut candidate = TrailEngine::new(&problem, bits, 128, 100_000);
                candidate.use_parent_ids = true;
                candidate.use_compact_residual = true;
                candidate.use_prehashed_cache = true;
                candidate.use_fused_compact = true;
                candidate.use_weight_class_cache = true;
                candidate.weight_cache_scope_max = Some(max_vars);
                let actual = candidate.count((1..=6).collect(), 0).unwrap();
                assert_eq!(
                    (actual.sat, &actual.count.re, &actual.count.im),
                    (expected.sat, &expected.count.re, &expected.count.im)
                );
                assert!(candidate.normalized_cache_keys > 0);
                if max_vars == 3 {
                    assert!(candidate.source_cache_keys > 0);
                }
                assert!(candidate.stats.nodes < previous.stats.nodes);
                assert!(candidate.assigned.iter().all(|&x| x == 0));
            }
        }
    }

    #[test]
    fn small_component_admission_does_not_alias_source_and_local_keys() {
        let mut seed = 0x9e3779b97f4a7c15_u64;
        for bits in [100_u32, 333_u32] {
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
                let problem = Problem {
                    n: 5,
                    clauses,
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
                let mut control = TrailEngine::new(&problem, bits, 128, 100_000);
                control.use_parent_ids = true;
                control.use_compact_residual = true;
                control.use_prehashed_cache = true;
                control.use_fused_compact = true;
                let expected = control.count((1..=5).collect(), 0).unwrap();
                for max_vars in [1_usize, 2, 3, 16, 64] {
                    let mut candidate = TrailEngine::new(&problem, bits, 128, 100_000);
                    candidate.use_parent_ids = true;
                    candidate.use_compact_residual = true;
                    candidate.use_prehashed_cache = true;
                    candidate.use_fused_compact = true;
                    candidate.use_weight_class_cache = true;
                    candidate.weight_cache_scope_max = Some(max_vars);
                    let actual = candidate.count((1..=5).collect(), 0).unwrap();
                    assert_eq!(
                        (actual.sat, &actual.count.re, &actual.count.im),
                        (expected.sat, &expected.count.re, &expected.count.im),
                        "case {case}, bits {bits}, max_vars {max_vars}"
                    );
                    assert!(candidate.assigned.iter().all(|&x| x == 0));
                }
            }
        }
    }
    #[test]
    fn inline_weight_relabel_exactly_matches_standalone_weight_cache() {
        let input = concat!(
            "p cnf 6 4\nc t wmc\n",
            "1 2 0\n-1 -2 0\n",
            "4 5 0\n-4 -5 0\n"
        );
        for bits in [100_u32, 333_u32] {
            let p = parse_emitted(input, bits).unwrap();
            for max_scope in [None, Some(1_usize), Some(2), Some(16)] {
                let mut old = TrailEngine::new(&p, bits, 128, 100_000);
                old.use_parent_ids = true;
                old.use_compact_residual = true;
                old.use_prehashed_cache = true;
                old.use_fused_compact = true;
                old.use_weight_class_cache = true;
                old.weight_cache_scope_max = max_scope;
                let expected = old.count((1..=p.n).collect(), 0).unwrap();
                let mut inline = TrailEngine::new(&p, bits, 128, 100_000);
                inline.use_parent_ids = true;
                inline.use_compact_residual = true;
                inline.use_prehashed_cache = true;
                inline.use_fused_compact = true;
                inline.use_weight_class_cache = true;
                inline.weight_cache_scope_max = max_scope;
                inline.use_inline_weight_relabel = true;
                let actual = inline.count((1..=p.n).collect(), 0).unwrap();
                assert_eq!(actual.sat, expected.sat);
                assert_eq!(actual.count.re, expected.count.re);
                assert_eq!(actual.count.im, expected.count.im);
                assert_eq!(inline.stats.nodes, old.stats.nodes);
                assert_eq!(inline.stats.cache_hits, old.stats.cache_hits);
                assert_eq!(inline.stats.decompositions, old.stats.decompositions);
                assert_eq!(inline.normalized_cache_keys, old.normalized_cache_keys);
                assert_eq!(inline.source_cache_keys, old.source_cache_keys);
                assert!(inline.trail.is_empty());
            }
        }
    }
    #[test]
    fn post_unit_cache_matches_independent_complex_counting() {
        let fixtures = [
            concat!(
                "p cnf 6 6\nc t wmc\n",
                "c p weight 1 0.5 0.25 0\n",
                "c p weight -1 -0.25 0.125 0\n",
                "c p weight 3 0.25 -0.5 0\n",
                "c p weight -3 -0.25 0 0\n",
                "1 3 0\n-1 4 0\n3 5 0\n4 5 0\n5 -6 0\n-5 6 0\n"
            ),
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 4 3\nc t wmc\n1 -2 0\n-1 2 0\n2 3 4 0\n",
        ];
        for bits in [100_u32, 333] {
            for input in fixtures {
                let p = parse_emitted(input, bits).unwrap();
                let mut independent = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                independent.fast_assign = true;
                let expected = independent
                    .count(p.clauses.clone(), (1..=p.n).collect(), 0)
                    .unwrap();
                for capacity in [0_usize, 1, 128] {
                    for max_scope in [None, Some(2_usize)] {
                        let mut post = TrailEngine::new(&p, bits, capacity, 100_000);
                        post.use_parent_ids = true;
                        post.use_compact_residual = true;
                        post.use_prehashed_cache = true;
                        post.use_fused_compact = true;
                        post.use_weight_class_cache = true;
                        post.weight_cache_scope_max = max_scope;
                        post.use_inline_weight_relabel = true;
                        post.use_post_unit_cache = true;
                        let actual = post.count((1..=p.n).collect(), 0).unwrap();
                        assert_eq!(actual.sat, expected.sat);
                        assert_eq!(actual.count.re, expected.count.re);
                        assert_eq!(actual.count.im, expected.count.im);
                        assert!(post.assigned.iter().all(|&x| x == 0));
                        assert!(post.trail.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn post_unit_cache_randomized_signed_dyadic_full_wmc() {
        let mut seed = 0x9e3779b97f4a7c15_u64;
        for bits in [100_u32, 333] {
            for case in 0..40 {
                let mut clauses = Vec::new();
                for _ in 0..(4 + case % 10) {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let n = 1 + seed as usize % 4;
                    let mut clause = Vec::new();
                    for _ in 0..n {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let var = 1 + seed as usize % 5;
                        clause.push(if seed & 4 == 0 {
                            var as i32
                        } else {
                            -(var as i32)
                        });
                    }
                    clauses.push(clause);
                }
                let p = Problem {
                    n: 5,
                    clauses,
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
                let mut old = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                old.fast_assign = true;
                let expected = old.count(p.clauses.clone(), (1..=5).collect(), 0).unwrap();
                let mut post = TrailEngine::new(&p, bits, 256, 100_000);
                post.use_parent_ids = true;
                post.use_compact_residual = true;
                post.use_prehashed_cache = true;
                post.use_fused_compact = true;
                post.use_weight_class_cache = true;
                post.use_inline_weight_relabel = true;
                post.use_post_unit_cache = true;
                let actual = post.count((1..=5).collect(), 0).unwrap();
                assert_eq!(actual.sat, expected.sat, "SAT case {case} bits {bits}");
                assert_eq!(
                    actual.count.re, expected.count.re,
                    "real case {case} bits {bits}"
                );
                assert_eq!(
                    actual.count.im, expected.count.im,
                    "imag case {case} bits {bits}"
                );
                assert!(post.assigned.iter().all(|&x| x == 0));
            }
        }
    }
}
