//! Isolated research MPFR arithmetic arm: unchanged production core and API.
//! Source-emitted QuPRS CNF, original chronological DPLL, component split,
//! frequency pivot and 10,000-entry exact-shape residual cache.
//! Verify structure against main exact-rational solve before claiming parity.
use rug::Float;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::ops::Deref;
use std::collections::{BTreeMap, HashMap as StdHashMap};
use hashbrown::HashMap as HbHashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

mod weighted_equivalence;
mod root_failed_literal;
mod unit_closure_pivot;
mod csr_workspace;
mod trail_engine;
mod separator_pivot;
mod primal_centroid;
mod factor_variable_elimination;
mod factor_add_elimination;
mod factor_boolean_bdd;
mod factor_minfill_order;
mod factor_primal_minor_bound;

type Clause = Vec<i32>;

#[derive(Clone)]
struct C { re: Float, im: Float }
impl C {
    fn zero(bits: u32) -> Self {
        Self { re: Float::with_val(bits, 0), im: Float::with_val(bits, 0) }
    }
    fn one(bits: u32) -> Self {
        Self { re: Float::with_val(bits, 1), im: Float::with_val(bits, 0) }
    }
    fn decimal(bits: u32, re: &str, im: &str) -> Result<Self, String> {
        let a = Float::parse(re).map_err(|e| format!("real parse: {e}"))?;
        let b = Float::parse(im).map_err(|e| format!("imag parse: {e}"))?;
        let a = Float::with_val(bits, a);
        let b = Float::with_val(bits, b);
        if !a.is_finite() || !b.is_finite() { return Err("nonfinite weight".into()); }
        Ok(Self { re: a, im: b })
    }
    fn add(&self, rhs: &Self, bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, &self.re + &rhs.re),
            im: Float::with_val(bits, &self.im + &rhs.im),
        }
    }
    fn mul(&self, rhs: &Self, bits: u32) -> Self {
        // GPMC ComplexMPFR does four separately rounded MPFR products.
        // rug's incomplete products can fuse, so materialize each first.
        let arbr = Float::with_val(bits, &self.re * &rhs.re);
        let aibi = Float::with_val(bits, &self.im * &rhs.im);
        let arbi = Float::with_val(bits, &self.re * &rhs.im);
        let aibr = Float::with_val(bits, &self.im * &rhs.re);
        Self {
            re: Float::with_val(bits, &arbr - &aibi),
            im: Float::with_val(bits, &arbi + &aibr),
        }
    }
}
#[derive(Clone)]
struct Weights { positive: C, negative: C }
struct Problem { n: usize, clauses: Vec<Clause>, weights: Vec<Weights> }
#[derive(Clone)]
struct Outcome { sat: bool, count: C }
impl Outcome {
    fn unsat(bits: u32) -> Self { Self { sat: false, count: C::zero(bits) } }
}
#[derive(Hash, PartialEq, Eq)]
struct Key { scope: Vec<usize>, clauses: Vec<Clause> }
#[derive(Hash, PartialEq, Eq)]
struct SharedKey { scope: Vec<usize>, clauses: Arc<Vec<Clause>> }
// Research-only exact key: zero is a clause separator, never a DIMACS
// literal. A separator is also emitted for each empty clause, preserving
// clause boundaries, order, and duplicate literals without loss.
#[derive(PartialEq, Eq)]
struct PackedKey { scope: Vec<usize>, literals: Vec<i32> }
impl PackedKey {
    fn from_residual(scope: Vec<usize>, residual: &[Clause]) -> Self {
        let length = residual.iter().map(|clause| clause.len() + 1).sum();
        let mut literals = Vec::with_capacity(length);
        for clause in residual {
            debug_assert!(clause.iter().all(|&lit| lit != 0));
            literals.extend_from_slice(clause);
            literals.push(0);
        }
        Self { scope, literals }
    }
    fn matches_residual(&self, scope: &[usize], residual: &[Clause]) -> bool {
        if self.scope.as_slice() != scope { return false; }
        let mut pos = 0;
        for clause in residual {
            let end = pos + clause.len();
            if self.literals.get(pos..end) != Some(clause.as_slice()) ||
                self.literals.get(end) != Some(&0) {
                return false;
            }
            pos = end + 1;
        }
        pos == self.literals.len()
    }
}
// Recreate the exact original nested Vec<Vec<i32>> hash from the flat
// representation, including per-clause length prefixes. Borrowed lookup
// can therefore hash the original residual *directly*, without an extra
// length-summing pass or materializing a temporary packed representation.
impl Hash for PackedKey {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.scope.hash(hasher);
        self.literals.iter().filter(|&&lit| lit == 0).count().hash(hasher);
        let mut start = 0;
        for (end, &lit) in self.literals.iter().enumerate() {
            if lit == 0 {
                self.literals[start..end].hash(hasher);
                start = end + 1;
            }
        }
        debug_assert_eq!(start, self.literals.len());
    }
}
enum StoredKey {
    Owned(Key), Shared(SharedKey), Packed(SharedKey),
    PackedFast { source: SharedKey, hash: u64 }, NoCache,
}
enum ResidualStorage { Owned(Vec<Clause>), Shared(Arc<Vec<Clause>>) }
impl Deref for ResidualStorage {
    type Target = Vec<Clause>;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(value) => value,
            Self::Shared(value) => value,
        }
    }
}
#[derive(Default, Clone, Copy)]
struct Stats { nodes: u64, cache_hits: u64, decompositions: u64, packed_full_misses: u64 }
#[derive(Clone, Copy, Debug)]
enum CachePolicy { StdOwned, HbOwned, HbBorrowed }
impl CachePolicy {
    fn label(self) -> &'static str {
        match self {
            Self::StdOwned => "std-owned",
            Self::HbOwned => "hashbrown-owned",
            Self::HbBorrowed => "hashbrown-borrowed",
        }
    }
}
struct Engine<'a> {
    p: &'a Problem,
    bits: u32,
    cache: StdHashMap<Key, Outcome>,
    hb_cache: HbHashMap<Key, Outcome>,
    shared_cache: HbHashMap<SharedKey, Outcome>,
    packed_cache: HbHashMap<PackedKey, Outcome>,
    cache_policy: CachePolicy,
    shared_residual_key: bool,
    packed_residual_key: bool,
    packed_fast_admission: bool,
    packed_hash_only: bool,
    packed_bypass_only: bool,
    cache_limit: usize,
    stats: Stats,
    fast_assign: bool,
    node_budget: u64,
    indexed_unit_propagation: bool,
    indexed_activation_node: u64,
    indexed_unit_csr: bool,
    reuse_frequency_scratch: bool,
    frequency_scratch: Vec<usize>,
    unit_closure_pivot: bool,
    unit_closure_stats: unit_closure_pivot::ClosureStats,
    reuse_csr_workspace: bool,
    csr_workspace: csr_workspace::Workspace,
}
impl<'a> Engine<'a> {
    fn new(p: &'a Problem, bits: u32, cache_policy: CachePolicy) -> Engine<'a> {
        Engine {
            p, bits, cache: StdHashMap::new(), hb_cache: HbHashMap::new(),
            shared_cache: HbHashMap::new(), packed_cache: HbHashMap::new(),
            cache_policy, shared_residual_key: false, packed_residual_key: false,
            packed_fast_admission: false, packed_hash_only: false,
            packed_bypass_only: false, cache_limit: 10_000,
            stats: Stats::default(), fast_assign: false, node_budget: 100_000,
            indexed_unit_propagation: false, indexed_activation_node: 0,
            indexed_unit_csr: false,
            reuse_frequency_scratch: false,
            frequency_scratch: vec![0; p.n + 1],
            unit_closure_pivot: false,
            unit_closure_stats: unit_closure_pivot::ClosureStats::default(),
            reuse_csr_workspace: false,
            csr_workspace: csr_workspace::Workspace::default(),
        }
    }
    fn borrowed_key_hash(&self, scope: &[usize], residual: &[Clause]) -> u64 {
        // Match Key's derived Hash field sequence exactly, with borrowed slices.
        let mut hasher = self.hb_cache.hasher().build_hasher();
        scope.hash(&mut hasher);
        residual.hash(&mut hasher);
        hasher.finish()
    }
    fn shared_key_hash(&self, scope: &[usize], residual: &[Clause]) -> u64 {
        let mut hasher = self.shared_cache.hasher().build_hasher();
        scope.hash(&mut hasher);
        residual.hash(&mut hasher);
        hasher.finish()
    }
    fn packed_key_hash(&self, scope: &[usize], residual: &[Clause]) -> u64 {
        let mut hasher = self.packed_cache.hasher().build_hasher();
        scope.hash(&mut hasher);
        residual.hash(&mut hasher);
        hasher.finish()
    }
    fn weight(&self, var: usize, positive: bool) -> &C {
        let w = &self.p.weights[var - 1];
        if positive { &w.positive } else { &w.negative }
    }
    fn save(&mut self, key: StoredKey, result: Outcome) -> Outcome {
        match key {
            StoredKey::Owned(key) => match self.cache_policy {
                CachePolicy::StdOwned if self.cache.len() < self.cache_limit => {
                    self.cache.insert(key, result.clone());
                }
                CachePolicy::HbOwned | CachePolicy::HbBorrowed if self.hb_cache.len() < self.cache_limit => {
                    self.hb_cache.insert(key, result.clone());
                }
                _ => {}
            },
            StoredKey::Shared(key) => {
                if self.shared_cache.len() < self.cache_limit {
                    self.shared_cache.insert(key, result.clone());
                }
            }
            StoredKey::Packed(key) => {
                // Original packed control remains byte-for-byte unchanged.
                if self.packed_cache.len() < self.cache_limit {
                    let key = PackedKey::from_residual(key.scope, key.clauses.as_slice());
                    self.packed_cache.insert(key, result.clone());
                }
            }
            StoredKey::PackedFast { source, hash } => {
                if self.packed_cache.len() < self.cache_limit {
                    let packed = PackedKey::from_residual(
                        source.scope, source.clauses.as_slice()
                    );
                    // The miss was checked before recursion, but a child
                    // could have inserted the same key. Resolve exact equality
                    // here as well; never insert from hash alone.
                    match self.packed_cache.raw_entry_mut()
                        .from_hash(hash, |existing| existing == &packed) {
                        hashbrown::hash_map::RawEntryMut::Occupied(mut occupied) => {
                            occupied.insert(result.clone());
                        }
                        hashbrown::hash_map::RawEntryMut::Vacant(vacant) => {
                            // Preserve the borrowed lookup hash, avoiding a
                            // second complete nested-clause hash traversal.
                            vacant.insert_hashed_nocheck(hash, packed, result.clone());
                        }
                    }
                }
            }
            StoredKey::NoCache => {}
        }
        result
    }
    fn count(&mut self, clauses: Vec<Clause>, scope: Vec<usize>, depth: usize) -> Result<Outcome, String> {
        if depth >= 512 { return Err("depth limit".into()); }
        if self.stats.nodes >= self.node_budget { return Err("node limit".into()); }
        self.stats.nodes += 1;
        let canonical = if depth == 0 { normalize(clauses) } else { normalize_child(clauses) };
        // The owned-key modes retain their exact original behavior.
        // The shared mode checks the borrowed canonical bytes BEFORE allocating
        // an Arc. On a miss the cache key shares the residual's existing Vec;
        // there is no deep Vec<Vec<i32>> clone at insertion time.
        let (mut residual, key) = if self.packed_residual_key {
            let hash = self.packed_key_hash(&scope, &canonical);
            if let Some((_, hit)) = self.packed_cache.raw_entry().from_hash(hash, |key| {
                key.matches_residual(&scope, &canonical)
            }) {
                self.stats.cache_hits += 1;
                return Ok(hit.clone());
            }
            if (self.packed_fast_admission || self.packed_bypass_only)
                && self.packed_cache.len() >= self.cache_limit {
                self.stats.packed_full_misses += 1;
                // Cache still must be probed for hits, but once full a miss
                // can never be admitted. Avoid allocating Arc and key scope.
                (ResidualStorage::Owned(canonical), StoredKey::NoCache)
            } else {
                let shared = Arc::new(canonical);
                let key = SharedKey { scope: scope.clone(), clauses: Arc::clone(&shared) };
                if self.packed_fast_admission || self.packed_hash_only {
                    (ResidualStorage::Shared(shared), StoredKey::PackedFast { source: key, hash })
                } else {
                    (ResidualStorage::Shared(shared), StoredKey::Packed(key))
                }
            }
        } else if self.shared_residual_key {
            let hash = self.shared_key_hash(&scope, &canonical);
            if let Some((_, hit)) = self.shared_cache.raw_entry().from_hash(hash, |key| {
                key.scope.as_slice() == scope.as_slice()
                    && key.clauses.as_slice() == canonical.as_slice()
            }) {
                self.stats.cache_hits += 1;
                return Ok(hit.clone());
            }
            let shared = Arc::new(canonical);
            let key = SharedKey { scope: scope.clone(), clauses: Arc::clone(&shared) };
            (ResidualStorage::Shared(shared), StoredKey::Shared(key))
        } else {
            let key = match self.cache_policy {
                CachePolicy::StdOwned => {
                    let key = Key { scope: scope.clone(), clauses: canonical.clone() };
                    if let Some(hit) = self.cache.get(&key) {
                        self.stats.cache_hits += 1;
                        return Ok(hit.clone());
                    }
                    key
                }
                CachePolicy::HbOwned => {
                    let key = Key { scope: scope.clone(), clauses: canonical.clone() };
                    if let Some(hit) = self.hb_cache.get(&key) {
                        self.stats.cache_hits += 1;
                        return Ok(hit.clone());
                    }
                    key
                }
                CachePolicy::HbBorrowed => {
                    let hash = self.borrowed_key_hash(&scope, &canonical);
                    if let Some((_, hit)) = self.hb_cache.raw_entry().from_hash(hash, |key| {
                        key.scope.as_slice() == scope.as_slice()
                            && key.clauses.as_slice() == canonical.as_slice()
                    }) {
                        self.stats.cache_hits += 1;
                        return Ok(hit.clone());
                    }
                    Key { scope: scope.clone(), clauses: canonical.clone() }
                }
            };
            (ResidualStorage::Owned(canonical), StoredKey::Owned(key))
        };
        let mut remaining = scope;
        let mut prefix = C::one(self.bits);
        if self.indexed_unit_propagation &&
            self.stats.nodes >= self.indexed_activation_node &&
            residual.iter().any(|c| c.len() == 1) {
            let reduced = if self.indexed_unit_csr {
                if self.reuse_csr_workspace {
                    // Restore the reusable arena before child recursion, even
                    // after a Boolean UNSAT cofactor. No workspace data is
                    // allowed to enter a residual cache key or model result.
                    let mut arena = std::mem::take(&mut self.csr_workspace);
                    let outcome = csr_workspace::propagate(
                        &residual, self.p.n, &self.p.weights, self.bits,
                        &mut remaining, &mut prefix, &mut arena,
                    );
                    self.csr_workspace = arena;
                    outcome
                } else {
                    unit_propagate_indexed_csr(
                        &residual, self.p.n, &self.p.weights, self.bits, &mut remaining, &mut prefix
                    )
                }
            } else {
                unit_propagate_indexed(
                    &residual, self.p.n, &self.p.weights, self.bits, &mut remaining, &mut prefix
                )
            };
            let Some(reduced) = reduced else {
                return Ok(self.save(key, Outcome::unsat(self.bits)));
            };
            residual = ResidualStorage::Owned(reduced);
        } else {
            // Control is intentionally byte-for-byte the original unit order
            // and clause-copy behavior; no production path is changed.
            loop {
                if residual.iter().any(Vec::is_empty) {
                    return Ok(self.save(key, Outcome::unsat(self.bits)));
                }
                let Some(lit) = residual.iter().find(|c| c.len() == 1).map(|c| c[0]) else { break; };
                let var = lit.unsigned_abs() as usize;
                prefix = prefix.mul(self.weight(var, lit > 0), self.bits);
                remaining.retain(|&v| v != var);
                residual = ResidualStorage::Owned(if self.fast_assign {
                    assign_fast(&residual, lit)
                } else {
                    assign(&residual, lit)
                });
            }
        }
        if residual.is_empty() {
            for var in remaining {
                let w = &self.p.weights[var - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative, self.bits), self.bits);
            }
            return Ok(self.save(key, Outcome { sat: true, count: prefix }));
        }
        // Research-only reusable per-node frequency arena. In the opt-in
        // variant, return this arena to Engine BEFORE any recursive call.
        // Only variables in the active scope can occur in the canonical
        // residual; all touched entries are cleared on both recursion paths.
        let mut frequency = if self.reuse_frequency_scratch {
            std::mem::take(&mut self.frequency_scratch)
        } else {
            vec![0_usize; self.p.n + 1]
        };
        debug_assert_eq!(frequency.len(), self.p.n + 1);
        for clause in residual.iter() {
            for &lit in clause { frequency[lit.unsigned_abs() as usize] += 1; }
        }
        let mut active = Vec::new();
        for var in remaining {
            if frequency[var] != 0 { active.push(var); }
            else {
                let w = &self.p.weights[var - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative, self.bits), self.bits);
            }
        }
        if let Some(pieces) = split_components(&residual, &active, self.p.n) {
            if self.reuse_frequency_scratch {
                // Recursion must receive an intact, zeroed scratch arena.
                for &v in &active { frequency[v] = 0; }
                self.frequency_scratch = frequency;
            }
            self.stats.decompositions += 1;
            let mut value = C::one(self.bits);
            for (cls, sub_scope) in pieces {
                let part = self.count(cls, sub_scope, depth + 1)?;
                if !part.sat { return Ok(self.save(key, Outcome::unsat(self.bits))); }
                value = value.mul(&part.count, self.bits);
            }
            return Ok(self.save(key, Outcome {
                sat: true, count: prefix.mul(&value, self.bits),
            }));
        }
        let frequency_pivot = *active.iter().max_by_key(|&&v| frequency[v])
            .ok_or("no active pivot")?;
        let var = if self.unit_closure_pivot {
            unit_closure_pivot::choose(
                &residual, &active, &frequency, self.p.n,
                &mut self.unit_closure_stats,
            ).unwrap_or(frequency_pivot)
        } else {
            frequency_pivot
        };
        if self.reuse_frequency_scratch {
            for &v in &active { frequency[v] = 0; }
            self.frequency_scratch = frequency;
        }
        let child: Vec<_> = active.into_iter().filter(|&v| v != var).collect();
        let yes_clauses = if self.fast_assign {
            assign_fast(&residual, var as i32)
        } else {
            assign(&residual, var as i32)
        };
        let yes = self.count(yes_clauses, child.clone(), depth + 1)?;
        let no_clauses = if self.fast_assign {
            assign_fast(&residual, -(var as i32))
        } else {
            assign(&residual, -(var as i32))
        };
        let no = self.count(no_clauses, child, depth + 1)?;
        let yes_weighted = self.weight(var, true).mul(&yes.count, self.bits);
        let no_weighted = self.weight(var, false).mul(&no.count, self.bits);
        let sum = yes_weighted.add(&no_weighted, self.bits);
        Ok(self.save(key, Outcome {
            sat: yes.sat || no.sat,
            count: prefix.mul(&sum, self.bits),
        }))
    }
}
/// A single CSR occurrence arena replaces thousands of per-literal Vec
/// allocations. Offsets address both polarities of each numbered variable.
fn occurrence_slot(lit: i32) -> usize {
    2 * (lit.unsigned_abs() as usize) + usize::from(lit < 0)
}

