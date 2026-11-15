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
    cache_limit: usize,
    stats: Stats,
    fast_assign: bool,
    node_budget: u64,
    indexed_unit_propagation: bool,
    indexed_activation_node: u64,
    indexed_unit_csr: bool,
}
impl<'a> Engine<'a> {
    fn new(p: &'a Problem, bits: u32, cache_policy: CachePolicy) -> Engine<'a> {
        Engine {
            p, bits, cache: StdHashMap::new(), hb_cache: HbHashMap::new(),
            shared_cache: HbHashMap::new(), packed_cache: HbHashMap::new(),
            cache_policy, shared_residual_key: false, packed_residual_key: false,
            packed_fast_admission: false, cache_limit: 10_000,
            stats: Stats::default(), fast_assign: false, node_budget: 100_000,
            indexed_unit_propagation: false, indexed_activation_node: 0,
            indexed_unit_csr: false,
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
            if self.packed_fast_admission && self.packed_cache.len() >= self.cache_limit {
                self.stats.packed_full_misses += 1;
                // Cache still must be probed for hits, but once full a miss
                // can never be admitted. Avoid allocating Arc and key scope.
                (ResidualStorage::Owned(canonical), StoredKey::NoCache)
            } else {
                let shared = Arc::new(canonical);
                let key = SharedKey { scope: scope.clone(), clauses: Arc::clone(&shared) };
                if self.packed_fast_admission {
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
                unit_propagate_indexed_csr(
                    &residual, self.p.n, &self.p.weights, self.bits, &mut remaining, &mut prefix
                )
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
        let mut frequency = vec![0_usize; self.p.n + 1];
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
        let var = *active.iter().max_by_key(|&&v| frequency[v]).ok_or("no active pivot")?;
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
        "usage: rawmocs-mpfr-research [--node-budget=250000|--node-budget=1000000] [--cache-limit=25000|--cache-limit=50000|--cache-limit=100000] [--indexed-unit-prop|--indexed-unit-prop-after-8192|--indexed-unit-prop-csr|--indexed-unit-prop-csr-after-8192] [--shared-residual-key|--packed-residual-key|--packed-residual-key-fast] [--fast-assign] [--hashbrown-owned|--hashbrown-borrowed] DIGITS FILE"
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
    let packed_residual_key = storage_start.to_str() == Some("--packed-residual-key")
        || packed_fast_admission;
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
    let path = args.next().ok_or("missing CNF")?;
    if args.next().is_some() { return Err("expected exactly one file".into()); }
    let t0 = Instant::now();
    let raw = std::fs::read_to_string(Path::new(&path)).map_err(|e| format!("read: {e}"))?;
    let read_ns = t0.elapsed().as_nanos();
    let t1 = Instant::now();
    let p = parse_emitted(&raw, bits)?;
    let parse_ns = t1.elapsed().as_nanos();
    let t2 = Instant::now();
    let mut engine = Engine::new(&p, bits, cache_policy);
    engine.fast_assign = fast_assign;
    engine.shared_residual_key = shared_residual_key;
    engine.packed_residual_key = packed_residual_key;
    engine.packed_fast_admission = packed_fast_admission;
    engine.cache_limit = cache_limit;
    engine.node_budget = node_budget;
    engine.indexed_unit_propagation = indexed_unit_propagation;
    engine.indexed_unit_csr = indexed_unit_csr;
    engine.indexed_activation_node = indexed_activation_node;
    let answer = engine.count(p.clauses.clone(), (1..=p.n).collect(), 0)?;
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
        engine.stats.nodes, engine.stats.cache_hits, engine.stats.decompositions);
    eprintln!("RESEARCH_MPFR_CACHE_POLICY {}", cache_policy.label());
    eprintln!("RESEARCH_MPFR_SHARED_RESIDUAL enabled={} entries={}",
        shared_residual_key, engine.shared_cache.len());
    eprintln!("RESEARCH_MPFR_CACHE_LIMIT capacity={} shared_entries={} owned_entries={}",
        cache_limit, engine.shared_cache.len(), engine.hb_cache.len() + engine.cache.len());
    eprintln!("RESEARCH_MPFR_PACKED_RESIDUAL enabled={} entries={}",
        packed_residual_key, engine.packed_cache.len());
    eprintln!("RESEARCH_MPFR_PACKED_FAST_ADMISSION enabled={packed_fast_admission} full_misses={}",
        engine.stats.packed_full_misses);
    eprintln!("RESEARCH_MPFR_UNIT_PROP_INDEXED enabled={indexed_unit_propagation} activation_node={indexed_activation_node}");
    eprintln!("RESEARCH_MPFR_UNIT_PROP_CSR enabled={indexed_unit_csr} activation_node={indexed_activation_node}");
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
    fn packed_fast_admission_never_hash_collides_to_false_cache_hit() {
        let p = parse_emitted("p cnf 3 0\\nc t wmc\\n", 100).unwrap();
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
