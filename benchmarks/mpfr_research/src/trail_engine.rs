//! Research-only reversible source-clause weighted DPLL.
//!
//! A literal assignment updates only occurrence-adjacent clause counters. Undo
//! restores the counters in reverse trail order. Unlike the standard MPFR DPLL,
//! neither Shannon branch materializes a Vec<Vec<i32>> cofactor. Canonical
//! reduced clauses are streamed only at the component-cache boundary.
//!
//! This is NOT CDCL and does not perform weight-unsafe SAT simplification.
use crate::{
    factor_variable_elimination, normalize, separator_pivot, Clause, Outcome, PackedKey, Problem,
    Stats, C,
};
use hashbrown::{hash_map::RawEntryMut, HashMap};
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap};
use std::hash::BuildHasher;
use std::time::Instant;

#[derive(Clone, Copy)]
struct ResidualKeyView {
    cid: usize,
    start: usize,
    end: usize,
}

// Bound retained capacity: no stale child CNF/MPFR state is retained.
const KEY_ARENA_MAX_LITERALS: usize = 131_072;
const KEY_ARENA_MAX_VIEWS: usize = 16_384;

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
    // Opt-in: project the already canonical pre-unit clauses instead of
    // re-reading all original source clauses to build post-unit key.
    pub(crate) use_projected_post_key: bool,
    // Opt-in post-unit frequency/component union from the same clause traversal.
    pub(crate) use_fused_post_analysis: bool,
    // Exact opt-in separator-aware pivot; the source CNF and weights remain
    // unchanged. Bounded probes return to Frequency when inconclusive.
    pub(crate) use_articulation_pivot: bool,
    // Research-only exact Factor-VE after reversible DPLL unit closure.
    // At most eight small residual probes per full solve.
    pub(crate) use_residual_factor_ve: bool,
    // Independent research arm: after exact DSU component decomposition,
    // try bounded dense signed-complex WMC on small *whole components*.
    // Other components stay on original Trail and exact component cache.
    pub(crate) use_component_local_ve: bool,
    // Research-only source-exact compact DSU over still-live variables.
    // Unlike the existing n+1 DSU, untouched/assigned source vars are
    // not initialized in every recursive component traversal.
    pub(crate) use_scoped_component_dsu: bool,
    // Research-only: bypass sort/dedup if original reduced clause spans
    // are provably already strictly increasing. Duplicates/inversions
    // always execute the exact original sort/dedup path.
    pub(crate) use_ordered_key_fastpath: bool,
    // Exact opt-in reuse of transient flat literals + clause views only.
    // Cache-owned canonical PackedKey remains independently allocated.
    pub(crate) use_reusable_key_arena: bool,
    // Research-only O(1) membership using the exact current scope stamp
    // installed by prepare_weighted_scope immediately before key creation.
    // This never changes variable ordering or signed MPFR weight classes.
    pub(crate) use_scope_stamp_admission: bool,
    pub(crate) scope_stamp_probes: u64,
    // Reversible, source-owned live-clause bitmap: a bit is set exactly
    // when satisfied[cid] == 0. Never changes original clause or literal
    // order, weight classes, or cache-key ownership. Opt-in only.
    active_clause_bits: Option<Vec<u64>>,
    pub(crate) active_bitmap_key_visits: u64,
    key_arena_flat: Vec<i32>,
    key_arena_views: Vec<ResidualKeyView>,
    pub(crate) key_arena_calls: u64,
    pub(crate) key_arena_flat_reuses: u64,
    pub(crate) key_arena_view_reuses: u64,
    pub(crate) ordered_key_probes: u64,
    pub(crate) ordered_key_hits: u64,
    // Disabled by default: stage timers measure opt-in instrumentation cost,
    // never a claim about the uninstrumented solver's stage distribution.
    pub(crate) stage_profile: bool,
    pub(crate) stage_key_ns: u128,
    // Key subphases are inclusive within stage_key_ns, never an
    // additional independent share of full native solver WallTime.
    pub(crate) stage_key_materialize_ns: u128,
    pub(crate) stage_key_order_ns: u128,
    pub(crate) stage_key_pack_ns: u128,
    pub(crate) stage_lookup_ns: u128,
    pub(crate) stage_unit_ns: u128,
    pub(crate) stage_frequency_ns: u128,
    pub(crate) stage_components_ns: u128,
    pub(crate) component_dsu_calls: u64,
    pub(crate) component_dsu_slots: u64,
    scoped_component_index: Vec<usize>,
    scoped_component_stamp: Vec<u64>,
    scoped_component_epoch: u64,
    pub(crate) component_ve_attempts: u64,
    pub(crate) component_ve_completed: u64,
    pub(crate) component_ve_capped: u64,
    pub(crate) component_ve_products: u64,
    pub(crate) component_ve_scans: u64,
    pub(crate) residual_ve_attempts: u64,
    pub(crate) residual_ve_completed: u64,
    pub(crate) residual_ve_capped: u64,
    pub(crate) residual_ve_products: u64,
    pub(crate) residual_ve_max_width: usize,
    // Root-only arm: inspect one pivot, not every recursive residual.
    pub(crate) articulation_root_only: bool,
    // Research-only fixed primal bag-centroid score. Does not modify
    // clauses/weights/cache keys or remove either weighted branch.
    pub(crate) primal_scores: Option<Vec<u16>>,
    pub(crate) primal_root_only: bool,
    pub(crate) primal_scans: u64,
    pub(crate) primal_overrides: u64,
    pub(crate) separator_scans: u64,
    pub(crate) separator_candidates: u64,
    pub(crate) separator_chosen: u64,
    pub(crate) separator_skipped: u64,
    pub(crate) reused_unforced_keys: u64,
    pub(crate) projected_forced_keys: u64,
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
            use_projected_post_key: false,
            use_fused_post_analysis: false,
            use_articulation_pivot: false,
            use_residual_factor_ve: false,
            use_component_local_ve: false,
            use_scoped_component_dsu: false,
            use_ordered_key_fastpath: false,
            use_reusable_key_arena: false,
            use_scope_stamp_admission: false,
            scope_stamp_probes: 0,
            active_clause_bits: None,
            active_bitmap_key_visits: 0,
            key_arena_flat: Vec::new(),
            key_arena_views: Vec::new(),
            key_arena_calls: 0,
            key_arena_flat_reuses: 0,
            key_arena_view_reuses: 0,
            ordered_key_probes: 0,
            ordered_key_hits: 0,
            stage_profile: false,
            stage_key_ns: 0,
            stage_key_materialize_ns: 0,
            stage_key_order_ns: 0,
            stage_key_pack_ns: 0,
            stage_lookup_ns: 0,
            stage_unit_ns: 0,
            stage_frequency_ns: 0,
            stage_components_ns: 0,
            component_dsu_calls: 0,
            component_dsu_slots: 0,
            scoped_component_index: vec![0; problem.n + 1],
            scoped_component_stamp: vec![0; problem.n + 1],
            scoped_component_epoch: 0,
            component_ve_attempts: 0,
            component_ve_completed: 0,
            component_ve_capped: 0,
            component_ve_products: 0,
            component_ve_scans: 0,
            residual_ve_attempts: 0,
            residual_ve_completed: 0,
            residual_ve_capped: 0,
            residual_ve_products: 0,
            residual_ve_max_width: 0,
            articulation_root_only: false,
            primal_scores: None,
            primal_root_only: false,
            primal_scans: 0,
            primal_overrides: 0,
            separator_scans: 0,
            separator_candidates: 0,
            separator_chosen: 0,
            separator_skipped: 0,
            reused_unforced_keys: 0,
            projected_forced_keys: 0,
            weight_classes: Vec::new(),
            local_variable: vec![0; problem.n + 1],
            local_generation: vec![0; problem.n + 1],
            local_epoch: 0,
        }
    }

    /// Opt-in bitmap initialized only at the root, before any assignment.
    /// The last word is masked to avoid visiting a nonexistent clause ID.
    pub(crate) fn enable_active_clause_bitmap(&mut self) {
        assert!(self.trail.is_empty(), "bitmap must start at the root");
        let n = self.clauses.len();
        let mut words = vec![u64::MAX; n.div_ceil(64)];
        if !n.is_multiple_of(64) {
            if let Some(last) = words.last_mut() {
                *last = (1_u64 << (n % 64)) - 1;
            }
        }
        self.active_clause_bits = Some(words);
        self.active_bitmap_key_visits = 0;
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
            if self.satisfied[cid] == 0 {
                if let Some(words) = self.active_clause_bits.as_mut() {
                    words[cid / 64] &= !(1_u64 << (cid % 64));
                }
            }
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
                if self.satisfied[cid] == 0 {
                    if let Some(words) = self.active_clause_bits.as_mut() {
                        words[cid / 64] |= 1_u64 << (cid % 64);
                    }
                }
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
        &mut self,
        scope: &[usize],
        parent_ids: Option<&[usize]>,
        inline_weight_relabel: bool,
    ) -> Option<(Vec<usize>, Vec<i32>)> {
        let materialize_started = self.stage_profile.then(Instant::now);
        let mut flat = if self.use_reusable_key_arena {
            self.key_arena_calls += 1;
            let arena = std::mem::take(&mut self.key_arena_flat);
            self.key_arena_flat_reuses += u64::from(arena.capacity() > 0);
            arena
        } else {
            Vec::new()
        };
        let mut views = if self.use_reusable_key_arena {
            let arena = std::mem::take(&mut self.key_arena_views);
            self.key_arena_view_reuses += u64::from(arena.capacity() > 0);
            arena
        } else {
            Vec::new()
        };
        flat.clear();
        views.clear();
        // Temporarily move only the fixed-size bitmap out of the solver
        // while the key visitor mutably borrows other solver fields.
        // Key construction never applies or undoes assignments.
        let active_words = self.active_clause_bits.take();
        let mut bitmap_visits = 0_u64;
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
                            let in_scope = if inline_weight_relabel
                                && self.use_scope_stamp_admission
                            {
                                self.scope_stamp_probes += 1;
                                let stamped = self.local_generation[variable] == self.local_epoch;
                                debug_assert_eq!(
                                    stamped,
                                    scope.binary_search(&variable).is_ok(),
                                    "exact scope stamp must equal source-sorted membership"
                                );
                                stamped
                            } else {
                                scope.binary_search(&variable).is_ok()
                            };
                            if !in_scope {
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
            views.push(ResidualKeyView {
                cid,
                start,
                end: flat.len(),
            });
            true
        };
        let complete = if let Some(parent) = parent_ids.filter(|_| self.use_parent_ids) {
            // Keep the original parent traversal order; omitted clauses
            // were already satisfied and originally returned true.
            parent
                .iter()
                .filter(|&&cid| {
                    active_words.as_ref().is_none_or(|words| {
                        words[cid / 64] & (1_u64 << (cid % 64)) != 0
                    })
                })
                .all(|&cid| {
                    bitmap_visits += u64::from(active_words.is_some());
                    visit(cid)
                })
        } else if let Some(words) = active_words.as_ref() {
            // Visit only unfulfilled source IDs, in exactly ascending ID
            // order. The last word's unused upper bits are always zero.
            let mut complete = true;
            'source: for (word_index, &word) in words.iter().enumerate() {
                let mut pending = word;
                while pending != 0 {
                    let cid = word_index * 64 + pending.trailing_zeros() as usize;
                    pending &= pending - 1;
                    bitmap_visits += 1;
                    if !visit(cid) {
                        complete = false;
                        break 'source;
                    }
                }
            }
            complete
        } else {
            (0..self.clauses.len()).all(&mut visit)
        };
        drop(visit);
        self.active_clause_bits = active_words;
        self.active_bitmap_key_visits += bitmap_visits;
        if !complete {
            // A Boolean contradiction never becomes a numeric zero WMC
            // certificate. Restore transient arena even on failed paths.
            if self.use_reusable_key_arena {
                if flat.capacity() <= KEY_ARENA_MAX_LITERALS {
                    self.key_arena_flat = flat;
                }
                if views.capacity() <= KEY_ARENA_MAX_VIEWS {
                    self.key_arena_views = views;
                }
            }
            return None;
        }
        if let Some(start) = materialize_started {
            self.stage_key_materialize_ns += start.elapsed().as_nanos();
        }
        let order_started = self.stage_profile.then(Instant::now);
        // The parent residual is often already in exact lexicographic order.
        // If every *adjacent* pair remains strictly increasing after source
        // assignments, a complete sort and duplicate elimination is redundant.
        // Equality must take the original path too: unstable-sort duplicate
        // representative IDs can affect deterministic Trail search topology.
        let already_ordered = if self.use_ordered_key_fastpath && views.len() >= 2 {
            self.ordered_key_probes += 1;
            let increasing = views
                .windows(2)
                .all(|pair| flat[pair[0].start..pair[0].end] < flat[pair[1].start..pair[1].end]);
            if increasing {
                self.ordered_key_hits += 1;
            }
            increasing
        } else {
            false
        };
        if !already_ordered {
            views.sort_unstable_by(|a, b| flat[a.start..a.end].cmp(&flat[b.start..b.end]));
            views.dedup_by(|a, b| flat[a.start..a.end] == flat[b.start..b.end]);
        }
        if let Some(start) = order_started {
            self.stage_key_order_ns += start.elapsed().as_nanos();
        }
        let pack_started = self.stage_profile.then(Instant::now);
        let mut ids = Vec::with_capacity(views.len());
        let mut literals = Vec::with_capacity(flat.len() + views.len());
        for view in &views {
            ids.push(view.cid);
            literals.extend_from_slice(&flat[view.start..view.end]);
            literals.push(0);
        }
        if let Some(start) = pack_started {
            self.stage_key_pack_ns += start.elapsed().as_nanos();
        }
        if self.use_reusable_key_arena {
            // Never retain an oversized branch arena across later DPLL nodes.
            // All returned source clause IDs and signed literals are owned.
            if flat.capacity() <= KEY_ARENA_MAX_LITERALS {
                self.key_arena_flat = flat;
            }
            if views.capacity() <= KEY_ARENA_MAX_VIEWS {
                self.key_arena_views = views;
            }
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

    /// Exact AND/OR accelerator for a proven disconnected component. The
    /// parent node already owns all forced and free-variable weights, so this
    /// helper must count only this component's original still-unassigned
    /// variables and clauses. Never cache a partial component on rejection.
    fn try_component_factor_ve(&mut self, part: &[usize], ids: &[usize]) -> Option<Outcome> {
        if !self.use_component_local_ve
            || self.component_ve_attempts >= 8
            || !(8..=32).contains(&part.len())
            || ids.len() > 512
        {
            return None;
        }
        let mut residual = Vec::<Clause>::new();
        for &cid in ids {
            if self.satisfied[cid] != 0 {
                continue;
            }
            self.component_ve_scans += 1;
            let mut clause = Vec::new();
            let mut intersects = false;
            for &literal in &self.clauses[cid] {
                let var = literal.unsigned_abs() as usize;
                if self.assigned[var] != 0 {
                    continue;
                }
                if part.binary_search(&var).is_ok() {
                    intersects = true;
                    clause.push(literal);
                } else {
                    // Preserve noncomponent literals temporarily. If any
                    // clause intersects both parts, the full membership
                    // check below rejects before any numeric operation.
                    clause.push(literal);
                }
            }
            if intersects {
                if clause
                    .iter()
                    .any(|lit| part.binary_search(&(lit.unsigned_abs() as usize)).is_err())
                {
                    return None;
                }
                if clause.is_empty() || clause.len() > 12 {
                    return None;
                }
                residual.push(clause);
                if residual.len() > 128 {
                    return None;
                }
            }
        }
        if residual.is_empty() {
            return None;
        }
        self.component_ve_attempts += 1;
        let (result, metrics) =
            factor_variable_elimination::count_clauses(self.problem, part, &residual, self.bits);
        self.component_ve_products += metrics.products as u64;
        if result.is_some() {
            self.component_ve_completed += 1;
        } else {
            self.component_ve_capped += 1;
        }
        result
    }

    fn components(&mut self, ids: &[usize], active: &[usize]) -> Vec<Vec<usize>> {
        self.component_dsu_calls += 1;
        if self.use_scoped_component_dsu {
            self.component_dsu_slots += active.len() as u64;
            self.scoped_component_epoch += 1;
            let stamp = self.scoped_component_epoch;
            for (i, &variable) in active.iter().enumerate() {
                self.scoped_component_stamp[variable] = stamp;
                self.scoped_component_index[variable] = i;
            }
            let mut parents: Vec<_> = (0..active.len()).collect();
            for &cid in ids {
                if self.satisfied[cid] != 0 {
                    continue;
                }
                let mut first = None;
                for &lit in &self.clauses[cid] {
                    let variable = lit.unsigned_abs() as usize;
                    if self.assigned[variable] != 0 {
                        continue;
                    }
                    // Every live literal in the current residual's canonical
                    // clause set must belong to the provided active scope.
                    // This invariant is already needed by the original DSU.
                    if self.scoped_component_stamp[variable] != stamp {
                        // Preserve original semantics on unexpected
                        // out-of-scope literals; no heuristic result.
                        return self.components_full(ids, active);
                    }
                    let local = self.scoped_component_index[variable];
                    if let Some(previous) = first {
                        let a = root(&mut parents, previous);
                        let b = root(&mut parents, local);
                        parents[b] = a;
                    } else {
                        first = Some(local);
                    }
                }
            }
            let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for (i, &variable) in active.iter().enumerate() {
                let representative = active[root(&mut parents, i)];
                groups.entry(representative).or_default().push(variable);
            }
            return groups.into_values().collect();
        }
        self.component_dsu_slots += (self.problem.n + 1) as u64;
        self.components_full(ids, active)
    }

    fn components_full(&self, ids: &[usize], active: &[usize]) -> Vec<Vec<usize>> {
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
        let key_started = self.stage_profile.then(Instant::now);
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
                if let Some(start) = key_started {
                    self.stage_key_ns += start.elapsed().as_nanos();
                }
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
                if let Some(start) = key_started {
                    self.stage_key_ns += start.elapsed().as_nanos();
                }
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
        if let Some(start) = key_started {
            self.stage_key_ns += start.elapsed().as_nanos();
        }
        let lookup_started = self.stage_profile.then(Instant::now);
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
        if let Some(start) = lookup_started {
            self.stage_lookup_ns += start.elapsed().as_nanos();
        }
        if let Some(hit) = hit {
            self.stats.cache_hits += 1;
            return Ok(hit.clone());
        }
        let unit_started = self.stage_profile.then(Instant::now);
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
                if let Some(start) = unit_started {
                    self.stage_unit_ns += start.elapsed().as_nanos();
                }
                return Ok(self.save(key, key_hash, Outcome::unsat(self.bits)));
            }
        }
        if let Some(start) = unit_started {
            self.stage_unit_ns += start.elapsed().as_nanos();
        }

        let frequency_started = self.stage_profile.then(Instant::now);
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
        if let Some(start) = frequency_started {
            self.stage_frequency_ns += start.elapsed().as_nanos();
        }
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
        let components_started = self.stage_profile.then(Instant::now);
        let pieces = self.components(&ids, &active);
        if let Some(start) = components_started {
            self.stage_components_ns += start.elapsed().as_nanos();
        }
        if pieces.len() > 1 {
            self.stats.decompositions += 1;
            let mut value = C::one(self.bits);
            for part in pieces {
                let result = if let Some(exact) = self.try_component_factor_ve(&part, &ids) {
                    exact
                } else {
                    self.count_relevant(part, depth + 1, Some(&ids))?
                };
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
        // Exact sum-product DP on the *live cofactor*, not the original
        // potentially huge root formula. Every forced literal and free
        // variable was already accounted for in 'prefix'; the residual
        // factor scope owns only the remaining active signed weights.
        // Failed bounded probes discard all partial DP states and proceed
        // to exactly the same full weighted DPLL children as the control.
        if self.use_residual_factor_ve
            && self.residual_ve_attempts < 8
            && !active.is_empty()
            && active.len() <= 64
            && ids.len() <= 512
        {
            let mut residual = Vec::<Clause>::with_capacity(ids.len());
            let mut width_ok = true;
            for &cid in &ids {
                if self.satisfied[cid] != 0 {
                    continue;
                }
                let clause: Clause = self.clauses[cid]
                    .iter()
                    .copied()
                    .filter(|&lit| self.assigned[lit.unsigned_abs() as usize] == 0)
                    .collect();
                if clause.len() > 12 {
                    width_ok = false;
                    break;
                }
                residual.push(clause);
            }
            if width_ok {
                self.residual_ve_attempts += 1;
                let (candidate, stats) = factor_variable_elimination::count_clauses(
                    self.problem,
                    &active,
                    &residual,
                    self.bits,
                );
                self.residual_ve_products += stats.products as u64;
                self.residual_ve_max_width = self.residual_ve_max_width.max(stats.width);
                if let Some(answer) = candidate {
                    self.residual_ve_completed += 1;
                    let result = if answer.sat {
                        Outcome {
                            sat: true,
                            count: prefix.mul(&answer.count, self.bits),
                        }
                    } else {
                        Outcome::unsat(self.bits)
                    };
                    return Ok(self.save(key, key_hash, result));
                }
                self.residual_ve_capped += 1;
            }
        }
        // A factor-graph articulation variable, if one exists, is a sound
        // separator pivot for both Shannon branches. This is after unit
        // closure and the standard disconnected-component check.
        let pivot =
            if self.use_articulation_pivot && (!self.articulation_root_only || depth == 0) {
                self.separator_scans += 1;
                let probe = separator_pivot::choose(
                    &active,
                    &ids,
                    &self.clauses,
                    &self.assigned,
                    &self.satisfied,
                    &self.frequency,
                );
                self.separator_candidates += probe.candidates as u64;
                if !probe.scanned {
                    self.separator_skipped += 1;
                }
                if probe.pivot.is_some() {
                    self.separator_chosen += 1;
                }
                probe.pivot.or(pivot)
            } else {
                pivot
            }
            .ok_or("missing trail pivot")?;
        // A fixed source-primal bag centroid can act as a branch-order
        // score, similar in *purpose* (not implementation) to GPMC's
        // FlowCutter-derived TD distance score. No component or WMC
        // factor is added, removed or approximated here.
        let pivot = if !self.primal_root_only || depth == 0 {
            if let Some(ref scores) = self.primal_scores {
                self.primal_scans += 1;
                let choice = *active
                    .iter()
                    .max_by_key(|&&var| (scores[var], self.frequency[var]))
                    .ok_or("no candidate for primal centroid pivot")?;
                if choice != pivot && scores[choice] > 0 {
                    self.primal_overrides += 1;
                    choice
                } else {
                    pivot
                }
            } else {
                pivot
            }
        } else {
            pivot
        };
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
    // Project an already canonical reduced source-key through the current
    // reversible assignments. This never revisits the original clauses.
    // Filtering can make previously different clauses equal, therefore
    // the projected spans must be sorted AND deduplicated again.
    fn project_pre_unit_key(&self, pre_lits: &[i32], local: bool) -> Option<Vec<i32>> {
        #[derive(Clone, Copy)]
        struct Span {
            start: usize,
            end: usize,
        }
        debug_assert!(pre_lits.is_empty() || pre_lits.last() == Some(&0));
        let mut flat = Vec::<i32>::with_capacity(pre_lits.len());
        let mut spans = Vec::<Span>::new();
        let mut start = 0;
        let mut satisfied = false;
        let mut unassigned = false;
        for &lit in pre_lits {
            if lit == 0 {
                if !satisfied {
                    if !unassigned {
                        // An empty live residual clause is Boolean UNSAT,
                        // even when its numeric weights sum to zero.
                        return None;
                    }
                    spans.push(Span {
                        start,
                        end: flat.len(),
                    });
                } else {
                    flat.truncate(start);
                }
                start = flat.len();
                satisfied = false;
                unassigned = false;
                continue;
            }
            if satisfied {
                continue;
            }
            let var = lit.unsigned_abs() as usize;
            let current = self.assigned[var];
            if current == 0 {
                let number = if local {
                    self.local_variable[var]
                } else {
                    var as i32
                };
                flat.push(if lit < 0 { -number } else { number });
                unassigned = true;
            } else if current == if lit < 0 { -1 } else { 1 } {
                satisfied = true;
                flat.truncate(start);
            }
        }
        debug_assert_eq!(flat.len(), start);
        spans.sort_unstable_by(|a, b| flat[a.start..a.end].cmp(&flat[b.start..b.end]));
        spans.dedup_by(|a, b| flat[a.start..a.end] == flat[b.start..b.end]);
        let mut compact = Vec::with_capacity(flat.len() + spans.len());
        for span in spans {
            compact.extend_from_slice(&flat[span.start..span.end]);
            compact.push(0);
        }
        Some(compact)
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
        let Some((ids, pre_lits)) = self.canonical_compact(&scope, parent_ids, false) else {
            return Ok(Outcome::unsat(self.bits));
        };
        let mut forced_here = 0_u64;
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
            forced_here += 1;
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
        let reuse_no_units = self.use_projected_post_key && forced_here == 0;
        // If the source key is reused verbatim there is no local rank
        // mapping yet, so use the original exact weighted relabel pass.
        let inline_relabel = localized && self.use_inline_weight_relabel && !reuse_no_units;
        let class_scope = if inline_relabel {
            Some(self.prepare_weighted_scope(&post_scope)?)
        } else {
            None
        };
        let literals = if reuse_no_units {
            self.reused_unforced_keys += 1;
            pre_lits
        } else if self.use_projected_post_key {
            self.projected_forced_keys += 1;
            let Some(projected) = self.project_pre_unit_key(&pre_lits, inline_relabel) else {
                return Ok(Outcome::unsat(self.bits));
            };
            projected
        } else {
            let Some((_, clauses)) =
                self.canonical_compact(&post_scope, Some(&ids), inline_relabel)
            else {
                return Ok(Outcome::unsat(self.bits));
            };
            clauses
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
        // The original post-unit path scans the same active source clauses
        // separately for frequency and DSU components. Preserve the old
        // path as the independent control; only the opt-in path fuses them.
        let mut fused_parents = if self.use_fused_post_analysis {
            let mut parents: Vec<_> = (0..=self.problem.n).collect();
            for &cid in &ids {
                if self.satisfied[cid] != 0 {
                    continue;
                }
                let mut first = None;
                for &lit in &self.clauses[cid] {
                    let var = lit.unsigned_abs() as usize;
                    if self.assigned[var] != 0 {
                        continue;
                    }
                    frequency[var] += 1;
                    if let Some(first_var) = first {
                        let a = root(&mut parents, first_var);
                        let b = root(&mut parents, var);
                        parents[b] = a;
                    } else {
                        first = Some(var);
                    }
                }
            }
            Some(parents)
        } else {
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
            None
        };
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
        let pieces = if let Some(ref mut parents) = fused_parents {
            let mut groups = BTreeMap::<usize, Vec<usize>>::new();
            for &var in &active {
                groups.entry(root(parents, var)).or_default().push(var);
            }
            groups.into_values().collect::<Vec<_>>()
        } else {
            self.components(&ids, &active)
        };
        if pieces.len() > 1 {
            self.stats.decompositions += 1;
            let mut value = C::one(self.bits);
            for part in pieces {
                let result = if let Some(exact) = self.try_component_factor_ve(&part, &ids) {
                    exact
                } else {
                    self.count_relevant(part, depth + 1, Some(&ids))?
                };
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
    #[test]
    fn projected_unit_key_exactly_matches_post_unit_key() {
        let fixtures = [
            concat!(
                "p cnf 7 8\nc t wmc\n",
                "1 2 0\n-1 -2 0\n",
                "1 3 4 0\n2 3 4 0\n",
                "1 6 7 0\n2 6 7 0\n",
                "3 5 0\n4 5 0\n"
            ),
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
        ];
        for bits in [100_u32, 333] {
            for input in fixtures {
                let p = parse_emitted(input, bits).unwrap();
                for max_scope in [None, Some(2_usize), Some(16)] {
                    for limit in [0_usize, 1, 256] {
                        let mut control = TrailEngine::new(&p, bits, limit, 100_000);
                        control.use_parent_ids = true;
                        control.use_compact_residual = true;
                        control.use_prehashed_cache = true;
                        control.use_fused_compact = true;
                        control.use_weight_class_cache = true;
                        control.weight_cache_scope_max = max_scope;
                        control.use_inline_weight_relabel = true;
                        control.use_post_unit_cache = true;
                        let expected = control.count((1..=p.n).collect(), 0).unwrap();
                        let mut optimized = TrailEngine::new(&p, bits, limit, 100_000);
                        optimized.use_parent_ids = true;
                        optimized.use_compact_residual = true;
                        optimized.use_prehashed_cache = true;
                        optimized.use_fused_compact = true;
                        optimized.use_weight_class_cache = true;
                        optimized.weight_cache_scope_max = max_scope;
                        optimized.use_inline_weight_relabel = true;
                        optimized.use_post_unit_cache = true;
                        optimized.use_projected_post_key = true;
                        let actual = optimized.count((1..=p.n).collect(), 0).unwrap();
                        assert_eq!(actual.sat, expected.sat);
                        assert_eq!(actual.count.re, expected.count.re);
                        assert_eq!(actual.count.im, expected.count.im);
                        assert_eq!(optimized.stats.nodes, control.stats.nodes);
                        assert_eq!(optimized.stats.cache_hits, control.stats.cache_hits);
                        assert_eq!(optimized.stats.decompositions, control.stats.decompositions);
                        assert_eq!(optimized.post_unit_hits, control.post_unit_hits);
                        assert_eq!(optimized.post_unit_forced, control.post_unit_forced);
                        assert!(optimized.assigned.iter().all(|&x| x == 0));
                    }
                }
            }
        }
    }

    #[test]
    fn articulation_pivot_preserves_full_signed_complex_wmc_and_sat() {
        let sources = [
            "p cnf 5 6\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 2 0\n1 3 0\n2 3 0\n3 4 0\n3 5 0\n4 5 0\n",
            "p cnf 3 0\nc t wmc\nc p weight 1 1 0 0\nc p weight -1 -1 0 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 5 6\nc t wmc\n-1 2 0\n1 3 0\n-2 3 0\n3 -4 0\n-3 5 0\n4 -5 0\n",
        ];
        for bits in [100_u32, 333] {
            for source in sources {
                let problem = parse_emitted(source, bits).unwrap();
                for capacity in [0_usize, 1, 128] {
                    let mut original = TrailEngine::new(&problem, bits, capacity, 100_000);
                    original.use_parent_ids = true;
                    original.use_compact_residual = true;
                    original.use_prehashed_cache = true;
                    original.use_fused_compact = true;
                    original.use_weight_class_cache = true;
                    original.use_inline_weight_relabel = true;
                    let expected = original.count((1..=problem.n).collect(), 0).unwrap();

                    let mut changed = TrailEngine::new(&problem, bits, capacity, 100_000);
                    changed.use_parent_ids = true;
                    changed.use_compact_residual = true;
                    changed.use_prehashed_cache = true;
                    changed.use_fused_compact = true;
                    changed.use_weight_class_cache = true;
                    changed.use_inline_weight_relabel = true;
                    changed.use_articulation_pivot = true;
                    let actual = changed.count((1..=problem.n).collect(), 0).unwrap();
                    assert_eq!(actual.sat, expected.sat);
                    assert_eq!(actual.count.re, expected.count.re);
                    assert_eq!(actual.count.im, expected.count.im);
                    assert!(changed.assigned.iter().all(|&v| v == 0));
                    assert!(original.assigned.iter().all(|&v| v == 0));
                }
            }
        }
    }

    #[test]
    fn root_only_separator_preserves_complex_wmc_and_limits_pivot_probes() {
        let source = "p cnf 5 6\nc t wmc\nc p weight 3 0.25 -0.125 0\nc p weight -3 -0.5 0.25 0\n1 2 0\n1 3 0\n2 3 0\n3 4 0\n3 5 0\n4 5 0\n";
        for bits in [100_u32, 333] {
            let problem = parse_emitted(source, bits).unwrap();
            for limit in [0_usize, 1, 128] {
                let mut baseline = TrailEngine::new(&problem, bits, limit, 100_000);
                baseline.use_parent_ids = true;
                baseline.use_compact_residual = true;
                baseline.use_prehashed_cache = true;
                baseline.use_fused_compact = true;
                baseline.use_weight_class_cache = true;
                baseline.use_inline_weight_relabel = true;
                let expected = baseline.count((1..=problem.n).collect(), 0).unwrap();

                let mut root = TrailEngine::new(&problem, bits, limit, 100_000);
                root.use_parent_ids = true;
                root.use_compact_residual = true;
                root.use_prehashed_cache = true;
                root.use_fused_compact = true;
                root.use_weight_class_cache = true;
                root.use_inline_weight_relabel = true;
                root.use_articulation_pivot = true;
                root.articulation_root_only = true;
                let actual = root.count((1..=problem.n).collect(), 0).unwrap();
                assert_eq!(actual.sat, expected.sat);
                assert_eq!(actual.count.re, expected.count.re);
                assert_eq!(actual.count.im, expected.count.im);
                assert!(root.separator_scans <= 1);
                assert!(root.separator_chosen <= 1);
                assert!(root.assigned.iter().all(|&v| v == 0));
            }
        }
    }

    #[test]
    fn component_local_ve_matches_independent_signed_complex_enumeration() {
        // Component A is an 8-cycle; component B is an unrelated edge.
        // A 10-variable brute-force assignment oracle has no solver cache,
        // AND/OR decomposition routine, or factor elimination.
        let source = "p cnf 10 9\nc t wmc\n\
c p weight 1 0.5 0.25 0\nc p weight -1 -0.25 -0.5 0\n\
c p weight 8 -0.125 0.5 0\nc p weight -8 0.25 -0.25 0\n\
c p weight 9 -0.5 0.125 0\nc p weight -9 0.25 -0.5 0\n\
1 2 0\n2 3 0\n3 4 0\n4 5 0\n5 6 0\n6 7 0\n7 8 0\n8 1 0\n9 10 0\n";
        for bits in [100_u32, 333] {
            let p = parse_emitted(source, bits).unwrap();
            let scope: Vec<usize> = (1..=p.n).collect();
            let mut sat = false;
            let mut sum = C::zero(bits);
            for assignment in 0..(1_usize << p.n) {
                let accepted = p.clauses.iter().all(|clause| {
                    clause.iter().any(|&lit| {
                        let index = lit.unsigned_abs() as usize - 1;
                        (assignment & (1 << index) != 0) == (lit > 0)
                    })
                });
                if !accepted {
                    continue;
                }
                sat = true;
                let mut product = C::one(bits);
                for (var, weight) in p.weights.iter().enumerate().take(p.n) {
                    product = product.mul(
                        if assignment & (1 << var) != 0 {
                            &weight.positive
                        } else {
                            &weight.negative
                        },
                        bits,
                    );
                }
                sum = sum.add(&product, bits);
            }
            for cache_limit in [0_usize, 1, 128] {
                let mut old = TrailEngine::new(&p, bits, cache_limit, 100_000);
                old.use_parent_ids = true;
                old.use_compact_residual = true;
                old.use_prehashed_cache = true;
                old.use_fused_compact = true;
                old.use_weight_class_cache = true;
                old.use_inline_weight_relabel = true;
                let expected = old.count(scope.clone(), 0).unwrap();

                let mut candidate = TrailEngine::new(&p, bits, cache_limit, 100_000);
                candidate.use_parent_ids = true;
                candidate.use_compact_residual = true;
                candidate.use_prehashed_cache = true;
                candidate.use_fused_compact = true;
                candidate.use_weight_class_cache = true;
                candidate.use_inline_weight_relabel = true;
                candidate.use_component_local_ve = true;
                let got = candidate.count(scope.clone(), 0).unwrap();
                assert_eq!(got.sat, sat);
                assert_eq!(got.count.re, sum.re);
                assert_eq!(got.count.im, sum.im);
                assert_eq!(got.sat, expected.sat);
                assert_eq!(got.count.re, expected.count.re);
                assert_eq!(got.count.im, expected.count.im);
                assert!(candidate.component_ve_attempts >= 1);
                assert!(candidate.component_ve_completed >= 1);
                assert!(candidate.component_ve_attempts <= 8);
                assert!(candidate.component_ve_scans > 0);
                assert_eq!(candidate.assigned.iter().filter(|&&v| v != 0).count(), 0);
                assert!(candidate.trail.is_empty());
            }
        }
    }

    #[test]
    fn component_local_ve_is_skipped_for_connected_formula() {
        // Admission is exclusively at an existing exact AND/OR split.
        let source = "p cnf 10 10\nc t wmc\n\
1 2 0\n2 3 0\n3 4 0\n4 5 0\n5 6 0\n6 7 0\n7 8 0\n8 9 0\n9 10 0\n10 1 0\n";
        for bits in [100_u32, 333] {
            let p = parse_emitted(source, bits).unwrap();
            let mut baseline = TrailEngine::new(&p, bits, 128, 100_000);
            let old = baseline.count((1..=p.n).collect(), 0).unwrap();
            let mut candidate = TrailEngine::new(&p, bits, 128, 100_000);
            candidate.use_component_local_ve = true;
            let got = candidate.count((1..=p.n).collect(), 0).unwrap();
            assert_eq!(old.sat, got.sat);
            assert_eq!(old.count.re, got.count.re);
            assert_eq!(old.count.im, got.count.im);
            assert!(candidate.component_ve_attempts <= 8);
            assert!(candidate.assigned.iter().all(|&v| v == 0));
        }
    }

    #[test]
    fn scoped_dsu_matches_exact_source_component_cache_and_search_topology() {
        // Distinct disconnected factors, signed complex weights, zero-ish
        // cancellation, and one fully connected case. No solver shortcut,
        // CNF regeneration or source weight conversion is admitted.
        let source_a = weighted_component_source("0.5");
        let source_b = weighted_component_source("0.125");
        let source_c = "p cnf 10 9\nc t wmc\n\
c p weight 1 0.5 0.25 0\nc p weight -1 -0.25 -0.5 0\n\
1 2 0\n2 3 0\n3 4 0\n4 5 0\n5 6 0\n6 7 0\n7 8 0\n8 1 0\n9 10 0\n";
        let source_d = "p cnf 4 4\nc t wmc\n\
c p weight 1 1 0 0\nc p weight -1 -1 0 0\n\
1 2 0\n-1 3 0\n-2 4 0\n-3 -4 0\n";
        for bits in [100_u32, 333] {
            for source in [source_a.as_str(), source_b.as_str(), source_c, source_d] {
                let p = parse_emitted(source, bits).unwrap();
                for cap in [0_usize, 1, 128] {
                    let scope = (1..=p.n).collect::<Vec<usize>>();
                    let mut old = TrailEngine::new(&p, bits, cap, 100_000);
                    old.use_parent_ids = true;
                    old.use_compact_residual = true;
                    old.use_prehashed_cache = true;
                    old.use_fused_compact = true;
                    old.use_weight_class_cache = true;
                    old.use_inline_weight_relabel = true;
                    let expected = old.count(scope.clone(), 0).unwrap();

                    let mut compact = TrailEngine::new(&p, bits, cap, 100_000);
                    compact.use_parent_ids = true;
                    compact.use_compact_residual = true;
                    compact.use_prehashed_cache = true;
                    compact.use_fused_compact = true;
                    compact.use_weight_class_cache = true;
                    compact.use_inline_weight_relabel = true;
                    compact.use_scoped_component_dsu = true;
                    let observed = compact.count(scope, 0).unwrap();

                    assert_eq!(observed.sat, expected.sat);
                    assert_eq!(observed.count.re, expected.count.re);
                    assert_eq!(observed.count.im, expected.count.im);
                    assert_eq!(compact.stats.nodes, old.stats.nodes);
                    assert_eq!(compact.stats.cache_hits, old.stats.cache_hits);
                    assert_eq!(compact.stats.decompositions, old.stats.decompositions);
                    assert_eq!(compact.component_dsu_calls, old.component_dsu_calls);
                    assert!(compact.component_dsu_slots <= old.component_dsu_slots);
                    assert!(compact.trail.is_empty() && old.trail.is_empty());
                    assert!(compact.assigned.iter().all(|&v| v == 0));
                    assert!(old.assigned.iter().all(|&v| v == 0));
                }
            }
        }
    }

    #[test]
    fn scoped_dsu_weighted_random_dyadic_cache_regression() {
        let mut seed = 0x9e3779b97f4a7c15_u64;
        for bits in [100_u32, 333] {
            for case in 0..50 {
                let mut clauses = Vec::new();
                for _ in 0..(5 + case % 13) {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let width = 1 + (seed as usize % 3);
                    let mut clause = Vec::new();
                    for _ in 0..width {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let v = 1 + ((seed >> 16) as usize % 7);
                        clause.push(if seed & 8 == 0 { v as i32 } else { -(v as i32) });
                    }
                    clauses.push(clause);
                }
                let p = Problem {
                    n: 7,
                    clauses,
                    weights: (0..7)
                        .map(|v| crate::Weights {
                            positive: C::decimal(
                                bits,
                                if v % 3 == 0 { "-0.25" } else { "0.5" },
                                if v % 2 == 0 { "0.125" } else { "-0.25" },
                            )
                            .unwrap(),
                            negative: C::decimal(
                                bits,
                                if v % 2 == 0 { "0.25" } else { "-0.125" },
                                if v % 3 == 0 { "0.5" } else { "-0.25" },
                            )
                            .unwrap(),
                        })
                        .collect(),
                };
                for cap in [0_usize, 128] {
                    let scope: Vec<usize> = (1..=7).collect();
                    let mut old = TrailEngine::new(&p, bits, cap, 200_000);
                    old.use_parent_ids = true;
                    old.use_compact_residual = true;
                    old.use_prehashed_cache = true;
                    old.use_fused_compact = true;
                    old.use_weight_class_cache = true;
                    old.use_inline_weight_relabel = true;
                    let reference = old.count(scope.clone(), 0).unwrap();

                    let mut new = TrailEngine::new(&p, bits, cap, 200_000);
                    new.use_parent_ids = true;
                    new.use_compact_residual = true;
                    new.use_prehashed_cache = true;
                    new.use_fused_compact = true;
                    new.use_weight_class_cache = true;
                    new.use_inline_weight_relabel = true;
                    new.use_scoped_component_dsu = true;
                    let observed = new.count(scope, 0).unwrap();
                    assert_eq!(reference.sat, observed.sat, "case={case}");
                    assert_eq!(reference.count.re, observed.count.re, "case={case}");
                    assert_eq!(reference.count.im, observed.count.im, "case={case}");
                    assert_eq!(old.stats.nodes, new.stats.nodes, "case={case}");
                    assert_eq!(old.stats.cache_hits, new.stats.cache_hits, "case={case}");
                    assert_eq!(
                        old.stats.decompositions, new.stats.decompositions,
                        "case={case}"
                    );
                }
            }
        }
    }

    #[test]
    fn bounded_post_unit_factor_ve_preserves_signed_complex_wmc() {
        // Exact Boolean SAT must remain separate from cancellations to zero.
        let cases = [
            "p cnf 5 5\nc t wmc\nc p weight 1 0.25 -0.5 0\nc p weight -1 -0.125 0.25 0\n1 2 0\n-1 3 0\n3 4 0\n-4 5 0\n-5 2 0\n",
            "p cnf 4 3\nc t wmc\nc p weight 2 0 0 0\nc p weight -2 -1 0 0\n2 0\n1 3 0\n-1 4 0\n",
            "p cnf 4 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 8 5\nc t wmc\nc p weight 6 -0.5 0.25 0\nc p weight -6 0.25 -0.5 0\n1 2 0\n-1 3 0\n4 5 0\n-4 6 0\n7 -8 0\n",
        ];
        for bits in [100_u32, 333] {
            for source in cases {
                let p = parse_emitted(source, bits).unwrap();
                let scope: Vec<usize> = (1..=p.n).collect();
                for cache_limit in [0_usize, 1, 128] {
                    let mut baseline = TrailEngine::new(&p, bits, cache_limit, 100_000);
                    baseline.use_parent_ids = true;
                    baseline.use_compact_residual = true;
                    baseline.use_prehashed_cache = true;
                    baseline.use_fused_compact = true;
                    baseline.use_weight_class_cache = true;
                    baseline.use_inline_weight_relabel = true;
                    let reference = baseline.count(scope.clone(), 0).unwrap();

                    let mut hybrid = TrailEngine::new(&p, bits, cache_limit, 100_000);
                    hybrid.use_parent_ids = true;
                    hybrid.use_compact_residual = true;
                    hybrid.use_prehashed_cache = true;
                    hybrid.use_fused_compact = true;
                    hybrid.use_weight_class_cache = true;
                    hybrid.use_inline_weight_relabel = true;
                    hybrid.use_residual_factor_ve = true;
                    let observed = hybrid.count(scope.clone(), 0).unwrap();
                    assert_eq!(observed.sat, reference.sat);
                    assert_eq!(observed.count.re, reference.count.re);
                    assert_eq!(observed.count.im, reference.count.im);
                    assert!(hybrid.residual_ve_attempts <= 8);
                    assert!(hybrid.residual_ve_completed <= hybrid.residual_ve_attempts);
                    assert!(hybrid.residual_ve_capped <= hybrid.residual_ve_attempts);
                    assert!(hybrid.assigned.iter().all(|&v| v == 0));
                }
            }
        }
    }

    #[test]
    fn fused_post_analysis_preserves_weighted_search() {
        let inputs = [
            "p cnf 7 8\nc t wmc\n1 2 0\n-1 -2 0\n1 3 4 0\n2 3 4 0\n1 6 7 0\n2 6 7 0\n3 5 0\n4 5 0\n",
            "p cnf 6 6\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 3 0\n-1 4 0\n3 5 0\n4 5 0\n5 -6 0\n-5 6 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
        ];
        for bits in [100_u32, 333] {
            for input in inputs {
                let p = parse_emitted(input, bits).unwrap();
                for capacity in [0_usize, 1, 128] {
                    for max_scope in [None, Some(2_usize), Some(16)] {
                        let mut old = TrailEngine::new(&p, bits, capacity, 100_000);
                        old.use_parent_ids = true;
                        old.use_compact_residual = true;
                        old.use_prehashed_cache = true;
                        old.use_fused_compact = true;
                        old.use_weight_class_cache = true;
                        old.weight_cache_scope_max = max_scope;
                        old.use_inline_weight_relabel = true;
                        old.use_post_unit_cache = true;
                        old.use_projected_post_key = true;
                        let expected = old.count((1..=p.n).collect(), 0).unwrap();

                        let mut next = TrailEngine::new(&p, bits, capacity, 100_000);
                        next.use_parent_ids = true;
                        next.use_compact_residual = true;
                        next.use_prehashed_cache = true;
                        next.use_fused_compact = true;
                        next.use_weight_class_cache = true;
                        next.weight_cache_scope_max = max_scope;
                        next.use_inline_weight_relabel = true;
                        next.use_post_unit_cache = true;
                        next.use_projected_post_key = true;
                        next.use_fused_post_analysis = true;
                        let actual = next.count((1..=p.n).collect(), 0).unwrap();
                        assert_eq!(actual.sat, expected.sat);
                        assert_eq!(actual.count.re, expected.count.re);
                        assert_eq!(actual.count.im, expected.count.im);
                        assert_eq!(next.stats.nodes, old.stats.nodes);
                        assert_eq!(next.stats.cache_hits, old.stats.cache_hits);
                        assert_eq!(next.stats.decompositions, old.stats.decompositions);
                        assert_eq!(next.post_unit_hits, old.post_unit_hits);
                        assert_eq!(next.post_unit_visited, old.post_unit_visited);
                        assert_eq!(next.post_unit_forced, old.post_unit_forced);
                        assert_eq!(next.reused_unforced_keys, old.reused_unforced_keys);
                        assert_eq!(next.projected_forced_keys, old.projected_forced_keys);
                        assert!(next.assigned.iter().all(|&v| v == 0));
                    }
                }
            }
        }
    }
    #[test]
    fn optional_stage_profile_preserves_signed_wmc_and_search_topology() {
        let inputs = [
            "p cnf 6 6\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 3 0\n-1 4 0\n3 5 0\n4 5 0\n5 -6 0\n-5 6 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
        ];
        for bits in [100_u32, 333] {
            for input in inputs {
                let p = parse_emitted(input, bits).unwrap();
                for capacity in [0_usize, 1, 128] {
                    let run = |profile: bool| {
                        let mut t = TrailEngine::new(&p, bits, capacity, 100_000);
                        t.use_parent_ids = true;
                        t.use_compact_residual = true;
                        t.use_prehashed_cache = true;
                        t.use_fused_compact = true;
                        t.use_weight_class_cache = true;
                        t.use_inline_weight_relabel = true;
                        t.stage_profile = profile;
                        let result = t.count((1..=p.n).collect(), 0).unwrap();
                        (result, t)
                    };
                    let (expected, control) = run(false);
                    let (actual, profiled) = run(true);
                    assert_eq!(
                        (actual.sat, &actual.count.re, &actual.count.im),
                        (expected.sat, &expected.count.re, &expected.count.im)
                    );
                    assert_eq!(
                        (
                            profiled.stats.nodes,
                            profiled.stats.cache_hits,
                            profiled.stats.decompositions
                        ),
                        (
                            control.stats.nodes,
                            control.stats.cache_hits,
                            control.stats.decompositions
                        )
                    );
                    assert_eq!(
                        (
                            control.stage_key_ns,
                            control.stage_lookup_ns,
                            control.stage_unit_ns,
                            control.stage_frequency_ns,
                            control.stage_components_ns
                        ),
                        (0, 0, 0, 0, 0)
                    );
                    assert!(profiled.stage_key_ns > 0);
                    assert_eq!(
                        (
                            control.stage_key_materialize_ns,
                            control.stage_key_order_ns,
                            control.stage_key_pack_ns
                        ),
                        (0, 0, 0)
                    );
                    let subphase_sum = profiled.stage_key_materialize_ns
                        + profiled.stage_key_order_ns
                        + profiled.stage_key_pack_ns;
                    assert!(subphase_sum > 0);
                    assert!(subphase_sum <= profiled.stage_key_ns);
                    assert!(profiled.trail.is_empty());
                    assert!(profiled.assigned.iter().all(|&v| v == 0));
                }
            }
        }
    }
    #[test]
    fn ordered_residual_fastpath_is_exact_for_strict_and_fallback_views() {
        let ordered_source = "p cnf 4 2\nc t wmc\n1 4 0\n2 3 0\n";
        let duplicate_source = "p cnf 3 2\nc t wmc\n1 2 0\n2 3 0\n";
        for bits in [100_u32, 333] {
            let p = parse_emitted(ordered_source, bits).unwrap();
            let mut baseline = TrailEngine::new(&p, bits, 128, 100_000);
            let mut optimized = TrailEngine::new(&p, bits, 128, 100_000);
            baseline.use_fused_compact = true;
            optimized.use_fused_compact = true;
            optimized.use_ordered_key_fastpath = true;
            let scope = (1..=p.n).collect::<Vec<_>>();
            assert_eq!(
                baseline.canonical_compact(&scope, None, false),
                optimized.canonical_compact(&scope, None, false)
            );
            assert_eq!(
                (optimized.ordered_key_probes, optimized.ordered_key_hits),
                (1, 1)
            );
            assert!(baseline.apply(-1, None));
            assert!(optimized.apply(-1, None));
            let reduced = vec![2, 3, 4];
            assert_eq!(
                baseline.canonical_compact(&reduced, None, false),
                optimized.canonical_compact(&reduced, None, false)
            );
            assert_eq!(
                (optimized.ordered_key_probes, optimized.ordered_key_hits),
                (2, 1)
            );

            let d = parse_emitted(duplicate_source, bits).unwrap();
            let mut original = TrailEngine::new(&d, bits, 128, 100_000);
            let mut shortcut = TrailEngine::new(&d, bits, 128, 100_000);
            original.use_fused_compact = true;
            shortcut.use_fused_compact = true;
            shortcut.use_ordered_key_fastpath = true;
            for e in [&mut original, &mut shortcut] {
                assert!(e.apply(-1, None));
                assert!(e.apply(-3, None));
            }
            assert_eq!(
                original.canonical_compact(&[2], None, false),
                shortcut.canonical_compact(&[2], None, false)
            );
            assert_eq!(
                (shortcut.ordered_key_probes, shortcut.ordered_key_hits),
                (1, 0)
            );
            assert_eq!(original.trail.len(), shortcut.trail.len());
        }
    }

    #[test]
    fn ordered_residual_fastpath_matches_independent_signed_complex_oracle() {
        let fixtures = [
            "p cnf 4 4\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 4 0\n2 3 0\n-1 2 0\n-2 -3 0\n",
            "p cnf 4 3\nc t wmc\nc p weight 3 -0.125 0.25 0\nc p weight -3 0.5 -0.25 0\n1 2 0\n-1 3 0\n2 4 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 3 2\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 2 0\n-2 3 0\n",
        ];
        let mut total_probes = 0_u64;
        let mut total_hits = 0_u64;
        for bits in [100_u32, 333] {
            for source in fixtures {
                let p = parse_emitted(source, bits).unwrap();
                let mut expected_sat = false;
                let mut expected_count = C::zero(bits);
                for mask in 0..(1_usize << p.n) {
                    let accepted = p.clauses.iter().all(|clause| {
                        clause.iter().any(|&lit| {
                            let index = lit.unsigned_abs() as usize - 1;
                            (mask & (1 << index) != 0) == (lit > 0)
                        })
                    });
                    if !accepted {
                        continue;
                    }
                    expected_sat = true;
                    let mut product = C::one(bits);
                    for (variable, weight) in p.weights.iter().enumerate().take(p.n) {
                        let factor = if mask & (1 << variable) != 0 {
                            &weight.positive
                        } else {
                            &weight.negative
                        };
                        product = product.mul(factor, bits);
                    }
                    expected_count = expected_count.add(&product, bits);
                }
                for capacity in [0_usize, 1, 128] {
                    let run = |fast: bool| {
                        let mut e = TrailEngine::new(&p, bits, capacity, 100_000);
                        e.use_parent_ids = true;
                        e.use_compact_residual = true;
                        e.use_prehashed_cache = true;
                        e.use_fused_compact = true;
                        e.use_weight_class_cache = true;
                        e.use_inline_weight_relabel = true;
                        e.use_ordered_key_fastpath = fast;
                        let result = e.count((1..=p.n).collect(), 0).unwrap();
                        (result, e)
                    };
                    let (control, baseline) = run(false);
                    let (result, fast) = run(true);
                    assert_eq!(
                        (result.sat, &result.count.re, &result.count.im),
                        (control.sat, &control.count.re, &control.count.im)
                    );
                    assert_eq!(result.sat, expected_sat);
                    assert_eq!(result.count.re, expected_count.re);
                    assert_eq!(result.count.im, expected_count.im);
                    assert_eq!(
                        (
                            fast.stats.nodes,
                            fast.stats.cache_hits,
                            fast.stats.decompositions
                        ),
                        (
                            baseline.stats.nodes,
                            baseline.stats.cache_hits,
                            baseline.stats.decompositions
                        )
                    );
                    assert_eq!(baseline.ordered_key_probes, 0);
                    assert_eq!(baseline.ordered_key_hits, 0);
                    assert!(fast.ordered_key_hits <= fast.ordered_key_probes);
                    assert!(fast.trail.is_empty());
                    assert!(fast.assigned.iter().all(|&v| v == 0));
                    total_probes += fast.ordered_key_probes;
                    total_hits += fast.ordered_key_hits;
                }
            }
        }
        assert!(total_probes > 0);
        assert!(total_hits > 0);
    }

    #[test]
    fn key_subphase_profile_is_nested_and_preserves_ordered_shortcut_result() {
        let sources = [
            "p cnf 4 4\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 4 0\n2 3 0\n-1 2 0\n-2 -3 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 3 2\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 2 0\n-2 3 0\n",
        ];
        for bits in [100_u32, 333] {
            for source in sources {
                let p = parse_emitted(source, bits).unwrap();
                for capacity in [0_usize, 1, 128] {
                    let run = |profile: bool| {
                        let mut e = TrailEngine::new(&p, bits, capacity, 100_000);
                        e.use_parent_ids = true;
                        e.use_compact_residual = true;
                        e.use_prehashed_cache = true;
                        e.use_fused_compact = true;
                        e.use_weight_class_cache = true;
                        e.use_inline_weight_relabel = true;
                        e.use_ordered_key_fastpath = true;
                        e.stage_profile = profile;
                        let result = e.count((1..=p.n).collect(), 0).unwrap();
                        (result, e)
                    };
                    let (control, a) = run(false);
                    let (profiled, b) = run(true);
                    assert_eq!(
                        (control.sat, &control.count.re, &control.count.im),
                        (profiled.sat, &profiled.count.re, &profiled.count.im)
                    );
                    assert_eq!(
                        (a.stats.nodes, a.stats.cache_hits, a.stats.decompositions),
                        (b.stats.nodes, b.stats.cache_hits, b.stats.decompositions)
                    );
                    assert_eq!(
                        (a.ordered_key_probes, a.ordered_key_hits),
                        (b.ordered_key_probes, b.ordered_key_hits)
                    );
                    assert_eq!(
                        (
                            a.stage_key_materialize_ns,
                            a.stage_key_order_ns,
                            a.stage_key_pack_ns,
                        ),
                        (0, 0, 0)
                    );
                    let nested =
                        b.stage_key_materialize_ns + b.stage_key_order_ns + b.stage_key_pack_ns;
                    assert!(nested > 0);
                    assert!(nested <= b.stage_key_ns);
                    assert!(b.trail.is_empty());
                    assert!(b.assigned.iter().all(|&x| x == 0));
                }
            }
        }
    }

    #[test]
    fn bounded_key_arena_reuse_preserves_weighted_wmc_and_branch_topology() {
        let sources = [
            "p cnf 4 4\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 4 0\n2 3 0\n-1 2 0\n-2 -3 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 3 2\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 2 0\n-2 3 0\n",
            "p cnf 4 3\nc t wmc\nc p weight 3 -0.125 0.25 0\nc p weight -3 0.5 -0.25 0\n1 2 0\n-1 3 0\n2 4 0\n",
        ];
        let mut observed_reuses = 0;
        for bits in [100_u32, 333] {
            for source in sources {
                let p = parse_emitted(source, bits).unwrap();
                for capacity in [0_usize, 1, 128] {
                    for ordered in [false, true] {
                        for profiled in [false, true] {
                            let run = |reuse: bool| {
                                let mut t = TrailEngine::new(&p, bits, capacity, 100_000);
                                t.use_parent_ids = true;
                                t.use_compact_residual = true;
                                t.use_prehashed_cache = true;
                                t.use_fused_compact = true;
                                t.use_weight_class_cache = true;
                                t.use_inline_weight_relabel = true;
                                t.use_reusable_key_arena = reuse;
                                t.use_ordered_key_fastpath = ordered;
                                t.stage_profile = profiled;
                                let value = t.count((1..=p.n).collect(), 0).unwrap();
                                (value, t)
                            };
                            let (control, base) = run(false);
                            let (candidate, arena) = run(true);
                            assert_eq!(
                                (candidate.sat, &candidate.count.re, &candidate.count.im),
                                (control.sat, &control.count.re, &control.count.im)
                            );
                            assert_eq!(
                                (
                                    arena.stats.nodes,
                                    arena.stats.cache_hits,
                                    arena.stats.decompositions
                                ),
                                (
                                    base.stats.nodes,
                                    base.stats.cache_hits,
                                    base.stats.decompositions
                                )
                            );
                            assert_eq!(
                                (arena.ordered_key_probes, arena.ordered_key_hits),
                                (base.ordered_key_probes, base.ordered_key_hits)
                            );
                            assert_eq!(base.key_arena_calls, 0);
                            assert_eq!(base.key_arena_flat_reuses, 0);
                            assert_eq!(base.key_arena_view_reuses, 0);
                            assert!(arena.key_arena_calls > 0);
                            assert!(arena.key_arena_flat_reuses <= arena.key_arena_calls);
                            assert!(arena.key_arena_view_reuses <= arena.key_arena_calls);
                            assert!(arena.key_arena_flat.capacity() <= KEY_ARENA_MAX_LITERALS);
                            assert!(arena.key_arena_views.capacity() <= KEY_ARENA_MAX_VIEWS);
                            if profiled {
                                assert!(arena.stage_key_materialize_ns > 0);
                                assert!(
                                    arena.stage_key_materialize_ns
                                        + arena.stage_key_order_ns
                                        + arena.stage_key_pack_ns
                                        <= arena.stage_key_ns
                                );
                            } else {
                                assert_eq!(
                                    (
                                        arena.stage_key_materialize_ns,
                                        arena.stage_key_order_ns,
                                        arena.stage_key_pack_ns,
                                    ),
                                    (0, 0, 0)
                                );
                            }
                            assert!(arena.trail.is_empty());
                            assert!(arena.assigned.iter().all(|&value| value == 0));
                            observed_reuses += arena.key_arena_flat_reuses;
                        }
                    }
                }
            }
        }
        assert!(observed_reuses > 0);
    }

    #[test]
    fn bounded_key_arena_preserves_duplicates_inversions_and_rollback() {
        let src = "p cnf 4 3\nc t wmc\n1 2 0\n2 3 0\n1 4 0\n";
        for bits in [100_u32, 333] {
            let p = parse_emitted(src, bits).unwrap();
            for ordered in [false, true] {
                let mut control = TrailEngine::new(&p, bits, 128, 100_000);
                let mut candidate = TrailEngine::new(&p, bits, 128, 100_000);
                control.use_fused_compact = true;
                candidate.use_fused_compact = true;
                control.use_ordered_key_fastpath = ordered;
                candidate.use_ordered_key_fastpath = ordered;
                candidate.use_reusable_key_arena = true;
                let scope = (1..=p.n).collect::<Vec<_>>();
                for literal in [-1, -3] {
                    assert_eq!(
                        control.canonical_compact(&scope, None, false),
                        candidate.canonical_compact(&scope, None, false)
                    );
                    let checkpoint = control.trail.len();
                    assert_eq!(control.apply(literal, None), candidate.apply(literal, None));
                    // Verify the *live* reduced key, not only the value
                    // reconstructed after rolling the Trail back.
                    assert_eq!(
                        control.canonical_compact(&scope, None, false),
                        candidate.canonical_compact(&scope, None, false)
                    );
                    control.undo(checkpoint);
                    candidate.undo(checkpoint);
                    assert_eq!(
                        control.canonical_compact(&scope, None, false),
                        candidate.canonical_compact(&scope, None, false)
                    );
                }
                // Two committed assignments create duplicate reduced
                // clauses. A second sequence creates a Boolean conflict.
                // Neither case can poison the reclaimed scratch buffers.
                for sequence in [[-1, -3], [-1, -2]] {
                    let checkpoint = control.trail.len();
                    for literal in sequence {
                        assert_eq!(control.apply(literal, None), candidate.apply(literal, None));
                        assert_eq!(
                            control.canonical_compact(&scope, None, false),
                            candidate.canonical_compact(&scope, None, false)
                        );
                    }
                    control.undo(checkpoint);
                    candidate.undo(checkpoint);
                    assert_eq!(
                        control.canonical_compact(&scope, None, false),
                        candidate.canonical_compact(&scope, None, false)
                    );
                }
                assert!(candidate.key_arena_calls > 0);
                assert!(candidate.key_arena_flat_reuses > 0);
                assert!(candidate.key_arena_view_reuses > 0);
                assert!(candidate.trail.is_empty());
            }
        }
    }

    #[test]
    fn exact_scope_stamp_matches_binary_search_under_weighted_trail() {
        let sources = [
            "p cnf 4 4\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 4 0\n2 3 0\n-1 2 0\n-2 -3 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 3 2\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 2 0\n-2 3 0\n",
            "p cnf 4 3\nc t wmc\nc p weight 3 -0.125 0.25 0\nc p weight -3 0.5 -0.25 0\n1 2 0\n-1 3 0\n2 4 0\n",
        ];
        let mut observed = 0_u64;
        for bits in [100_u32, 333] {
            for source in sources {
                let p = parse_emitted(source, bits).unwrap();
                for capacity in [0_usize, 1, 128] {
                    let run = |stamped: bool| {
                        let mut t = TrailEngine::new(&p, bits, capacity, 100_000);
                        t.use_parent_ids = true;
                        t.use_compact_residual = true;
                        t.use_prehashed_cache = true;
                        t.use_fused_compact = true;
                        t.use_weight_class_cache = true;
                        t.use_inline_weight_relabel = true;
                        t.use_reusable_key_arena = true;
                        t.use_scope_stamp_admission = stamped;
                        let result = t.count((1..=p.n).collect(), 0).unwrap();
                        (result, t)
                    };
                    let (baseline, a) = run(false);
                    let (candidate, b) = run(true);
                    assert_eq!(
                        (baseline.sat, &baseline.count.re, &baseline.count.im),
                        (candidate.sat, &candidate.count.re, &candidate.count.im)
                    );
                    assert_eq!(
                        (a.stats.nodes, a.stats.cache_hits, a.stats.decompositions),
                        (b.stats.nodes, b.stats.cache_hits, b.stats.decompositions)
                    );
                    assert_eq!(a.scope_stamp_probes, 0);
                    assert!(b.scope_stamp_probes > 0);
                    assert_eq!(a.key_arena_calls, b.key_arena_calls);
                    assert_eq!(a.key_arena_flat_reuses, b.key_arena_flat_reuses);
                    assert_eq!(a.key_arena_view_reuses, b.key_arena_view_reuses);
                    assert!(b.trail.is_empty());
                    assert!(b.assigned.iter().all(|&value| value == 0));
                    observed += b.scope_stamp_probes;
                }
            }
        }
        assert!(observed > 0);
    }

    #[test]
    fn scope_stamp_exact_canonical_bytes_across_components_assignments_and_undo() {
        let source = "p cnf 5 5\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 2 0\n-1 2 0\n3 4 0\n-3 4 0\n5 0\n";
        let scopes = [vec![1, 2, 3, 4, 5], vec![1, 2], vec![3, 4], vec![5]];
        let assignments = [vec![], vec![-1], vec![-1, -2], vec![3]];
        for bits in [100_u32, 333] {
            let p = parse_emitted(source, bits).unwrap();
            for capacity in [0_usize, 128] {
                let build = |stamp: bool| {
                    let mut t = TrailEngine::new(&p, bits, capacity, 100_000);
                    t.use_fused_compact = true;
                    t.use_weight_class_cache = true;
                    t.use_inline_weight_relabel = true;
                    t.use_reusable_key_arena = true;
                    t.use_scope_stamp_admission = stamp;
                    t.initialize_weight_classes();
                    t
                };
                let mut control = build(false);
                let mut candidate = build(true);
                for assigned in &assignments {
                    let checkpoint = control.trail.len();
                    for &literal in assigned {
                        assert_eq!(control.apply(literal, None), candidate.apply(literal, None));
                    }
                    for scope in &scopes {
                        assert_eq!(
                            control.prepare_weighted_scope(scope).unwrap(),
                            candidate.prepare_weighted_scope(scope).unwrap()
                        );
                        // Complete, independently owned key bytes and original
                        // exact source clause IDs must match, including None
                        // on Boolean conflicts and out-of-component checks.
                        assert_eq!(
                            control.canonical_compact(scope, None, true),
                            candidate.canonical_compact(scope, None, true)
                        );
                    }
                    control.undo(checkpoint);
                    candidate.undo(checkpoint);
                    assert_eq!(
                        control.canonical_compact(&[1, 2, 3, 4, 5], None, false),
                        candidate.canonical_compact(&[1, 2, 3, 4, 5], None, false)
                    );
                }
                assert!(candidate.scope_stamp_probes > 0);
                assert_eq!(control.scope_stamp_probes, 0);
                assert!(control.trail.is_empty());
                assert!(candidate.trail.is_empty());
                assert!(candidate.assigned.iter().all(|&value| value == 0));
            }
        }
    }

    #[test]
    fn reversible_active_clause_bitmap_is_exact_across_multiword_undo() {
        let mut source = "p cnf 70 70\nc t wmc\n".to_owned();
        for variable in 1..=70 {
            source.push_str(&format!("{variable} 0\n"));
        }
        for bits in [100_u32, 333] {
            let p = parse_emitted(&source, bits).unwrap();
            let mut control = TrailEngine::new(&p, bits, 128, 100_000);
            let mut bitmap = TrailEngine::new(&p, bits, 128, 100_000);
            control.use_reusable_key_arena = true;
            bitmap.use_reusable_key_arena = true;
            bitmap.enable_active_clause_bitmap();
            assert_eq!(bitmap.active_clause_bits.as_ref().unwrap().len(), 2);
            assert_eq!(bitmap.active_clause_bits.as_ref().unwrap()[1], 63);
            let full = (1..=p.n).collect::<Vec<_>>();
            let parents = (0..p.clauses.len()).rev().collect::<Vec<_>>();
            for assignments in [vec![], vec![1], vec![1, 65], vec![-70]] {
                let checkpoint = control.trail.len();
                for &lit in &assignments {
                    assert_eq!(control.apply(lit, None), bitmap.apply(lit, None));
                }
                for cid in 0..bitmap.clauses.len() {
                    let present = bitmap.active_clause_bits.as_ref().unwrap()[cid / 64]
                        & (1_u64 << (cid % 64))
                        != 0;
                    assert_eq!(present, bitmap.satisfied[cid] == 0);
                }
                for use_parents in [false, true] {
                    control.use_parent_ids = use_parents;
                    bitmap.use_parent_ids = use_parents;
                    for scope in [&full[..], &[1, 2][..], &[65, 66, 67][..]] {
                        let candidate_ids = use_parents.then_some(parents.as_slice());
                        assert_eq!(
                            control.canonical_compact(scope, candidate_ids, false),
                            bitmap.canonical_compact(scope, candidate_ids, false)
                        );
                    }
                }
                control.undo(checkpoint);
                bitmap.undo(checkpoint);
                assert_eq!(
                    control.canonical_compact(&full, None, false),
                    bitmap.canonical_compact(&full, None, false)
                );
            }
            assert!(bitmap.active_bitmap_key_visits > 0);
            assert!(bitmap.trail.is_empty());
            assert!(bitmap.assigned.iter().all(|&value| value == 0));
        }
    }

    #[test]
    fn active_clause_bitmap_preserves_exact_weighted_solve_and_topology() {
        let sources = [
            "p cnf 4 4\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 -0.25 0.125 0\n1 4 0\n2 3 0\n-1 2 0\n-2 -3 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 4 3\nc t wmc\nc p weight 3 -0.125 0.25 0\nc p weight -3 0.5 -0.25 0\n1 2 0\n-1 3 0\n2 4 0\n",
        ];
        for bits in [100_u32, 333] {
            for source in sources {
                let p = parse_emitted(source, bits).unwrap();
                for capacity in [0_usize, 1, 128] {
                    let run = |use_bitmap: bool| {
                        let mut t = TrailEngine::new(&p, bits, capacity, 100_000);
                        t.use_parent_ids = true;
                        t.use_compact_residual = true;
                        t.use_prehashed_cache = true;
                        t.use_fused_compact = true;
                        t.use_weight_class_cache = true;
                        t.use_inline_weight_relabel = true;
                        t.use_reusable_key_arena = true;
                        if use_bitmap {
                            t.enable_active_clause_bitmap();
                        }
                        let answer = t.count((1..=p.n).collect(), 0).unwrap();
                        (answer, t)
                    };
                    let (expected, base) = run(false);
                    let (got, bitmap) = run(true);
                    assert_eq!(
                        (expected.sat, &expected.count.re, &expected.count.im),
                        (got.sat, &got.count.re, &got.count.im)
                    );
                    assert_eq!(
                        (base.stats.nodes, base.stats.cache_hits, base.stats.decompositions),
                        (
                            bitmap.stats.nodes,
                            bitmap.stats.cache_hits,
                            bitmap.stats.decompositions
                        )
                    );
                    assert_eq!(base.active_bitmap_key_visits, 0);
                    assert!(bitmap.active_bitmap_key_visits > 0);
                    assert!(bitmap.trail.is_empty());
                    assert!(bitmap.assigned.iter().all(|&value| value == 0));
                    for cid in 0..bitmap.clauses.len() {
                        let present = bitmap.active_clause_bits.as_ref().unwrap()[cid / 64]
                            & (1_u64 << (cid % 64))
                            != 0;
                        assert_eq!(present, bitmap.satisfied[cid] == 0);
                    }
                }
            }
        }
    }

}