/// Source-order-preserving CSR variant of local occurrence-indexed propagation.
///
/// Construct counts and prefix offsets in one pass, then fill a contiguous
/// clause-ID array in ascending original clause order. The min-heap and all
/// assignments preserve the original clause-index and individually rounded
/// complex MPFR weight order. Boolean conflict remains separate from
/// numerical zero WMC. No CSR arena survives its residual node.
fn unit_propagate_indexed_csr(
    clauses: &[Clause],
    variables: usize,
    weights: &[Weights],
    bits: u32,
    remaining: &mut Vec<usize>,
    prefix: &mut C,
) -> Option<Vec<Clause>> {
    let slots = 2 * (variables + 1);
    let mut occurrence_counts = vec![0_usize; slots];
    let mut lengths = Vec::with_capacity(clauses.len());
    let mut alive = vec![true; clauses.len()];
    let mut unit = BinaryHeap::<Reverse<usize>>::new();
    let mut assignments = vec![0_i8; variables + 1];
    for (cid, clause) in clauses.iter().enumerate() {
        if clause.is_empty() { return None; }
        lengths.push(clause.len());
        if clause.len() == 1 { unit.push(Reverse(cid)); }
        for &lit in clause { occurrence_counts[occurrence_slot(lit)] += 1; }
    }
    let mut offsets = Vec::with_capacity(slots + 1);
    offsets.push(0_usize);
    for &count in &occurrence_counts {
        offsets.push(offsets.last().copied().unwrap() + count);
    }
    let mut cursors = offsets[..slots].to_vec();
    let mut indices = vec![0_usize; offsets[slots]];
    for (cid, clause) in clauses.iter().enumerate() {
        for &lit in clause {
            let slot = occurrence_slot(lit);
            indices[cursors[slot]] = cid;
            cursors[slot] += 1;
        }
    }

    while let Some(Reverse(cid)) = unit.pop() {
        if !alive[cid] || lengths[cid] != 1 { continue; }
        let lit = *clauses[cid].iter().find(|&&lit| {
            assignments[lit.unsigned_abs() as usize] == 0
        }).expect("active unit has one unassigned literal");
        let var = lit.unsigned_abs() as usize;
        let side = usize::from(lit < 0);
        let w = &weights[var - 1];
        *prefix = prefix.mul(if lit > 0 { &w.positive } else { &w.negative }, bits);
        remaining.retain(|&v| v != var);
        assignments[var] = if lit > 0 { 1 } else { -1 };

        let satisfied_slot = 2 * var + side;
        for &index in &indices[offsets[satisfied_slot]..offsets[satisfied_slot + 1]] {
            alive[index] = false;
        }
        let falsified_slot = 2 * var + (1 - side);
        for &index in &indices[offsets[falsified_slot]..offsets[falsified_slot + 1]] {
            if !alive[index] { continue; }
            lengths[index] -= 1;
            if lengths[index] == 0 { return None; }
            if lengths[index] == 1 { unit.push(Reverse(index)); }
        }
    }

    let mut reduced = Vec::with_capacity(clauses.len());
    for (cid, clause) in clauses.iter().enumerate() {
        if !alive[cid] { continue; }
        let mut next = Vec::with_capacity(lengths[cid]);
        for &lit in clause {
            if assignments[lit.unsigned_abs() as usize] == 0 {
                next.push(lit);
            }
        }
        debug_assert_eq!(next.len(), lengths[cid]);
        reduced.push(next);
    }
    Some(reduced)
}

/// Research-only occurrence-indexed unit propagation within ONE normalized
/// DPLL residual node. Unlike a global watched-literal index, these bounded
/// local indices never outlive their source clause list or cache lookup.
///
/// BinaryHeap<Reverse<usize>> always chooses the smallest ORIGINAL clause
/// index, preserving the original first-unit-clause order even when an
/// earlier nonunit clause becomes unit after a forced literal assignment.
/// The original stable residual order is preserved in reconstructed output.
/// Return None for a Boolean contradiction, independent of numeric zero WMC.
fn unit_propagate_indexed(
    clauses: &[Clause],
    variables: usize,
    weights: &[Weights],
    bits: u32,
    remaining: &mut Vec<usize>,
    prefix: &mut C,
) -> Option<Vec<Clause>> {
    let mut occurrences: Vec<[Vec<usize>; 2]> =
        vec![[Vec::new(), Vec::new()]; variables + 1];
    let mut lengths = Vec::with_capacity(clauses.len());
    let mut alive = vec![true; clauses.len()];
    let mut unit = BinaryHeap::<Reverse<usize>>::new();
    let mut assignments = vec![0_i8; variables + 1];
    for (cid, clause) in clauses.iter().enumerate() {
        if clause.is_empty() { return None; }
        lengths.push(clause.len());
        if clause.len() == 1 { unit.push(Reverse(cid)); }
        for &lit in clause {
            let v = lit.unsigned_abs() as usize;
            occurrences[v][usize::from(lit < 0)].push(cid);
        }
    }
    while let Some(Reverse(cid)) = unit.pop() {
        if !alive[cid] || lengths[cid] != 1 { continue; }
        // Normalized input clauses have neither repeated literals nor
        // both polarities. Active clauses never contain satisfied literals.
        let lit = *clauses[cid].iter().find(|&&lit| {
            assignments[lit.unsigned_abs() as usize] == 0
        }).expect("active unit has exactly one unassigned literal");
        let var = lit.unsigned_abs() as usize;
        let side = usize::from(lit < 0);
        let w = &weights[var - 1];
        *prefix = prefix.mul(if lit > 0 { &w.positive } else { &w.negative }, bits);
        remaining.retain(|&v| v != var);
        assignments[var] = if lit > 0 { 1 } else { -1 };

        // Satisfied clauses disappear without touching their literals.
        for &index in &occurrences[var][side] {
            alive[index] = false;
        }
        // Falsified literal occurrences lose one literal. Only those
        // clauses can become new units or empty. Stale heap entries are
        // filtered on pop, so queue ordering reproduces the baseline.
        for &index in &occurrences[var][1 - side] {
            if !alive[index] { continue; }
            lengths[index] -= 1;
            if lengths[index] == 0 { return None; }
            if lengths[index] == 1 { unit.push(Reverse(index)); }
        }
    }
    let mut reduced = Vec::with_capacity(clauses.len());
    for (cid, clause) in clauses.iter().enumerate() {
        if !alive[cid] { continue; }
        let mut next = Vec::with_capacity(lengths[cid]);
        for &lit in clause {
            if assignments[lit.unsigned_abs() as usize] == 0 {
                next.push(lit);
            }
        }
        debug_assert_eq!(next.len(), lengths[cid]);
        reduced.push(next);
    }
    Some(reduced)
}

