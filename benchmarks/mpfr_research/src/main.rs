//! Isolated research MPFR arithmetic arm: unchanged production core and API.
//! Source-emitted QuPRS CNF, original chronological DPLL, component split,
//! frequency pivot and 10,000-entry exact-shape residual cache.
//! Verify structure against main exact-rational solve before claiming parity.
use rug::Float;
use std::cell::Cell;
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
    cache_policy: CachePolicy,
    identity_mul: bool,
    identity_skips: Cell<u64>,
    stats: Stats,
}
impl<'a> Engine<'a> {
    fn new(p: &'a Problem, bits: u32, cache_policy: CachePolicy) -> Engine<'a> {
        Engine {
            p, bits, cache: StdHashMap::new(), hb_cache: HbHashMap::new(),
            cache_policy, identity_mul: false, identity_skips: Cell::new(0),
            stats: Stats::default(),
        }
    }
    fn borrowed_key_hash(&self, scope: &[usize], residual: &[Clause]) -> u64 {
        // Match Key's derived Hash field sequence exactly, with borrowed slices.
        let mut hasher = self.hb_cache.hasher().build_hasher();
        scope.hash(&mut hasher);
        residual.hash(&mut hasher);
        hasher.finish()
    }
    fn multiply(&self, lhs: &C, rhs: &C) -> C {
        // Opt-in research-only identity path. No approximate zero threshold:
        // exactly 1+0i is the MPFR complex multiplicative identity.
        // Keep the same precision; never substitute fused arithmetic.
        if self.identity_mul {
            if lhs.re == 1 && lhs.im == 0 {
                self.identity_skips.set(self.identity_skips.get().saturating_add(1));
                return rhs.clone();
            }
            if rhs.re == 1 && rhs.im == 0 {
                self.identity_skips.set(self.identity_skips.get().saturating_add(1));
                return lhs.clone();
            }
        }
        lhs.mul(rhs, self.bits)
    }
    fn weight(&self, var: usize, positive: bool) -> &C {
        let w = &self.p.weights[var - 1];
        if positive { &w.positive } else { &w.negative }
    }
    fn save(&mut self, key: Key, result: Outcome) -> Outcome {
        match self.cache_policy {
            CachePolicy::StdOwned if self.cache.len() < 10_000 => {
                self.cache.insert(key, result.clone());
            }
            CachePolicy::HbOwned | CachePolicy::HbBorrowed if self.hb_cache.len() < 10_000 => {
                self.hb_cache.insert(key, result.clone());
            }
            _ => {}
        }
        result
    }
    fn count(&mut self, clauses: Vec<Clause>, scope: Vec<usize>, depth: usize) -> Result<Outcome, String> {
        if depth >= 512 { return Err("depth limit".into()); }
        if self.stats.nodes >= 100_000 { return Err("node limit".into()); }
        self.stats.nodes += 1;
        let mut residual = if depth == 0 { normalize(clauses) } else { normalize_child(clauses) };
        // The original arm duplicates the entire Key before every lookup.
        // The borrowed arm hashes the existing normalized residual and scope
        // without deep-copying either; only a true miss constructs a Key.
        // Exact equality is still checked in the hash table (no hash-only hits).
        let key = match self.cache_policy {
            CachePolicy::StdOwned => {
                let key = Key { scope: scope.clone(), clauses: residual.clone() };
                if let Some(hit) = self.cache.get(&key) {
                    self.stats.cache_hits += 1;
                    return Ok(hit.clone());
                }
                key
            }
            CachePolicy::HbOwned => {
                let key = Key { scope: scope.clone(), clauses: residual.clone() };
                if let Some(hit) = self.hb_cache.get(&key) {
                    self.stats.cache_hits += 1;
                    return Ok(hit.clone());
                }
                key
            }
            CachePolicy::HbBorrowed => {
                let hash = self.borrowed_key_hash(&scope, &residual);
                if let Some((_, hit)) = self.hb_cache.raw_entry().from_hash(hash, |key| {
                    key.scope.as_slice() == scope.as_slice()
                        && key.clauses.as_slice() == residual.as_slice()
                }) {
                    self.stats.cache_hits += 1;
                    return Ok(hit.clone());
                }
                Key { scope: scope.clone(), clauses: residual.clone() }
            }
        };
        let mut remaining = scope;
        let mut prefix = C::one(self.bits);
        loop {
            if residual.iter().any(Vec::is_empty) {
                return Ok(self.save(key, Outcome::unsat(self.bits)));
            }
            let Some(lit) = residual.iter().find(|c| c.len() == 1).map(|c| c[0]) else { break; };
            let var = lit.unsigned_abs() as usize;
            prefix = self.multiply(&prefix, self.weight(var, lit > 0));
            remaining.retain(|&v| v != var);
            residual = assign(&residual, lit);
        }
        if residual.is_empty() {
            for var in remaining {
                let w = &self.p.weights[var - 1];
                prefix = self.multiply(&prefix, &w.positive.add(&w.negative, self.bits));
            }
            return Ok(self.save(key, Outcome { sat: true, count: prefix }));
        }
        let mut frequency = vec![0_usize; self.p.n + 1];
        for clause in &residual {
            for &lit in clause { frequency[lit.unsigned_abs() as usize] += 1; }
        }
        let mut active = Vec::new();
        for var in remaining {
            if frequency[var] != 0 { active.push(var); }
            else {
                let w = &self.p.weights[var - 1];
                prefix = self.multiply(&prefix, &w.positive.add(&w.negative, self.bits));
            }
        }
        if let Some(pieces) = split_components(&residual, &active, self.p.n) {
            self.stats.decompositions += 1;
            let mut value = C::one(self.bits);
            for (cls, sub_scope) in pieces {
                let part = self.count(cls, sub_scope, depth + 1)?;
                if !part.sat { return Ok(self.save(key, Outcome::unsat(self.bits))); }
                value = self.multiply(&value, &part.count);
            }
            return Ok(self.save(key, Outcome {
                sat: true, count: self.multiply(&prefix, &value),
            }));
        }
        let var = *active.iter().max_by_key(|&&v| frequency[v]).ok_or("no active pivot")?;
        let child: Vec<_> = active.into_iter().filter(|&v| v != var).collect();
        let yes = self.count(assign(&residual, var as i32), child.clone(), depth + 1)?;
        let no = self.count(assign(&residual, -(var as i32)), child, depth + 1)?;
        let yes_weighted = self.multiply(self.weight(var, true), &yes.count);
        let no_weighted = self.multiply(self.weight(var, false), &no.count);
        let sum = yes_weighted.add(&no_weighted, self.bits);
        Ok(self.save(key, Outcome {
            sat: yes.sat || no.sat,
            count: self.multiply(&prefix, &sum),
        }))
    }
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
    let mut next = args.next().ok_or(
        "usage: rawmocs-mpfr-research [--hashbrown-owned|--hashbrown-borrowed] [--identity-mul] DIGITS FILE"
    )?;
    let mut cache_policy = CachePolicy::StdOwned;
    let mut selected_cache = false;
    let mut identity_mul = false;
    loop {
        match next.to_str() {
            Some("--hashbrown-owned") | Some("--hashbrown-borrowed") => {
                if selected_cache {
                    return Err("duplicate or conflicting cache policy".into());
                }
                selected_cache = true;
                cache_policy = if next.to_str() == Some("--hashbrown-owned") {
                    CachePolicy::HbOwned
                } else { CachePolicy::HbBorrowed };
            }
            Some("--identity-mul") => {
                if identity_mul { return Err("duplicate identity-mul flag".into()); }
                identity_mul = true;
            }
            _ => break,
        }
        next = args.next().ok_or("precision missing after research mode flag")?;
    }
    let digits: u32 = next.to_str().ok_or("non-UTF8 precision")?
        .parse().map_err(|_| "bad precision")?;
    if !(30..=100).contains(&digits) { return Err("supported precision 30..100 digits".into()); }
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
    engine.identity_mul = identity_mul;
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
    eprintln!("RESEARCH_MPFR_IDENTITY_MUL_SKIPS {}", engine.identity_skips.get());
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

    #[test]
    fn exact_identity_shortcut_preserves_mpfr_components() {
        for bits in [100, 167, 333] {
            let p = parse_emitted("p cnf 1 0\nc t wmc\n", bits).unwrap();
            let mut engine = Engine::new(&p, bits, CachePolicy::HbBorrowed);
            engine.identity_mul = true;
            let one = C::one(bits);
            let values = [
                C::decimal(bits, "0.123456789012345678", "-0.777777777777777").unwrap(),
                C::decimal(bits, "-0.125", "0.375").unwrap(),
                C::decimal(bits, "0", "0").unwrap(),
                one.clone(),
            ];
            for value in &values {
                let reference_left = one.mul(value, bits);
                let reference_right = value.mul(&one, bits);
                let left = engine.multiply(&one, value);
                let right = engine.multiply(value, &one);
                assert_eq!(left.re, reference_left.re);
                assert_eq!(left.im, reference_left.im);
                assert_eq!(right.re, reference_right.re);
                assert_eq!(right.im, reference_right.im);
            }
            assert!(engine.identity_skips.get() > 0);
        }
    }

    #[test]
    fn identity_mode_preserves_dpll_structure_and_full_wmc() {
        let source = "p cnf 3 3\nc t wmc\nc p weight 1 0.5 0.25 0\nc p weight -1 1 0 0\nc p weight 2 -0.75 0.125 0\nc p weight -2 1 0 0\n1 2 0\n-1 3 0\n-2 3 0\n";
        for bits in [100, 333] {
            let p = parse_emitted(source, bits).unwrap();
            let mut baseline = Engine::new(&p, bits, CachePolicy::HbBorrowed);
            let normal = baseline.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
            let mut optimized = Engine::new(&p, bits, CachePolicy::HbBorrowed);
            optimized.identity_mul = true;
            let fast = optimized.count(p.clauses.clone(), (1..=p.n).collect(), 0).unwrap();
            assert_eq!(normal.sat, fast.sat);
            assert_eq!(normal.count.re, fast.count.re);
            assert_eq!(normal.count.im, fast.count.im);
            assert_eq!(baseline.stats.nodes, optimized.stats.nodes);
            assert_eq!(baseline.stats.cache_hits, optimized.stats.cache_hits);
            assert_eq!(baseline.stats.decompositions, optimized.stats.decompositions);
            assert!(optimized.identity_skips.get() > 0);
        }
    }

}
