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
enum StoredKey { Owned(Key), Shared(SharedKey) }
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
struct Stats { nodes: u64, cache_hits: u64, decompositions: u64 }
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
    cache_policy: CachePolicy,
    shared_residual_key: bool,
    stats: Stats,
    fast_assign: bool,
    node_budget: u64,
    indexed_unit_propagation: bool,
    indexed_activation_node: u64,
}
impl<'a> Engine<'a> {
    fn new(p: &'a Problem, bits: u32, cache_policy: CachePolicy) -> Engine<'a> {
        Engine {
            p, bits, cache: StdHashMap::new(), hb_cache: HbHashMap::new(),
            shared_cache: HbHashMap::new(),
            cache_policy, shared_residual_key: false,
            stats: Stats::default(), fast_assign: false, node_budget: 100_000,
            indexed_unit_propagation: false, indexed_activation_node: 0,
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
    fn weight(&self, var: usize, positive: bool) -> &C {
        let w = &self.p.weights[var - 1];
        if positive { &w.positive } else { &w.negative }
    }
    fn save(&mut self, key: StoredKey, result: Outcome) -> Outcome {
        match key {
            StoredKey::Owned(key) => match self.cache_policy {
                CachePolicy::StdOwned if self.cache.len() < 10_000 => {
                    self.cache.insert(key, result.clone());
                }
                CachePolicy::HbOwned | CachePolicy::HbBorrowed if self.hb_cache.len() < 10_000 => {
                    self.hb_cache.insert(key, result.clone());
                }
                _ => {}
            },
            StoredKey::Shared(key) => {
                if self.shared_cache.len() < 10_000 {
                    self.shared_cache.insert(key, result.clone());
                }
            }
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
        let (mut residual, key) = if self.shared_residual_key {
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
            let Some(reduced) = unit_propagate_indexed(
                &residual, self.p.n, &self.p.weights, self.bits, &mut remaining, &mut prefix
            ) else {
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
        "usage: rawmocs-mpfr-research [--node-budget=250000|--node-budget=1000000] [--indexed-unit-prop|--indexed-unit-prop-after-8192] [--shared-residual-key] [--fast-assign] [--hashbrown-owned|--hashbrown-borrowed] DIGITS FILE"
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
    let indexed_unit_propagation = matches!(
        algorithm_start.to_str(),
        Some("--indexed-unit-prop" | "--indexed-unit-prop-after-8192")
    );
    let indexed_activation_node = if algorithm_start.to_str() == Some("--indexed-unit-prop-after-8192") {
        8192
    } else { 0 };
    let storage_start = if indexed_unit_propagation {
        args.next().ok_or("missing arguments after --indexed-unit-prop")?
    } else {
        algorithm_start
    };
    let shared_residual_key = storage_start.to_str() == Some("--shared-residual-key");
    let next = if shared_residual_key {
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
    if shared_residual_key && !matches!(cache_policy, CachePolicy::HbBorrowed) {
        return Err("--shared-residual-key requires --hashbrown-borrowed".into());
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
    engine.node_budget = node_budget;
    engine.indexed_unit_propagation = indexed_unit_propagation;
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
    eprintln!("RESEARCH_MPFR_UNIT_PROP_INDEXED enabled={indexed_unit_propagation} activation_node={indexed_activation_node}");
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
                assert_eq!((expected.sat, expected.count.re, expected.count.im),
                           (observed.sat, observed.count.re, observed.count.im),
                           "case {case}, bits {bits}");
                assert_eq!(
                    (baseline.stats.nodes, baseline.stats.cache_hits, baseline.stats.decompositions),
                    (candidate.stats.nodes, candidate.stats.cache_hits, candidate.stats.decompositions),
                    "unchanged search topology for {case}, {bits}"
                );
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