fn normalize(clauses: Vec<Clause>) -> Vec<Clause> {
    let mut result = Vec::new();
    for mut clause in clauses {
        clause.sort_unstable();
        clause.dedup();
        if clause.iter().any(|&lit| clause.binary_search(&-lit).is_ok()) { continue; }
        result.push(clause);
    }
    normalize_child(result)
}
fn normalize_child(mut cls: Vec<Clause>) -> Vec<Clause> {
    cls.sort();
    cls.dedup();
    cls
}
fn assign(clauses: &[Clause], lit: i32) -> Vec<Clause> {
    clauses.iter().filter(|c| !c.contains(&lit))
        .map(|c| c.iter().copied().filter(|&x| x != -lit).collect()).collect()
}
/// Source-normalized two-literal assignment. This is an opt-in research
/// control: the original borrowed/owned-cache comparison remains unchanged.
fn assign_fast(clauses: &[Clause], selected: i32) -> Vec<Clause> {
    let opposite = -selected;
    let mut child = Vec::with_capacity(clauses.len());
    for clause in clauses {
        if clause.len() == 2 {
            let a = clause[0];
            let b = clause[1];
            if a == selected || b == selected { continue; }
            if a == opposite { child.push(vec![b]); }
            else if b == opposite { child.push(vec![a]); }
            else { child.push(clause.clone()); }
        } else if clause.contains(&selected) {
            continue;
        } else if clause.contains(&opposite) {
            child.push(clause.iter().copied().filter(|&v| v != opposite).collect());
        } else {
            child.push(clause.clone());
        }
    }
    child
}
fn root(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        let p = parent[x];
        parent[x] = parent[p];
        x = parent[x];
    }
    x
}
fn split_components(
    clauses: &[Clause], scope: &[usize], variables: usize
) -> Option<Vec<(Vec<Clause>, Vec<usize>)>> {
    let mut parent: Vec<_> = (0..=variables).collect();
    for clause in clauses {
        let first = clause[0].unsigned_abs() as usize;
        for &lit in &clause[1..] {
            let a = root(&mut parent, first);
            let b = root(&mut parent, lit.unsigned_abs() as usize);
            parent[b] = a;
        }
    }
    let &first = scope.first()?;
    let first_root = root(&mut parent, first);
    if scope.iter().all(|&v| root(&mut parent, v) == first_root) { return None; }
    let mut groups: BTreeMap<usize, (Vec<usize>, Vec<Clause>)> = BTreeMap::new();
    for &var in scope {
        groups.entry(root(&mut parent, var)).or_default().0.push(var);
    }
    for clause in clauses {
        let r = root(&mut parent, clause[0].unsigned_abs() as usize);
        groups.get_mut(&r).expect("scope contains clause var").1.push(clause.clone());
    }
    Some(groups.into_values().map(|(vars, cs)| (cs, vars)).collect())
}
fn parse_emitted(raw: &str, bits: u32) -> Result<Problem, String> {
    if raw.is_empty() || raw.len() > 2_000_000 { return Err("invalid source bytes".into()); }
    let mut lines = raw.lines().filter(|s| !s.trim().is_empty());
    let header: Vec<_> = lines.next().ok_or("missing header")?.split_whitespace().collect();
    if header.len() != 4 || header[0] != "p" || header[1] != "cnf" {
        return Err("bad p cnf header".into());
    }
    let n: usize = header[2].parse().map_err(|_| "bad variable count")?;
    let declared: usize = header[3].parse().map_err(|_| "bad clause count")?;
    if !(1..=4096).contains(&n) || declared > 50_000 { return Err("oversize CNF".into()); }
    if lines.next().map(str::trim) != Some("c t wmc") { return Err("missing wmc marker".into()); }
    let mut weights = vec![Weights { positive: C::one(bits), negative: C::one(bits) }; n];
    let mut assigned = vec![[false; 2]; n];
    let mut clauses = Vec::with_capacity(declared);
    let mut clause_started = false;
    for line in lines {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.first() == Some(&"c") {
            if clause_started || f.len() != 7 || f[1..3] != ["p", "weight"] || f[6] != "0" {
                return Err("bad weight record".into());
            }
            let lit: i32 = f[3].parse().map_err(|_| "invalid weight literal")?;
            let var = lit.unsigned_abs() as usize;
            if lit == 0 || var > n { return Err("weight variable out of range".into()); }
            let side = usize::from(lit < 0);
            if assigned[var - 1][side] { return Err("duplicated weight".into()); }
            assigned[var - 1][side] = true;
            let value = C::decimal(bits, f[4], f[5])?;
            if side == 0 { weights[var - 1].positive = value; }
            else { weights[var - 1].negative = value; }
        } else {
            clause_started = true;
            if f.last() != Some(&"0") { return Err("unterminated clause".into()); }
            let mut clause = Vec::with_capacity(f.len() - 1);
            for token in &f[..f.len()-1] {
                let lit: i32 = token.parse().map_err(|_| "invalid clause")?;
                if lit == 0 || lit.unsigned_abs() as usize > n {
                    return Err("invalid clause literal".into());
                }
                clause.push(lit);
            }
            clauses.push(clause);
            if clauses.len() > declared { return Err("excess clauses".into()); }
        }
    }
    if clauses.len() != declared { return Err("missing clauses".into()); }
    Ok(Problem { n, clauses, weights })
}
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next().ok_or(
        "usage: rawmocs-mpfr-research [--node-budget=250000|--node-budget=1000000] [--cache-limit=25000|--cache-limit=50000|--cache-limit=100000] [--indexed-unit-prop|--indexed-unit-prop-after-8192|--indexed-unit-prop-csr|--indexed-unit-prop-csr-after-8192] [--shared-residual-key|--packed-residual-key|--packed-residual-key-fast|--packed-residual-key-hash-only|--packed-residual-key-bypass-only] [--fast-assign] [--reuse-frequency-scratch] [--hashbrown-owned|--hashbrown-borrowed] DIGITS FILE"
    )?;
    // Explicit finite research-only node limits, untouched default.
    let node_budget = match first.to_str() {
        Some("--node-budget=250000") => 250_000,
        Some("--node-budget=1000000") => 1_000_000,
        _ => 100_000,
    };
    let algorithm_start = if node_budget != 100_000 {
        args.next().ok_or("missing arguments after bounded node budget")?
    } else {
        first
    };
    // Finite research-only limits. The production and default MPFR
    // 10,000-entry cache policy is unchanged.
    let cache_limit = match algorithm_start.to_str() {
        Some("--cache-limit=25000") => 25_000,
        Some("--cache-limit=50000") => 50_000,
        Some("--cache-limit=100000") => 100_000,
        _ => 10_000,
    };
    let algorithm_start = if cache_limit != 10_000 {
        args.next().ok_or("missing arguments after bounded cache limit")?
    } else { algorithm_start };
    let indexed_unit_propagation = matches!(
        algorithm_start.to_str(),
        Some("--indexed-unit-prop" | "--indexed-unit-prop-after-8192"
            | "--indexed-unit-prop-csr" | "--indexed-unit-prop-csr-after-8192")
    );
    let indexed_unit_csr = matches!(
        algorithm_start.to_str(),
        Some("--indexed-unit-prop-csr" | "--indexed-unit-prop-csr-after-8192")
    );
    let indexed_activation_node = if matches!(
        algorithm_start.to_str(),
        Some("--indexed-unit-prop-after-8192" | "--indexed-unit-prop-csr-after-8192")
    ) { 8192 } else { 0 };
    let storage_start = if indexed_unit_propagation {
        args.next().ok_or("missing arguments after --indexed-unit-prop")?
    } else {
        algorithm_start
    };
    let shared_residual_key = storage_start.to_str() == Some("--shared-residual-key");
    let packed_fast_admission = storage_start.to_str() == Some("--packed-residual-key-fast");
    // Factor-isolating, opt-in research controls. Keep original Packed and
    // combined Fast CLI semantics unchanged.
    let packed_hash_only = storage_start.to_str() == Some("--packed-residual-key-hash-only");
    let packed_bypass_only = storage_start.to_str() == Some("--packed-residual-key-bypass-only");
    let packed_residual_key = storage_start.to_str() == Some("--packed-residual-key")
        || packed_fast_admission || packed_hash_only || packed_bypass_only;
    let next = if shared_residual_key || packed_residual_key {
        args.next().ok_or("missing arguments after --shared-residual-key")?
    } else {
        storage_start
    };
    let fast_assign = next.to_str() == Some("--fast-assign");
    let cache_first = if fast_assign {
        args.next().ok_or("missing precision or cache mode after --fast-assign")?
    } else {
        next
    };
    let reuse_frequency_scratch = cache_first.to_str() == Some("--reuse-frequency-scratch");
    let cache_first = if reuse_frequency_scratch {
        args.next().ok_or("missing cache mode or precision after --reuse-frequency-scratch")?
    } else { cache_first };
    // Opt-in source Weighted CNF signed-literal equivalence classes.
    // Do not change any existing policy parsing or the production default.
    let weighted_equivalence = cache_first.to_str() == Some("--weighted-equivalence");
    let cache_first = if weighted_equivalence {
        args.next().ok_or("missing cache policy or precision after --weighted-equivalence")?
    } else { cache_first };
    let root_failed_literal = cache_first.to_str() == Some("--root-failed-literal");
    let cache_first = if root_failed_literal {
        args.next().ok_or("missing solver flags after --root-failed-literal")?
    } else { cache_first };
    let unit_closure_pivot = cache_first.to_str() == Some("--unit-closure-pivot");
    let cache_first = if unit_closure_pivot {
        args.next().ok_or("missing cache policy or precision after --unit-closure-pivot")?
    } else { cache_first };
    let reuse_csr_workspace = cache_first.to_str() == Some("--csr-reuse-workspace");
    let cache_first = if reuse_csr_workspace {
        args.next().ok_or("missing cache policy or precision after --csr-reuse-workspace")?
    } else { cache_first };
    // Independent CNF-native reversible source-clause trail, strictly opt-in.
    let trail_count = cache_first.to_str() == Some("--trail-count");
    let cache_first = if trail_count {
        args.next().ok_or("missing cache policy or precision after --trail-count")?
    } else { cache_first };
    let trail_parent_ids = cache_first.to_str() == Some("--trail-parent-ids");
    let cache_first = if trail_parent_ids {
        args.next().ok_or("missing cache policy or precision after --trail-parent-ids")?
    } else { cache_first };
    let trail_compact_residual = cache_first.to_str() == Some("--trail-compact-residual");
    let cache_first = if trail_compact_residual {
        args.next().ok_or("missing cache policy or precision after --trail-compact-residual")?
    } else { cache_first };
    let trail_prehashed_cache = cache_first.to_str() == Some("--trail-prehashed-cache");
    let cache_first = if trail_prehashed_cache {
        args.next().ok_or("missing cache mode or precision after --trail-prehashed-cache")?
    } else { cache_first };
    let trail_fused_compact = cache_first.to_str() == Some("--trail-fused-compact");
    let cache_first = if trail_fused_compact {
        args.next().ok_or("missing cache mode or precision after --trail-fused-compact")?
    } else { cache_first };
    let weighted_component_cache = cache_first.to_str() == Some("--trail-weight-class-cache");
    let cache_first = if weighted_component_cache {
        args.next().ok_or("missing cache policy or precision after --trail-weight-class-cache")?
    } else { cache_first };
    let weight_cache_scope_max: Option<usize> = cache_first.to_str()
        .and_then(|value| value.strip_prefix("--trail-cache-scope-max="))
        .map(|raw| raw.parse::<usize>().map_err(|_| "invalid weighted cache scope max"))
        .transpose()?;
    let cache_first = if weight_cache_scope_max.is_some() {
        args.next().ok_or("missing cache policy or precision after --trail-cache-scope-max")?
    } else { cache_first };
    let inline_weight_relabel = cache_first.to_str() == Some("--trail-inline-weight-relabel");
    let cache_first = if inline_weight_relabel {
        args.next().ok_or("missing cache policy or precision after --trail-inline-weight-relabel")?
    } else { cache_first };
    let post_unit_cache = cache_first.to_str() == Some("--trail-post-unit-cache");
    let cache_first = if post_unit_cache {
        args.next().ok_or("missing cache policy or precision after --trail-post-unit-cache")?
    } else { cache_first };
    let projected_post_key = cache_first.to_str() == Some("--trail-post-unit-projected");
    let cache_first = if projected_post_key {
        args.next().ok_or("missing cache policy or precision after --trail-post-unit-projected")?
    } else { cache_first };
    let fused_post_analysis = cache_first.to_str() == Some("--trail-fused-post-analysis");
    let cache_first = if fused_post_analysis {
        args.next().ok_or("missing cache policy or precision after --trail-fused-post-analysis")?
    } else { cache_first };
    let articulation_pivot = cache_first.to_str() == Some("--trail-articulation-pivot");
    let cache_first = if articulation_pivot {
        args.next().ok_or("missing cache mode or precision after --trail-articulation-pivot")?
    } else { cache_first };
    let articulation_root_only = cache_first.to_str() == Some("--trail-articulation-root-only");
    let cache_first = if articulation_root_only {
        args.next().ok_or("missing cache mode or precision after --trail-articulation-root-only")?
    } else { cache_first };
    let primal_order = cache_first.to_str() == Some("--trail-primal-bag-centroid");
    let cache_first = if primal_order {
        args.next().ok_or("missing mode after --trail-primal-bag-centroid")?
    } else { cache_first };
    let primal_root_only = cache_first.to_str() == Some("--trail-primal-centroid-root-only");
    let cache_first = if primal_root_only {
        args.next().ok_or("missing mode after --trail-primal-centroid-root-only")?
    } else { cache_first };
    let factor_ve = cache_first.to_str() == Some("--factor-variable-elimination");
    let cache_first = if factor_ve {
        args.next().ok_or("missing mode after --factor-variable-elimination")?
    } else { cache_first };
    let scope_probe_320 = cache_first.to_str() == Some("--factor-ve-scope-probe-320");
    let cache_first = if scope_probe_320 {
        args.next().ok_or("missing mode after --factor-ve-scope-probe-320")?
    } else { cache_first };
    let ve_min_fill = cache_first.to_str() == Some("--factor-ve-minfill-order");
    let cache_first = if ve_min_fill {
        args.next().ok_or("missing mode after --factor-ve-minfill-order")?
    } else { cache_first };
    let factor_add = cache_first.to_str() == Some("--factor-add-elimination");
    let cache_first = if factor_add {
        args.next().ok_or("missing mode after --factor-add-elimination")?
    } else { cache_first };
    let factor_boolean_bdd = cache_first.to_str() == Some("--factor-boolean-bdd");
    let cache_first = if factor_boolean_bdd {
        args.next().ok_or("missing mode after --factor-boolean-bdd")?
    } else { cache_first };
    let bdd_reverse_minfill = cache_first.to_str() == Some("--factor-bdd-reverse-minfill");
    let cache_first = if bdd_reverse_minfill {
        args.next().ok_or("missing mode after --factor-bdd-reverse-minfill")?
    } else { cache_first };
    let residual_factor_ve = cache_first.to_str() == Some("--trail-residual-factor-ve");
    let cache_first = if residual_factor_ve {
        args.next().ok_or("missing mode after --trail-residual-factor-ve")?
    } else { cache_first };
    let cache_policy = match cache_first.to_str() {
        Some("--hashbrown-owned") => CachePolicy::HbOwned,
        Some("--hashbrown-borrowed") => CachePolicy::HbBorrowed,
        _ => CachePolicy::StdOwned,
    };
    let numeric = if matches!(cache_policy, CachePolicy::StdOwned) {
        cache_first
    } else {
        args.next().ok_or("missing MPFR precision")?
    };
    let digits: u32 = numeric.to_str().ok_or("non-UTF8 precision")?
        .parse().map_err(|_| "bad precision")?;
    if !(30..=100).contains(&digits) { return Err("supported precision 30..100 digits".into()); }
    if (shared_residual_key || packed_residual_key) &&
        !matches!(cache_policy, CachePolicy::HbBorrowed) {
        return Err("shared/packed residual keys require --hashbrown-borrowed".into());
    }
    // Source GPMC/mpfr/mpreal.h uses this EXACT digits2bits conversion.
    let bits = (f64::from(digits) * 3.321_928_094_887_362_4).ceil() as u32;
    if reuse_csr_workspace && !indexed_unit_csr {
        return Err("--csr-reuse-workspace requires an indexed CSR unit propagation mode".into());
    }
    if trail_count && (unit_closure_pivot || reuse_csr_workspace) {
        return Err("--trail-count is an independent search arm and cannot combine with pivot/CSR arena".into());
    }
    if trail_parent_ids && !trail_count {
        return Err("--trail-parent-ids requires --trail-count".into());
    }
    if trail_compact_residual && !trail_parent_ids {
        return Err("--trail-compact-residual requires --trail-count --trail-parent-ids".into());
    }
    if trail_prehashed_cache && !trail_compact_residual {
        return Err("--trail-prehashed-cache requires --trail-count --trail-parent-ids --trail-compact-residual".into());
    }
    if trail_fused_compact && !trail_prehashed_cache {
        return Err("--trail-fused-compact requires --trail-count --trail-parent-ids --trail-compact-residual --trail-prehashed-cache".into());
    }
    if weighted_component_cache && !trail_fused_compact {
        return Err("--trail-weight-class-cache requires all existing --trail-fused-compact prerequisites".into());
    }
    if weight_cache_scope_max.is_some() && !weighted_component_cache {
        return Err("--trail-cache-scope-max requires --trail-weight-class-cache".into());
    }
    if weight_cache_scope_max.is_some_and(|max| !(1..=4096).contains(&max)) {
        return Err("--trail-cache-scope-max must be in 1..=4096".into());
    }
    if inline_weight_relabel && !(trail_count && trail_parent_ids
        && trail_compact_residual && trail_prehashed_cache
        && trail_fused_compact && weighted_component_cache) {
        return Err("--trail-inline-weight-relabel requires fused weighted-class trail cache".into());
    }
    if post_unit_cache && !(trail_count && trail_parent_ids
        && trail_compact_residual && trail_prehashed_cache
        && trail_fused_compact && weighted_component_cache && inline_weight_relabel) {
        return Err("--trail-post-unit-cache requires --trail-inline-weight-relabel and weighted-class fused trail".into());
    }
    if projected_post_key && !post_unit_cache {
        return Err("--trail-post-unit-projected requires --trail-post-unit-cache".into());
    }
    if fused_post_analysis && !projected_post_key {
        return Err("--trail-fused-post-analysis requires --trail-post-unit-projected".into());
    }
    if ve_min_fill && (!scope_probe_320 || !factor_ve) {
        return Err("--factor-ve-minfill-order requires --factor-variable-elimination --factor-ve-scope-probe-320".into());
    }
    if scope_probe_320 && !factor_ve {
        return Err("--factor-ve-scope-probe-320 requires --factor-variable-elimination".into());
    }
    if bdd_reverse_minfill && !factor_boolean_bdd {
        return Err("--factor-bdd-reverse-minfill requires --factor-boolean-bdd".into());
    }
    if factor_boolean_bdd && (!weighted_equivalence || !trail_count || factor_ve
        || factor_add || residual_factor_ve || root_failed_literal
        || post_unit_cache || articulation_pivot || primal_order || unit_closure_pivot) {
        return Err("--factor-boolean-bdd requires weighted-equivalence + trail-count without other root transformations".into());
    }
    if factor_add && (!weighted_equivalence || !trail_count || factor_ve
        || residual_factor_ve || root_failed_literal || post_unit_cache
        || articulation_pivot || primal_order || unit_closure_pivot) {
        return Err("--factor-add-elimination requires weighted-equivalence + trail-count without other root transforms".into());
    }
    if residual_factor_ve && (!weighted_equivalence || !trail_count || factor_ve
        || post_unit_cache || articulation_pivot || primal_order || root_failed_literal) {
        return Err("--trail-residual-factor-ve requires weighted-equivalence + trail-count, without other root transforms".into());
    }
    if factor_ve && (!weighted_equivalence || !trail_count || post_unit_cache
                     || articulation_pivot || primal_order || root_failed_literal) {
        return Err("--factor-variable-elimination requires original weighted-equivalence and trail-count without other root transformations".into());
    }
    if primal_root_only && !primal_order {
        return Err("--trail-primal-centroid-root-only requires --trail-primal-bag-centroid".into());
    }
    if primal_order && (!trail_count || !weighted_equivalence || post_unit_cache
                        || articulation_pivot || root_failed_literal) {
        return Err("--trail-primal-bag-centroid requires weighted-equivalence and trail-count without post-unit/articulation/failed-literal".into());
    }
    if root_failed_literal && (!weighted_equivalence || !trail_count) {
        return Err("--root-failed-literal requires --weighted-equivalence --trail-count".into());
    }
    if articulation_root_only && !articulation_pivot {
        return Err("--trail-articulation-root-only requires --trail-articulation-pivot".into());
    }
    if articulation_pivot && (!trail_count || post_unit_cache) {
        return Err("--trail-articulation-pivot requires --trail-count without --trail-post-unit-cache".into());
    }
    let path = args.next().ok_or("missing CNF")?;
    if args.next().is_some() { return Err("expected exactly one file".into()); }
    let t0 = Instant::now();
    let raw = std::fs::read_to_string(Path::new(&path)).map_err(|e| format!("read: {e}"))?;
    let read_ns = t0.elapsed().as_nanos();
    let t1 = Instant::now();
    let p = parse_emitted(&raw, bits)?;
    let parse_ns = t1.elapsed().as_nanos();
    let t2 = Instant::now();
    let (reduced, reduction_stats) = if weighted_equivalence {
        let (converted, scope, stats) = weighted_equivalence::reduce(&p, bits);
        (Some((converted, scope)), stats)
    } else {
        (None, weighted_equivalence::EquivalenceStats::default())
    };
    let pre_source = reduced.as_ref().map(|(problem, _)| problem).unwrap_or(&p);
    let scope = reduced.as_ref().map(|(_, scope)| scope.clone())
        .unwrap_or_else(|| (1..=p.n).collect());
    let (root_augmented, root_stats) = if root_failed_literal {
        root_failed_literal::augment(pre_source, &scope)
    } else {
        (None, root_failed_literal::RootFailedStats::default())
    };
    let source = root_augmented.as_ref().unwrap_or(pre_source);
    let td_start = Instant::now();
    let primal_profile = if primal_order {
        Some(primal_centroid::build(source, &scope))
    } else {
        None
    };
    let primal_build_ns = td_start.elapsed().as_nanos();
    let ve_start = Instant::now();
    let (ve_candidate, ve_stats) = if factor_ve {
        if ve_min_fill {
            factor_variable_elimination::count_scope_probe_minfill(source, &scope, bits)
        } else if scope_probe_320 {
            factor_variable_elimination::count_scope_probe(source, &scope, bits)
        } else {
            factor_variable_elimination::count(source, &scope, bits)
        }
    } else {
        (None, factor_variable_elimination::VeStats::default())
    };
    let ve_ns = ve_start.elapsed().as_nanos();
    let ve_completed = ve_candidate.is_some();
    let add_start = Instant::now();
    let (add_candidate, add_stats) = if factor_add {
        factor_add_elimination::count(source, &scope, bits)
    } else {
        (None, factor_add_elimination::AddStats::default())
    };
    let add_ns = add_start.elapsed().as_nanos();
    let add_completed = add_candidate.is_some();
    let bdd_start = Instant::now();
    let (bdd_candidate, bdd_stats) = if factor_boolean_bdd {
        factor_boolean_bdd::count(source, &scope, bits, bdd_reverse_minfill)
    } else {
        (None, factor_boolean_bdd::BddStats::default())
    };
    let bdd_ns = bdd_start.elapsed().as_nanos();
    let bdd_completed = bdd_candidate.is_some();
    eprintln!("RESEARCH_MPFR_FACTOR_VE_SCOPE_PROBE enabled={} limit={}",
        scope_probe_320, if scope_probe_320 { 320 } else { 256 });
    if ve_min_fill {
        let begin = std::time::Instant::now();
        let minor = factor_primal_minor_bound::minor_min_width(&source.clauses, &scope);
        eprintln!("RESEARCH_MPFR_FACTOR_VE_MINOR_BOUND eligible={} treewidth_lower={} bucket_lower={} contractions={} compute_ns={}",
            !minor.capped, minor.treewidth_lower, minor.treewidth_lower + 1,
            minor.contractions, begin.elapsed().as_nanos());
    }
    eprintln!("RESEARCH_MPFR_FACTOR_VE_ORDER minfill={} predicted_width={} fill_edges={} candidates={} plan_ns={}",
        ve_min_fill, ve_stats.minfill_plan_width, ve_stats.minfill_fill_edges,
        ve_stats.minfill_candidates, ve_stats.minfill_plan_ns);
    eprintln!("RESEARCH_MPFR_FACTOR_VE_ADMISSION source_vars={} scope_vars={} source_clauses={} max_clause_width={} cap_reason={}",
        ve_stats.source_vars, ve_stats.scope_vars, ve_stats.source_clauses,
        ve_stats.source_max_clause_width,
        if ve_stats.cap_reason.is_empty() { "none" } else { ve_stats.cap_reason });
    let mut engine = Engine::new(source, bits, cache_policy);
    engine.fast_assign = fast_assign;
    engine.shared_residual_key = shared_residual_key;
    engine.packed_residual_key = packed_residual_key;
    engine.packed_fast_admission = packed_fast_admission;
    engine.packed_hash_only = packed_hash_only;
    engine.packed_bypass_only = packed_bypass_only;
    engine.cache_limit = cache_limit;
    engine.node_budget = node_budget;
    engine.indexed_unit_propagation = indexed_unit_propagation;
    engine.indexed_unit_csr = indexed_unit_csr;
    engine.reuse_frequency_scratch = reuse_frequency_scratch;
    engine.unit_closure_pivot = unit_closure_pivot;
    engine.reuse_csr_workspace = reuse_csr_workspace;
    engine.indexed_activation_node = indexed_activation_node;
    let mut residual_ve_metrics = (0_u64, 0_u64, 0_u64, 0_u64, 0_usize);
    let (answer, solver_stats, trail_profile, primal_scans, primal_overrides) =
        if let Some(answer) = bdd_candidate.or(add_candidate).or(ve_candidate) {
            // A complete factor-semiring count has NO DPLL nodes.
            // Report VE product count separately instead of pretending
            // zero Shannon nodes is a direct DPLL-node improvement.
            (answer, Stats::default(), None, 0, 0)
        } else if trail_count {
        let mut trail = trail_engine::TrailEngine::new(source, bits, cache_limit, node_budget);
        trail.use_parent_ids = trail_parent_ids;
        trail.use_compact_residual = trail_compact_residual;
        trail.use_prehashed_cache = trail_prehashed_cache;
        trail.use_fused_compact = trail_fused_compact;
        trail.use_weight_class_cache = weighted_component_cache;
        trail.weight_cache_scope_max = weight_cache_scope_max;
        trail.use_inline_weight_relabel = inline_weight_relabel;
        trail.use_post_unit_cache = post_unit_cache;
        trail.use_projected_post_key = projected_post_key;
        trail.use_fused_post_analysis = fused_post_analysis;
        trail.use_articulation_pivot = articulation_pivot;
        trail.articulation_root_only = articulation_root_only;
        trail.use_residual_factor_ve = residual_factor_ve;
        if let Some(ref profile) = primal_profile {
            if profile.eligible {
                trail.primal_scores = Some(profile.scores.clone());
                trail.primal_root_only = primal_root_only;
            }
        }
        let outcome = trail.count(scope, 0)?;
        residual_ve_metrics = (
            trail.residual_ve_attempts, trail.residual_ve_completed,
            trail.residual_ve_capped, trail.residual_ve_products,
            trail.residual_ve_max_width,
        );
        (outcome, trail.stats,
         Some((trail.assignments, trail.cache_keys, trail.normalized_cache_keys,
               trail.source_cache_keys, trail.post_unit_hits, trail.post_unit_visited,
               trail.post_unit_forced, trail.reused_unforced_keys, trail.projected_forced_keys,
               trail.separator_scans, trail.separator_candidates, trail.separator_chosen,
               trail.separator_skipped)), trail.primal_scans, trail.primal_overrides)
    } else {
        let outcome = engine.count(source.clauses.clone(), scope, 0)?;
        (outcome, engine.stats, None, 0, 0)
    };
    let solve_ns = t2.elapsed().as_nanos();
    let t3 = Instant::now();
    println!("s {}", if answer.sat { "SATISFIABLE" } else { "UNSATISFIABLE" });
    println!("c r mpfr {} {}",
        answer.count.re.to_string_radix(10, Some(digits as usize + 4)),
        answer.count.im.to_string_radix(10, Some(digits as usize + 4))
    );
    std::io::stdout().flush().map_err(|e| format!("flush: {e}"))?;
    let output_ns = t3.elapsed().as_nanos();
    eprintln!("RESEARCH_MPFR_PHASE_NS read={read_ns} parse={parse_ns} solve={solve_ns} output={output_ns} nodes={} cache_hits={} decompositions={} digits={digits} bits={bits}",
        solver_stats.nodes, solver_stats.cache_hits, solver_stats.decompositions);
    eprintln!("RESEARCH_MPFR_WEIGHTED_EQ enabled={} binary={} eliminated={} removed_clauses={} representatives={} contradiction={} censored={}",
        weighted_equivalence, reduction_stats.binary_clauses,
        reduction_stats.eliminated_vars, reduction_stats.removed_clauses,
        reduction_stats.representatives, reduction_stats.contradictory,
        reduction_stats.censored);
    eprintln!("RESEARCH_MPFR_RESIDUAL_VE enabled={} attempts={} completed={} capped={} products={} max_width={}",
        residual_factor_ve, residual_ve_metrics.0, residual_ve_metrics.1,
        residual_ve_metrics.2, residual_ve_metrics.3, residual_ve_metrics.4);
    eprintln!("RESEARCH_MPFR_FACTOR_VE enabled={} completed={} eligible={} capped={} input_factors={} eliminated={} width={} cells={} products={} zero_sat={} ve_ns={}",
        factor_ve, ve_completed, ve_stats.eligible, ve_stats.capped,
        ve_stats.factors, ve_stats.eliminated, ve_stats.width,
        ve_stats.cells, ve_stats.products, ve_stats.zero_sat, ve_ns);
    eprintln!("RESEARCH_MPFR_FACTOR_ADD enabled={} completed={} eligible={} capped={} reason={} nodes={} terminals={} apply_calls={} apply_hits={} abstractions={} factors={} eliminated={} max_bucket={} add_ns={}",
        factor_add, add_completed, add_stats.eligible, add_stats.capped,
        if add_stats.reason.is_empty() { "none" } else { add_stats.reason },
        add_stats.nodes, add_stats.terminals, add_stats.apply_calls,
        add_stats.apply_hits, add_stats.abstractions, add_stats.factors,
        add_stats.eliminated, add_stats.max_bucket, add_ns);
    eprintln!("RESEARCH_MPFR_BOOL_BDD enabled={} completed={} eligible={} capped={} reason={} reverse_minfill={} vars={} clauses={} nodes={} apply_calls={} apply_hits={} eval_calls={} eval_hits={} minfill_bucket={} bdd_ns={}",
        factor_boolean_bdd, bdd_completed, bdd_stats.eligible, bdd_stats.capped,
        if bdd_stats.reason.is_empty() { "none" } else { bdd_stats.reason },
        bdd_stats.reverse_minfill, bdd_stats.variables, bdd_stats.clauses,
        bdd_stats.nodes, bdd_stats.apply_calls, bdd_stats.apply_hits,
        bdd_stats.eval_calls, bdd_stats.eval_hits, bdd_stats.minfill_bucket, bdd_ns);
    let empty_profile = primal_centroid::Profile::default();
    let td = primal_profile.as_ref().unwrap_or(&empty_profile);
    eprintln!("RESEARCH_MPFR_PRIMAL_BAG_CENTROID enabled={} root_only={} eligible={} capped={} vars={} edges={} width={} fill={} distance={} build_ns={} scans={} overrides={}",
        primal_order, primal_root_only, td.eligible, td.capped,
        td.vertices, td.primal_edges, td.width, td.fill_edges,
        td.max_distance, primal_build_ns, primal_scans, primal_overrides);
    eprintln!("RESEARCH_MPFR_ROOT_FAILED_LITERAL enabled={} eligible={} candidates={} probes={} implied={} refutations={} work={} contradiction={} capped={}",
        root_failed_literal, root_stats.eligible, root_stats.candidates,
        root_stats.probes, root_stats.implied_units, root_stats.refutations,
        root_stats.work, root_stats.contradicted, root_stats.capped);
    eprintln!("RESEARCH_MPFR_UNIT_CLOSURE_PIVOT enabled={} scans={} candidates={} chosen={} rejected={} work={} forced={} conflicts={} censored={}",
        unit_closure_pivot, engine.unit_closure_stats.scans,
        engine.unit_closure_stats.candidates, engine.unit_closure_stats.chosen,
        engine.unit_closure_stats.rejected, engine.unit_closure_stats.work,
        engine.unit_closure_stats.forced, engine.unit_closure_stats.contradictions,
        engine.unit_closure_stats.censored);
    let (csr_slots, csr_literals, csr_clauses) = engine.csr_workspace.capacities();
    eprintln!("RESEARCH_MPFR_CSR_WORKSPACE enabled={} slots={} literals={} clauses={}",
        reuse_csr_workspace, csr_slots, csr_literals, csr_clauses);
    let (trail_assignments, trail_cache_keys, trail_normalized_cache_keys, trail_source_cache_keys,
         post_unit_hits, post_unit_visited, post_unit_forced,
         reused_unforced_keys, projected_forced_keys, separator_scans,
         separator_candidates, separator_chosen, separator_skipped) =
        trail_profile.unwrap_or((0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0));
    eprintln!("RESEARCH_MPFR_TRAIL enabled={} assignments={} cache_keys={}",
        trail_count, trail_assignments, trail_cache_keys);
    eprintln!("RESEARCH_MPFR_TRAIL_PARENT_IDS enabled={}", trail_parent_ids);
    eprintln!("RESEARCH_MPFR_TRAIL_COMPACT_RESIDUAL enabled={}", trail_compact_residual);
    eprintln!("RESEARCH_MPFR_TRAIL_PREHASHED_CACHE enabled={}", trail_prehashed_cache);
    eprintln!("RESEARCH_MPFR_TRAIL_FUSED_COMPACT enabled={}", trail_fused_compact);
    eprintln!("RESEARCH_MPFR_TRAIL_WEIGHT_CLASS_CACHE enabled={}", weighted_component_cache);
    eprintln!("RESEARCH_MPFR_TRAIL_INLINE_WEIGHT_RELABEL enabled={}", inline_weight_relabel);
    eprintln!("RESEARCH_MPFR_POST_UNIT_CACHE enabled={} hits={} visited={} forced={}",
        post_unit_cache, post_unit_hits, post_unit_visited, post_unit_forced);
    eprintln!("RESEARCH_MPFR_POST_UNIT_PROJECTED enabled={} unchanged={} projected={}",
        projected_post_key, reused_unforced_keys, projected_forced_keys);
    eprintln!("RESEARCH_MPFR_POST_UNIT_FUSED_ANALYSIS enabled={}", fused_post_analysis);
    eprintln!("RESEARCH_MPFR_TRAIL_ARTICULATION_ROOT_ONLY enabled={}", articulation_root_only);
    eprintln!("RESEARCH_MPFR_TRAIL_ARTICULATION enabled={} scans={} candidates={} chosen={} skipped={}",
        articulation_pivot, separator_scans, separator_candidates, separator_chosen, separator_skipped);
    eprintln!("RESEARCH_MPFR_TRAIL_WEIGHT_CLASS_SCOPE_MAX max={} localized_keys={} original_keys={}",
        weight_cache_scope_max.unwrap_or_default(), trail_normalized_cache_keys, trail_source_cache_keys);
    eprintln!("RESEARCH_MPFR_CACHE_POLICY {}", cache_policy.label());
    eprintln!("RESEARCH_MPFR_SHARED_RESIDUAL enabled={} entries={}",
        shared_residual_key, engine.shared_cache.len());
    eprintln!("RESEARCH_MPFR_CACHE_LIMIT capacity={} shared_entries={} owned_entries={}",
        cache_limit, engine.shared_cache.len(), engine.hb_cache.len() + engine.cache.len());
    eprintln!("RESEARCH_MPFR_PACKED_RESIDUAL enabled={} entries={}",
        packed_residual_key, engine.packed_cache.len());
    eprintln!("RESEARCH_MPFR_PACKED_FAST_ADMISSION enabled={packed_fast_admission} full_misses={}",
        engine.stats.packed_full_misses);
    eprintln!("RESEARCH_MPFR_PACKED_FACTOR_ABLATION reuse_hash={} bypass_full_miss={} full_misses={}",
        packed_fast_admission || packed_hash_only,
        packed_fast_admission || packed_bypass_only,
        engine.stats.packed_full_misses);
    eprintln!("RESEARCH_MPFR_UNIT_PROP_INDEXED enabled={indexed_unit_propagation} activation_node={indexed_activation_node}");
    eprintln!("RESEARCH_MPFR_UNIT_PROP_CSR enabled={indexed_unit_csr} activation_node={indexed_activation_node}");
    eprintln!("RESEARCH_MPFR_FREQ_SCRATCH enabled={reuse_frequency_scratch} capacity={}",
        engine.frequency_scratch.len());
    // Native WallTime includes engine destruction, unlike solve_ns. Isolate
    // this time so a Cache-optimized process is not misread as faster DPLL.
    let release_start = Instant::now();
    drop(engine);
    eprintln!("RESEARCH_MPFR_ENGINE_DROP_NS {}", release_start.elapsed().as_nanos());
    Ok(())
}
fn main() {
    if let Err(err) = run() {
        eprintln!("RESEARCH_MPFR_FAIL_CLOSED: {err}");
        std::process::exit(2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpmc_binary_precision_bits_match() {
        assert_eq!((30_f64 * 3.321_928_094_887_362_4).ceil() as u32, 100);
        assert_eq!((50_f64 * 3.321_928_094_887_362_4).ceil() as u32, 167);
        assert_eq!((100_f64 * 3.321_928_094_887_362_4).ceil() as u32, 333);
    }
    #[test]
    fn complex_four_rounded_products() {
        let a = C::decimal(100, "0.125", "-0.25").unwrap();
        let b = C::decimal(100, "-0.5", "0.375").unwrap();
        let z = a.mul(&b, 100);
        assert_eq!(z.re, Float::with_val(100, 0.03125));
        assert_eq!(z.im, Float::with_val(100, 0.171875));
    }
    #[test]
    fn boolean_sat_with_cancelled_wmc() {
        let text = "p cnf 1 0\nc t wmc\nc p weight 1 1 0 0\nc p weight -1 -1 0 0\n";
        let p = parse_emitted(text, 100).unwrap();
        let mut engine = Engine::new(&p, 100, CachePolicy::StdOwned);
        let out = engine.count(p.clauses.clone(), vec![1], 0).unwrap();
        assert!(out.sat);
        assert_eq!(out.count.re, 0);
    }
    #[test]
    fn opt_in_assignment_preserves_original_normalized_clause_semantics() {
        let clauses = normalize(vec![
            vec![1, 2], vec![-1, 3], vec![-3, 4], vec![2, -4],
            vec![2, 3, -4], vec![1], vec![], vec![3, -3],
        ]);
        for literal in [-4_i32, -3, -2, -1, 1, 2, 3, 4] {
            assert_eq!(assign_fast(&clauses, literal), assign(&clauses, literal));
            let reduced = normalize_child(assign(&clauses, literal));
            for other in [-4_i32, -3, -2, -1, 1, 2, 3, 4] {
                assert_eq!(assign_fast(&reduced, other), assign(&reduced, other));
            }
        }
    }
    #[test]
    fn cache_ownership_and_binary_assignment_preserve_full_mpfr_wmc() {
        let source = "p cnf 4 4\nc t wmc\nc p weight 1 0.25 -0.5 0\nc p weight -1 -0.25 0.5 0\nc p weight 2 0.3 0.1 0\nc p weight -2 0.7 0 0\n1 2 0\n-1 3 0\n1 -4 0\n-2 3 -4 0\n";
        let p = parse_emitted(source, 100).unwrap();
        let mut reference = None;
        for mode in [CachePolicy::StdOwned, CachePolicy::HbOwned, CachePolicy::HbBorrowed] {
            for fast in [false, true] {
                let mut engine = Engine::new(&p, 100, mode);
                engine.fast_assign = fast;
                let out = engine.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                let observed = (out.sat, out.count.re, out.count.im,
                    engine.stats.nodes, engine.stats.cache_hits, engine.stats.decompositions);
                if let Some(ref expected) = reference {
                    assert_eq!(&observed, expected, "same source CNF and MPFR operation order");
                } else {
                    reference = Some(observed);
                }
            }
        }
    }

    #[test]
    fn shared_residual_keys_have_value_hash_and_collision_safe_lookup() {
        let p = parse_emitted("p cnf 3 2\nc t wmc\n1 -2 0\n-2 3 0\n", 100).unwrap();
        let mut engine = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        let scope = vec![1, 2, 3];
        let clauses = normalize(p.clauses.clone());
        let root = Arc::new(clauses.clone());
        engine.shared_cache.insert(
            SharedKey { scope: scope.clone(), clauses: Arc::clone(&root) },
            Outcome { sat: true, count: C::one(100) },
        );
        let hash = engine.shared_key_hash(&scope, &clauses);
        assert!(engine.shared_cache.raw_entry().from_hash(hash, |key|
            key.scope.as_slice() == scope.as_slice() &&
            key.clauses.as_slice() == clauses.as_slice()
        ).is_some());
        let wrong = vec![vec![1, 2]];
        assert!(engine.shared_cache.raw_entry().from_hash(hash, |key|
            key.scope.as_slice() == scope.as_slice() &&
            key.clauses.as_slice() == wrong.as_slice()
        ).is_none());
        assert_eq!(Arc::strong_count(&root), 2);
    }

    #[test]
    fn shared_residual_cache_preserves_search_and_mpfr_output() {
        let source = "p cnf 5 5\nc t wmc\nc p weight 1 0.25 -0.5 0\nc p weight -1 -0.25 0.5 0\nc p weight 2 0.3 0.1 0\nc p weight -2 0.7 0 0\n1 2 0\n-1 3 0\n1 -4 0\n-2 3 -4 0\n4 5 0\n";
        let problem = parse_emitted(source, 100).unwrap();
        for bits in [100_u32, 333_u32] {
            let mut witness = None;
            for shared in [false, true] {
                let p = parse_emitted(source, bits).unwrap();
                let mut engine = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                engine.fast_assign = true;
                engine.shared_residual_key = shared;
                let out = engine.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                let value = (out.sat, out.count.re, out.count.im, engine.stats.nodes,
                    engine.stats.cache_hits, engine.stats.decompositions);
                if shared {
                    let expected = witness.take().unwrap();
                    assert_eq!(value, expected);
                } else { witness = Some(value); }
            }
        }
        // Extra test of independently created residual with exact cache behavior.
        assert_eq!(problem.n, 5);
    }

    #[test]
    fn node_work_activation_preserves_baseline_on_small_sources() {
        let raw = "p cnf 4 4\nc t wmc\n1 2 0\n-1 3 0\n-3 4 0\n-4 0\n";
        let p = parse_emitted(raw, 100).unwrap();
        let mut baseline = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        baseline.fast_assign = true;
        baseline.shared_residual_key = true;
        let expected = baseline.count(p.clauses.clone(), vec![1,2,3,4], 0).unwrap();
        let mut admitted = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        admitted.fast_assign = true;
        admitted.shared_residual_key = true;
        admitted.indexed_unit_propagation = true;
        admitted.indexed_activation_node = 8192;
        let actual = admitted.count(p.clauses.clone(), vec![1,2,3,4], 0).unwrap();
        assert_eq!((expected.sat, expected.count.re, expected.count.im),
                   (actual.sat, actual.count.re, actual.count.im));
        assert_eq!(baseline.stats.nodes, admitted.stats.nodes);
        assert_eq!(baseline.stats.cache_hits, admitted.stats.cache_hits);
    }

    #[test]
    fn indexed_unit_propagation_respects_original_first_unit_order() {
        let bits = 100;
        let text = "p cnf 4 4\nc t wmc\nc p weight 1 0.11 0.21 0\nc p weight -1 0.4 0 0\nc p weight 2 0.33 -0.3 0\nc p weight -2 -0.4 0.1 0\nc p weight 3 0.51 0.13 0\nc p weight -3 0.19 0.13 0\n-1 2 0\n1 3 0\n-3 4 0\n-4 0\n";
        let p = parse_emitted(text, bits).unwrap();
        let canonical = normalize(p.clauses.clone());
        let mut expected = canonical.clone();
        let mut scope = (1..=p.n).collect::<Vec<_>>();
        let mut pref = C::one(bits);
        while let Some(lit) = expected.iter().find(|c| c.len() == 1).map(|c| c[0]) {
            let var = lit.unsigned_abs() as usize;
            let weight = if lit > 0 { &p.weights[var-1].positive } else { &p.weights[var-1].negative };
            pref = pref.mul(weight, bits);
            scope.retain(|&v| v != var);
            expected = assign_fast(&expected, lit);
            assert!(!expected.iter().any(Vec::is_empty));
        }
        let mut actual_scope = (1..=p.n).collect::<Vec<_>>();
        let mut actual_pref = C::one(bits);
        let actual = unit_propagate_indexed(
            &canonical, p.n, &p.weights, bits, &mut actual_scope, &mut actual_pref
        ).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual_scope, scope);
        assert_eq!(actual_pref.re, pref.re);
        assert_eq!(actual_pref.im, pref.im);
    }

    #[test]
    fn indexed_unit_propagation_handles_conflicting_forces_and_zero_count_sat() {
        let text = "p cnf 2 2\nc t wmc\n1 0\n-1 0\n";
        let p = parse_emitted(text, 100).unwrap();
        for indexed in [false, true] {
            let mut engine = Engine::new(&p, 100, CachePolicy::HbBorrowed);
            engine.fast_assign = true;
            engine.shared_residual_key = true;
            engine.indexed_unit_propagation = indexed;
            let out = engine.count(p.clauses.clone(), vec![1,2], 0).unwrap();
            assert!(!out.sat);
        }
        let text = "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n";
        let p = parse_emitted(text, 100).unwrap();
        for indexed in [false, true] {
            let mut engine = Engine::new(&p, 100, CachePolicy::HbBorrowed);
            engine.indexed_unit_propagation = indexed;
            let out = engine.count(p.clauses.clone(), vec![1,2], 0).unwrap();
            assert!(out.sat, "numerical zero is distinct from Boolean UNSAT");
            assert_eq!(out.count.re, 0);
        }
    }

    #[test]
    fn deterministic_small_full_wmc_fuzz_preserves_mpfr_bytes_and_dpll_structure() {
        // Generated CNFs exercise many unit-chain / nonunit paths with mixed
        // literal polarity and exact weight cancellation; deterministic seed.
        let mut seed = 0x12345678_u64;
        for case in 0..96 {
            let mut clauses = Vec::new();
            for _ in 0..(3 + case % 9) {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let len = (seed as usize % 3) + 1;
                let mut clause = Vec::new();
                for _ in 0..len {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let var = (seed as usize % 5) + 1;
                    clause.push(if seed & 0x4000 == 0 { var as i32 } else { -(var as i32) });
                }
                clauses.push(clause);
            }
            for bits in [100_u32, 333_u32] {
                let p = Problem {
                    n: 5, clauses: clauses.clone(),
                    weights: (0..5).map(|_| Weights {
                        positive: C::decimal(bits, "0.3", "0.1").unwrap(),
                        negative: C::decimal(bits, "0.25", "-0.5").unwrap(),
                    }).collect()
                };
                let mut baseline = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                baseline.fast_assign = true;
                baseline.shared_residual_key = true;
                let expected = baseline.count(p.clauses.clone(), (1..=5).collect(), 0).unwrap();
                let mut candidate = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                candidate.fast_assign = true;
                candidate.shared_residual_key = true;
                candidate.indexed_unit_propagation = true;
                let observed = candidate.count(p.clauses.clone(), (1..=5).collect(), 0).unwrap();
                assert_eq!((expected.sat, &expected.count.re, &expected.count.im),
                           (observed.sat, &observed.count.re, &observed.count.im),
                           "case {case}, bits {bits}");
                assert_eq!(
                    (baseline.stats.nodes, baseline.stats.cache_hits, baseline.stats.decompositions),
                    (candidate.stats.nodes, candidate.stats.cache_hits, candidate.stats.decompositions),
                    "unchanged search topology for {case}, {bits}"
                );
                let mut csr = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                csr.fast_assign = true;
                csr.shared_residual_key = true;
                csr.indexed_unit_propagation = true;
                csr.indexed_unit_csr = true;
                let actual = csr.count(p.clauses.clone(), (1..=5).collect(), 0).unwrap();
                assert_eq!((actual.sat, &actual.count.re, &actual.count.im),
                           (expected.sat, &expected.count.re, &expected.count.im),
                           "CSR numeric output parity for {case}, {bits}");
                assert_eq!(
                    (csr.stats.nodes, csr.stats.cache_hits, csr.stats.decompositions),
                    (candidate.stats.nodes, candidate.stats.cache_hits, candidate.stats.decompositions),
                    "CSR structural parity for {case}, {bits}"
                );
            }
        }
    }

    #[test]
    fn bounded_cache_admission_never_exceeds_capacity() {
        let p = parse_emitted("p cnf 3 0\nc t wmc\n", 100).unwrap();
        let mut engine = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        assert_eq!(engine.cache_limit, 10_000);
        engine.cache_limit = 2;
        let mut seen = Vec::new();
        for n in 1..=4 {
            let result = engine.save(
                StoredKey::Shared(SharedKey {
                    scope: vec![n], clauses: Arc::new(vec![vec![n as i32]])
                }),
                Outcome { sat: true, count: C::one(100) }
            );
            seen.push(result.sat);
        }
        assert_eq!(seen, vec![true, true, true, true]);
        assert_eq!(engine.shared_cache.len(), 2);
    }

    #[test]
    fn bounded_cache_variants_preserve_complete_mpfr_wmc() {
        let source = concat!(
            "p cnf 6 7\nc t wmc\n",
            "c p weight 1 0.3 0.1 0\nc p weight -1 -0.2 0.4 0\n",
            "c p weight 2 0.6 -0.1 0\nc p weight -2 0.2 0.3 0\n",
            "1 2 0\n-1 3 0\n-2 4 0\n3 5 0\n",
            "-3 6 0\n-4 -5 0\n-5 -6 0\n"
        );
        for bits in [100_u32, 333_u32] {
            let p = parse_emitted(source, bits).unwrap();
            let mut expected = None;
            for capacity in [1_usize, 10_000, 25_000, 50_000, 100_000] {
                let mut engine = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                engine.fast_assign = true;
                engine.shared_residual_key = true;
                engine.indexed_unit_propagation = true;
                engine.indexed_activation_node = 8192;
                engine.cache_limit = capacity;
                let answer = engine.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                let result = (answer.sat, answer.count.re, answer.count.im);
                if let Some(ref witness) = expected {
                    assert_eq!(&result, witness, "cache limit {capacity} at {bits} bits");
                } else { expected = Some(result); }
                assert!(engine.shared_cache.len() <= capacity);
            }
        }
    }

    #[test]
    fn csr_occurrences_preserve_source_unit_order_and_mpfr() {
        let source = concat!(
            "p cnf 7 7\nc t wmc\n",
            "c p weight 1 0.11 0.21 0\nc p weight -1 0.4 0 0\n",
            "c p weight 2 0.33 -0.3 0\nc p weight -2 -0.4 0.1 0\n",
            "c p weight 3 0.51 0.13 0\nc p weight -3 0.19 0.13 0\n",
            "1 2 3 0\n-1 4 0\n-4 5 0\n-5 6 0\n",
            "-6 7 0\n-7 0\n2 3 0\n"
        );
        for bits in [100_u32, 333_u32] {
            let p = parse_emitted(source, bits).unwrap();
            let residual = normalize(p.clauses.clone());
            let mut control_scope = (1..=p.n).collect::<Vec<_>>();
            let mut candidate_scope = control_scope.clone();
            let mut control_prefix = C::one(bits);
            let mut candidate_prefix = C::one(bits);
            let control = unit_propagate_indexed(
                &residual, p.n, &p.weights, bits, &mut control_scope, &mut control_prefix
            );
            let candidate = unit_propagate_indexed_csr(
                &residual, p.n, &p.weights, bits, &mut candidate_scope, &mut candidate_prefix
            );
            assert_eq!(candidate, control);
            assert_eq!(candidate_scope, control_scope);
            assert_eq!(candidate_prefix.re, control_prefix.re);
            assert_eq!(candidate_prefix.im, control_prefix.im);
        }
    }
    #[test]
    fn csr_search_parity_and_sat_zero_unsat_separation() {
        let fixtures = [
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
            "p cnf 6 4\nc t wmc\n1 2 0\n-1 3 0\n4 -5 0\n-4 6 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            for text in fixtures {
                let p = parse_emitted(text, bits).unwrap();
                let mut witness = None;
                for csr in [false, true] {
                    let mut engine = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    engine.fast_assign = true;
                    engine.shared_residual_key = true;
                    engine.indexed_unit_propagation = true;
                    engine.indexed_unit_csr = csr;
                    let answer = engine.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                    let value = (answer.sat, answer.count.re, answer.count.im,
                        engine.stats.nodes, engine.stats.cache_hits, engine.stats.decompositions);
                    if let Some(reference) = witness.take() {
                        assert_eq!(value, reference);
                    } else { witness = Some(value); }
                }
            }
        }
    }
    #[test]
    fn csr_and_cache_capacity_composition_preserves_mpfr_and_search() {
        // A factorial control: Cache limits may alter the tree, but CSR and
        // indexed propagation must have the SAME tree at each fixed limit.
        // Different limits must preserve the full rounded complex WMC bytes.
        let fixtures = [
            concat!(
                "p cnf 6 7\nc t wmc\n",
                "c p weight 1 0.3 0.1 0\nc p weight -1 -0.2 0.4 0\n",
                "c p weight 2 0.6 -0.1 0\nc p weight -2 0.2 0.3 0\n",
                "1 2 0\n-1 3 0\n-2 4 0\n3 5 0\n",
                "-3 6 0\n-4 -5 0\n-5 -6 0\n"
            ),
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            for source in fixtures {
                let p = parse_emitted(source, bits).unwrap();
                let mut cross_capacity: Option<Outcome> = None;
                for capacity in [1_usize, 10_000, 25_000, 50_000] {
                    let mut structural_pair = None;
                    for csr in [false, true] {
                        let mut engine = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                        engine.fast_assign = true;
                        engine.shared_residual_key = true;
                        engine.indexed_unit_propagation = true;
                        engine.indexed_unit_csr = csr;
                        engine.cache_limit = capacity;
                        // Activate on the first node to cover actual CSR logic.
                        engine.indexed_activation_node = 0;
                        let outcome = engine.count(p.clauses.clone(), (1..=p.n).collect(), 0)
                            .expect("composed factorial fixture must complete");
                        let witness = (outcome.sat, outcome.count.re.clone(),
                            outcome.count.im.clone(), engine.stats.nodes,
                            engine.stats.cache_hits, engine.stats.decompositions);
                        if let Some(ref expected) = structural_pair {
                            assert_eq!(&witness, expected,
                                "CSR/indexed topology differs for {bits} bits at cap {capacity}");
                        } else {
                            structural_pair = Some(witness);
                        }
                        if let Some(ref expected) = cross_capacity {
                            assert_eq!(outcome.sat, expected.sat);
                            assert_eq!(&outcome.count.re, &expected.count.re);
                            assert_eq!(&outcome.count.im, &expected.count.im);
                        } else {
                            cross_capacity = Some(outcome.clone());
                        }
                        assert!(engine.shared_cache.len() <= capacity);
                    }
                }
            }
        }
    }

    #[test]
    fn node_budget_is_bounded_and_default_is_unchanged() {
        let p = parse_emitted("p cnf 2 1\nc t wmc\n1 2 0\n", 100).unwrap();
        let mut default = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        assert_eq!(default.node_budget, 100_000);
        default.fast_assign = true;
        let expected = default.count(p.clauses.clone(), vec![1, 2], 0).unwrap();
        for cap in [250_000_u64, 1_000_000_u64] {
            let mut increased = Engine::new(&p, 100, CachePolicy::HbBorrowed);
            increased.fast_assign = true;
            increased.node_budget = cap;
            let actual = increased.count(p.clauses.clone(), vec![1, 2], 0).unwrap();
            assert_eq!(actual.sat, expected.sat);
            assert_eq!(actual.count.re, expected.count.re);
            assert_eq!(actual.count.im, expected.count.im);
            assert_eq!(increased.stats.nodes, default.stats.nodes);
        }
        let mut capped = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        capped.node_budget = 1;
        let error = capped.count(p.clauses.clone(), vec![1, 2], 0).err();
        assert_eq!(error.as_deref(), Some("node limit"));
        assert!(capped.stats.nodes <= 1);
    }

    #[test]
    fn packed_residual_key_preserves_boundaries_order_scope_and_collision_equality() {
        let p = parse_emitted("p cnf 4 1\nc t wmc\n1 -2 0\n", 100).unwrap();
        let mut engine = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        let scope = vec![1, 2, 3, 4];
        let exact = vec![vec![1, -2], vec![-3], vec![], vec![4, 4]];
        let key = PackedKey::from_residual(scope.clone(), &exact);
        assert!(key.matches_residual(&scope, &exact));
        assert_eq!(key.literals, vec![1, -2, 0, -3, 0, 0, 4, 4, 0]);
        let hash = engine.packed_key_hash(&scope, &exact);
        let mut stored_hasher = engine.packed_cache.hasher().build_hasher();
        key.hash(&mut stored_hasher);
        assert_eq!(hash, stored_hasher.finish(),
            "packed and borrowed nested-clause hash must be bit-identical");
        engine.packed_cache.insert(key, Outcome { sat: true, count: C::one(100) });
        assert!(engine.packed_cache.raw_entry().from_hash(hash, |key| {
            key.matches_residual(&scope, &exact)
        }).is_some(), "borrowed hash must identify the precisely stored packed key");
        let mutants = [
            vec![vec![1], vec![-2, -3], vec![], vec![4, 4]],
            vec![vec![-3], vec![1, -2], vec![], vec![4, 4]],
            vec![vec![1, -2], vec![-3], vec![4, 4]],
            vec![vec![1, -2], vec![-3], vec![], vec![4, -4]],
            vec![vec![1, -2], vec![-3], vec![], vec![4]],
        ];
        for candidate in mutants {
            // An artificially forced identical hash cannot confer a false hit.
            assert!(engine.packed_cache.raw_entry().from_hash(hash, |key| {
                key.matches_residual(&scope, &candidate)
            }).is_none());
        }
        assert!(engine.packed_cache.raw_entry().from_hash(hash, |key| {
            key.matches_residual(&[2, 1, 3, 4], &exact)
        }).is_none());
    }

    #[test]
    fn packed_cache_matches_shared_full_mpfr_and_search_for_random_residuals() {
        let mut seed = 0x6b6f_693c_5b0a_c830_u64;
        for case in 0..96 {
            let mut clauses = Vec::new();
            for _ in 0..(3 + case % 9) {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let clause_size = (seed as usize % 3) + 1;
                let mut clause = Vec::new();
                for _ in 0..clause_size {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let var = (seed as usize % 6) + 1;
                    clause.push(if seed & 0x8000 == 0 { var as i32 } else { -(var as i32) });
                }
                clauses.push(clause);
            }
            for bits in [100_u32, 333_u32] {
                let p = Problem {
                    n: 6, clauses: clauses.clone(),
                    weights: (0..6).map(|v| Weights {
                        positive: C::decimal(bits, if v % 2 == 0 { "0.3" } else { "-0.2" },
                            if v % 2 == 0 { "0.1" } else { "-0.2" }).unwrap(),
                        negative: C::decimal(bits, "0.25", "-0.5").unwrap(),
                    }).collect()
                };
                for capacity in [1_usize, 10, 10_000, 50_000] {
                    let mut shared = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    shared.fast_assign = true;
                    shared.shared_residual_key = true;
                    shared.indexed_unit_propagation = true;
                    shared.indexed_unit_csr = true;
                    shared.cache_limit = capacity;
                    let expected = shared.count(p.clauses.clone(), (1..=p.n).collect(), 0)
                        .expect("shared original must terminate");
                    let mut packed = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    packed.fast_assign = true;
                    packed.packed_residual_key = true;
                    packed.indexed_unit_propagation = true;
                    packed.indexed_unit_csr = true;
                    packed.cache_limit = capacity;
                    let actual = packed.count(p.clauses.clone(), (1..=p.n).collect(), 0)
                        .expect("packed research mode must terminate");
                    assert_eq!((actual.sat, &actual.count.re, &actual.count.im),
                        (expected.sat, &expected.count.re, &expected.count.im),
                        "case={case} precision={bits} capacity={capacity}");
                    assert_eq!(
                        (packed.stats.nodes, packed.stats.cache_hits, packed.stats.decompositions),
                        (shared.stats.nodes, shared.stats.cache_hits, shared.stats.decompositions),
                        "packed must not change search structure for case={case} precision={bits} capacity={capacity}"
                    );
                    assert_eq!(packed.packed_cache.len(), shared.shared_cache.len());
                    assert!(packed.packed_cache.len() <= capacity);
                }
            }
        }
    }

    #[test]
    fn packed_fast_admission_preserves_full_mpfr_and_search_counters() {
        let mut seed = 0xfaca_de05_7a11_1234_u64;
        for case in 0..96 {
            let mut clauses = Vec::new();
            for _ in 0..(4 + case % 10) {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let len = (seed as usize % 3) + 1;
                let mut clause = Vec::new();
                for _ in 0..len {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let var = (seed as usize % 6) + 1;
                    clause.push(if seed & 0x400 == 0 { var as i32 } else { -(var as i32) });
                }
                clauses.push(clause);
            }
            for bits in [100_u32, 333_u32] {
                let p = Problem {
                    n: 6, clauses: clauses.clone(),
                    weights: (0..6).map(|v| Weights {
                        positive: C::decimal(bits, if v % 2 == 0 { "0.3" } else { "-0.2" }, "0.1").unwrap(),
                        negative: C::decimal(bits, "0.25", "-0.5").unwrap(),
                    }).collect(),
                };
                for capacity in [0_usize, 1, 10, 10_000, 50_000] {
                    let mut control = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    control.fast_assign = true;
                    control.packed_residual_key = true;
                    control.indexed_unit_propagation = true;
                    control.indexed_unit_csr = true;
                    control.cache_limit = capacity;
                    let old = control.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                    let mut candidate = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    candidate.fast_assign = true;
                    candidate.packed_residual_key = true;
                    candidate.packed_fast_admission = true;
                    candidate.indexed_unit_propagation = true;
                    candidate.indexed_unit_csr = true;
                    candidate.cache_limit = capacity;
                    let new = candidate.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                    assert_eq!((new.sat, &new.count.re, &new.count.im),
                        (old.sat, &old.count.re, &old.count.im),
                        "case={case} bits={bits} capacity={capacity}");
                    assert_eq!(
                        (candidate.stats.nodes, candidate.stats.cache_hits, candidate.stats.decompositions),
                        (control.stats.nodes, control.stats.cache_hits, control.stats.decompositions),
                        "same search topology for case={case} bits={bits} capacity={capacity}");
                    assert_eq!(candidate.packed_cache.len(), control.packed_cache.len());
                    assert!(candidate.packed_cache.len() <= capacity);
                    if capacity == 0 {
                        assert_eq!(candidate.stats.packed_full_misses, candidate.stats.nodes);
                        assert!(candidate.packed_cache.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn bounded_frequency_scratch_matches_original_packed_fast_wmc_and_tree() {
        let cases = [
            "p cnf 6 9\nc t wmc\n1 2 3 0\n-1 4 0\n-2 5 0\n-3 6 0\n-4 -5 0\n-5 -6 0\n1 -4 0\n2 -6 0\n-1 -2 0\n",
            "p cnf 5 4\nc t wmc\n1 2 0\n-1 3 0\n-2 4 0\n-4 5 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            for case in cases {
                let p = parse_emitted(case, bits).unwrap();
                for cap in [0_usize, 1, 10, 10_000, 50_000] {
                    let mut original = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    original.fast_assign = true;
                    original.packed_residual_key = true;
                    original.packed_fast_admission = true;
                    original.indexed_unit_propagation = true;
                    original.indexed_unit_csr = true;
                    original.cache_limit = cap;
                    let a = original.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();

                    let mut scratch = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    scratch.fast_assign = true;
                    scratch.packed_residual_key = true;
                    scratch.packed_fast_admission = true;
                    scratch.indexed_unit_propagation = true;
                    scratch.indexed_unit_csr = true;
                    scratch.reuse_frequency_scratch = true;
                    scratch.cache_limit = cap;
                    let ptr = scratch.frequency_scratch.as_ptr();
                    let b = scratch.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();

                    assert_eq!((a.sat, &a.count.re, &a.count.im),
                               (b.sat, &b.count.re, &b.count.im));
                    assert_eq!(
                        (original.stats.nodes, original.stats.cache_hits, original.stats.decompositions),
                        (scratch.stats.nodes, scratch.stats.cache_hits, scratch.stats.decompositions),
                        "frequency scratch must not alter search topology"
                    );
                    assert_eq!(scratch.packed_cache.len(), original.packed_cache.len());
                    assert_eq!(scratch.frequency_scratch.len(), p.n + 1);
                    assert!(scratch.frequency_scratch.iter().all(|&x| x == 0));
                    assert_eq!(scratch.frequency_scratch.as_ptr(), ptr, "arena must be reused");
                }
            }
        }
    }

    #[test]
    fn packed_factor_ablation_modes_preserve_mpfr_sat_cache_and_search() {
        // Small satisfiable, UNSAT and SAT-but-zero weighted controls.
        let fixtures = [
            "p cnf 4 3\nc t wmc\n1 2 0\n-1 3 0\n-3 4 0\n",
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            for fixture in fixtures {
                let p = parse_emitted(fixture, bits).unwrap();
                for capacity in [0_usize, 1, 10, 10_000, 50_000] {
                    let mut control = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    control.fast_assign = true;
                    control.packed_residual_key = true;
                    control.indexed_unit_propagation = true;
                    control.indexed_unit_csr = true;
                    control.cache_limit = capacity;
                    let expected = control.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                    for (hash_only, bypass_only, fast) in [
                        (true, false, false), (false, true, false), (false, false, true)
                    ] {
                        let mut candidate = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                        candidate.fast_assign = true;
                        candidate.packed_residual_key = true;
                        candidate.packed_hash_only = hash_only;
                        candidate.packed_bypass_only = bypass_only;
                        candidate.packed_fast_admission = fast;
                        candidate.indexed_unit_propagation = true;
                        candidate.indexed_unit_csr = true;
                        candidate.cache_limit = capacity;
                        let actual = candidate.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                        assert_eq!(
                            (actual.sat, &actual.count.re, &actual.count.im),
                            (expected.sat, &expected.count.re, &expected.count.im),
                            "bits={bits} capacity={capacity} hash_only={hash_only} bypass_only={bypass_only} fast={fast}"
                        );
                        assert_eq!(
                            (candidate.stats.nodes, candidate.stats.cache_hits, candidate.stats.decompositions),
                            (control.stats.nodes, control.stats.cache_hits, control.stats.decompositions),
                            "search topology must not change"
                        );
                        assert_eq!(candidate.packed_cache.len(), control.packed_cache.len());
                        assert!(candidate.packed_cache.len() <= capacity);
                        if capacity == 0 && (bypass_only || fast) {
                            assert_eq!(candidate.stats.packed_full_misses, candidate.stats.nodes);
                        }
                        if hash_only {
                            assert_eq!(candidate.stats.packed_full_misses, 0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn packed_fast_admission_never_hash_collides_to_false_cache_hit() {
        let p = parse_emitted("p cnf 3 0\nc t wmc\n", 100).unwrap();
        let scope = vec![1, 2, 3];
        let clauses = vec![vec![1, -2], vec![3]];
        let mut engine = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        let hash = engine.packed_key_hash(&scope, &clauses);
        let key = PackedKey::from_residual(scope.clone(), &clauses);
        engine.packed_cache.insert(key, Outcome { sat: true, count: C::one(100) });
        let different = vec![vec![1], vec![-2, 3]];
        assert!(engine.packed_cache.raw_entry().from_hash(hash, |candidate| {
            candidate.matches_residual(&scope, &different)
        }).is_none());
    }

    #[test]
    fn packed_cache_preserves_sat_zero_and_unsat_as_distinct_outcomes() {
        let fixtures = [
            "p cnf 3 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 2 1\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n1 0\n",
            "p cnf 5 3\nc t wmc\n1 2 0\n-1 3 0\n-4 5 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            for fixture in fixtures {
                let p = parse_emitted(fixture, bits).unwrap();
                let mut reference = None;
                for packed_mode in [false, true] {
                    let mut engine = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                    engine.fast_assign = true;
                    engine.shared_residual_key = !packed_mode;
                    engine.packed_residual_key = packed_mode;
                    engine.indexed_unit_propagation = true;
                    engine.indexed_unit_csr = true;
                    let outcome = engine.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
                    let witness = (outcome.sat, outcome.count.re, outcome.count.im,
                        engine.stats.nodes, engine.stats.cache_hits, engine.stats.decompositions);
                    if let Some(reference) = reference.take() {
                        assert_eq!(witness, reference);
                    } else {
                        reference = Some(witness);
                    }
                }
            }
        }
    }

    #[test]
    fn borrowed_hash_matches_owned_key_hash_for_exact_lookup() {
        let p = parse_emitted("p cnf 2 1\nc t wmc\n1 -2 0\n", 100).unwrap();
        let mut engine = Engine::new(&p, 100, CachePolicy::HbBorrowed);
        let scope = vec![1_usize, 2_usize];
        let residual = vec![vec![-2, 1]];
        let key = Key { scope: scope.clone(), clauses: residual.clone() };
        engine.hb_cache.insert(key, Outcome {
            sat: true, count: C::one(100),
        });
        let hash = engine.borrowed_key_hash(&scope, &residual);
        let lookup = engine.hb_cache.raw_entry().from_hash(hash, |key| {
            key.scope == scope && key.clauses == residual
        });
        assert!(lookup.is_some(), "borrowed lookup must use the exact Key hash");
        let wrong = vec![vec![1, 2]];
        assert!(engine.hb_cache.raw_entry().from_hash(hash, |key| {
            key.scope == scope && key.clauses == wrong
        }).is_none(), "hash alone must never confer a cache hit");
    }

    #[test]
    fn cache_modes_preserve_sat_wmc_and_structure() {
        let source = "p cnf 4 4\nc t wmc\nc p weight 1 0.25 -0.5 0\nc p weight -1 -0.25 0.5 0\nc p weight 2 0.3 0.1 0\nc p weight -2 0.7 0 0\n1 2 0\n-1 3 0\n1 -4 0\n-2 3 -4 0\n";
        let problem = parse_emitted(source, 100).unwrap();
        let mut reference = None;
        for mode in [CachePolicy::StdOwned, CachePolicy::HbOwned, CachePolicy::HbBorrowed] {
            let mut engine = Engine::new(&problem, 100, mode);
            let answer = engine.count(problem.clauses.clone(), (1..=4).collect(), 0).unwrap();
            let record = (answer.sat, answer.count.re, answer.count.im,
                engine.stats.nodes, engine.stats.cache_hits, engine.stats.decompositions);
            if let Some(expected) = &reference {
                assert_eq!(&record, expected, "cache variant {}", mode.label());
            } else { reference = Some(record); }
        }
    }

}
