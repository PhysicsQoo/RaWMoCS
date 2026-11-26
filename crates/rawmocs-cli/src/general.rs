//! Minimal production port of the original MPFR chronological DPLL control.
//! Research source: 379b47242cbd9b4a1e597afc6377cbaf0ac13bce,
//! benchmarks/mpfr_research/src/main.rs (Engine::count, normalize,
//! assign, split_components). Preserve chronological order and exact cache equality.
use crate::{Clause, Outcome, Problem, C};
use std::borrow::Borrow;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::hash::{Hash, Hasher};
use std::time::Instant;

#[derive(Hash, Eq, PartialEq)]
struct Key {
    scope: Vec<usize>,
    clauses: Vec<Clause>,
}
// Borrowed queries hash and compare the entire ordered key, including clause
// boundaries and scope. The HashMap still owns every admitted cache key.
trait KeyView {
    fn scope(&self) -> &[usize];
    fn clauses(&self) -> &[Clause];
}
impl KeyView for Key {
    fn scope(&self) -> &[usize] {
        &self.scope
    }
    fn clauses(&self) -> &[Clause] {
        &self.clauses
    }
}
struct KeyQuery<'a> {
    scope: &'a [usize],
    clauses: &'a [Clause],
}
impl KeyView for KeyQuery<'_> {
    fn scope(&self) -> &[usize] {
        self.scope
    }
    fn clauses(&self) -> &[Clause] {
        self.clauses
    }
}
impl Hash for dyn KeyView + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.scope().hash(state);
        self.clauses().hash(state);
    }
}
impl PartialEq for dyn KeyView + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.scope() == other.scope() && self.clauses() == other.clauses()
    }
}
impl Eq for dyn KeyView + '_ {}
impl<'a> Borrow<dyn KeyView + 'a> for Key {
    fn borrow(&self) -> &(dyn KeyView + 'a) {
        self
    }
}
fn key_items(scope: &[usize], clauses: &[Clause]) -> usize {
    scope.len() + clauses.len() + clauses.iter().map(Vec::len).sum::<usize>()
}
fn cache_can_admit(entries: usize, used: usize, items: usize) -> bool {
    entries < 10_000 && used + items <= 400_000
}
type Component = (Vec<usize>, Vec<Clause>);

enum Step {
    Enter(Vec<Clause>, Vec<usize>),
    Done(Outcome),
}
enum Frame {
    Yes {
        key: Option<Key>,
        prefix: C,
        var: usize,
        no_clauses: Vec<Clause>,
        no_scope: Vec<usize>,
    },
    No {
        key: Option<Key>,
        prefix: C,
        var: usize,
        yes: Outcome,
    },
    Parts {
        key: Option<Key>,
        prefix: C,
        value: C,
        remaining: std::vec::IntoIter<Component>,
    },
}
fn push(stack: &mut Vec<Frame>, frame: Frame) -> Result<(), String> {
    stack
        .try_reserve(1)
        .map_err(|_| "general frame allocation")?;
    stack.push(frame);
    Ok(())
}

// Test-only trace; never add fingerprint overhead to release builds.
#[cfg(test)]
#[derive(Default)]
struct WorkTrace {
    hasher: std::collections::hash_map::DefaultHasher,
    nodes: usize,
    hits: usize,
    splits: usize,
    branches: usize,
    free: usize,
}
#[cfg(test)]
impl WorkTrace {
    fn enter(&mut self, scope: &[usize], clauses: &[Clause]) {
        self.nodes += 1;
        scope.hash(&mut self.hasher);
        clauses.hash(&mut self.hasher);
    }
    fn summary(&self) -> (u64,usize,usize,usize,usize,usize) {
        (self.hasher.finish(),self.nodes,self.hits,self.splits,self.branches,self.free)
    }
}

struct Engine<'a> {
    p: &'a Problem,
    bits: u32,
    deadline: Instant,
    cache: HashMap<Key, Outcome>,
    cache_items: usize,
    large: bool,
    frequency: Vec<usize>,
    parent: Vec<usize>,
    #[cfg(test)]
    trace: Option<WorkTrace>,
    #[cfg(test)]
    work_nodes: usize,
    #[cfg(test)]
    work_limit: usize,
}
fn normalize(mut clauses: Vec<Clause>) -> Vec<Clause> {
    for clause in &mut clauses {
        clause.sort_unstable();
        clause.dedup();
    }
    clauses.retain(|c| !c.iter().any(|&l| c.binary_search(&-l).is_ok()));
    clauses.sort();
    clauses.dedup();
    clauses
}
fn assign(clauses: &[Clause], lit: i32) -> Vec<Clause> {
    clauses
        .iter()
        .filter(|c| !c.contains(&lit))
        .map(|c| c.iter().copied().filter(|&x| x != -lit).collect())
        .collect()
}
// Stable compaction preserves the clause and literal order of assign.
fn assign_inplace(clauses: &mut Vec<Clause>, lit: i32) {
    clauses.retain_mut(|c| {
        if c.contains(&lit) {
            false
        } else {
            c.retain(|&x| x != -lit);
            true
        }
    });
}
struct Closure {
    clauses: Vec<Clause>,
    forced: Vec<i32>,
}

fn occurrence_slot(lit: i32) -> usize {
    2 * (lit.unsigned_abs() as usize - 1) + usize::from(lit < 0)
}

fn indexed_units(
    clauses: Vec<Clause>,
    n: usize,
    deadline: Instant,
) -> Result<Option<Closure>, String> {
    let check = || {
        if Instant::now() >= deadline {
            Err("general deadline".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    let mut occurrences = vec![Vec::<usize>::new(); 2 * n];
    let mut lengths = Vec::with_capacity(clauses.len());
    let mut units = BinaryHeap::new();
    for (i, clause) in clauses.iter().enumerate() {
        check()?;
        if clause.is_empty() {
            return Ok(None);
        }
        lengths.push(clause.len());
        if clause.len() == 1 {
            units.push(Reverse(i));
        }
        for &lit in clause {
            occurrences[occurrence_slot(lit)].push(i);
        }
    }
    let mut values = vec![0i8; n + 1];
    let mut forced = Vec::new();
    // usize::MAX denotes a satisfied clause; zero is an immediate conflict.
    while let Some(Reverse(i)) = units.pop() {
        check()?;
        if lengths[i] != 1 {
            continue;
        }
        let lit = clauses[i]
            .iter()
            .copied()
            .find(|l| values[l.unsigned_abs() as usize] == 0)
            .ok_or("indexed unit invariant")?;
        let v = lit.unsigned_abs() as usize;
        values[v] = if lit > 0 { 1 } else { -1 };
        forced.push(lit);
        for &j in &occurrences[occurrence_slot(lit)] {
            lengths[j] = usize::MAX;
        }
        for &j in &occurrences[occurrence_slot(-lit)] {
            if lengths[j] == usize::MAX {
                continue;
            }
            lengths[j] -= 1;
            if lengths[j] == 0 {
                return Ok(None);
            }
            if lengths[j] == 1 {
                units.push(Reverse(j));
            }
        }
    }
    let mut residual = Vec::new();
    for (i, mut clause) in clauses.into_iter().enumerate() {
        check()?;
        if lengths[i] != usize::MAX {
            clause.retain(|l| values[l.unsigned_abs() as usize] == 0);
            residual.push(clause);
        }
    }
    Ok(Some(Closure {
        clauses: residual,
        forced,
    }))
}

fn root(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        let p = parent[x];
        parent[x] = parent[p];
        x = parent[x];
    }
    x
}
fn components_with_parent(
    clauses: &mut Vec<Clause>,
    scope: &[usize],
    parent: &mut [usize],
) -> Vec<Component> {
    for &v in scope {
        parent[v] = v;
    }
    for c in clauses.iter() {
        let first = c[0].unsigned_abs() as usize;
        for &lit in &c[1..] {
            let a = root(parent, first);
            let b = root(parent, lit.unsigned_abs() as usize);
            parent[b] = a;
        }
    }
    let mut groups: BTreeMap<usize, (Vec<usize>, Vec<Clause>)> = BTreeMap::new();
    for &v in scope {
        groups.entry(root(parent, v)).or_default().0.push(v);
    }
    if groups.len() <= 1 {
        return Vec::new();
    }
    // Consume the residual only when a split is known to exist. Keep the
    // original representatives, component order and per-component clause order.
    for c in std::mem::take(clauses) {
        let r = root(parent, c[0].unsigned_abs() as usize);
        groups.entry(r).or_default().1.push(c);
    }
    groups.into_values().collect()
}
impl Engine<'_> {
    fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("general deadline".into());
        }
        Ok(())
    }
    fn weight(&self, v: usize, positive: bool) -> &C {
        let w = &self.p.weights[v - 1];
        if positive {
            &w.positive
        } else {
            &w.negative
        }
    }
    fn save(&mut self, key: Option<Key>, out: Outcome) -> Result<Outcome, String> {
        if !out.count.re.is_finite() || !out.count.im.is_finite() {
            return Err("nonfinite general".into());
        }
        // Descendant inserts may consume the remaining budget.
        if let Some(key) = key {
            let items = key_items(&key.scope, &key.clauses);
            if cache_can_admit(self.cache.len(), self.cache_items, items) {
                self.cache_items += items;
                self.cache.insert(key, out.clone());
            }
        }
        Ok(out)
    }
    fn enter(
        &mut self,
        clauses: Vec<Clause>,
        scope: Vec<usize>,
        stack: &mut Vec<Frame>,
    ) -> Result<Step, String> {
        self.check()?;
        #[cfg(test)]
        {
            if self.work_nodes >= self.work_limit { return Err("baseline research node limit".into()); }
            self.work_nodes += 1;
        }
        let mut residual = normalize(clauses);
        #[cfg(test)]
        if let Some(t) = &mut self.trace { t.enter(&scope, &residual); }
        let query = KeyQuery {
            scope: &scope,
            clauses: &residual,
        };
        let view: &dyn KeyView = &query;
        if let Some(hit) = self.cache.get(view) {
            #[cfg(test)]
            if let Some(t) = &mut self.trace { t.hits += 1; }
            return Ok(Step::Done(hit.clone()));
        }
        // Without eviction, both admission counters are monotone.
        // An inadmissible key cannot become admissible during this frame.
        let key = cache_can_admit(
            self.cache.len(),
            self.cache_items,
            key_items(&scope, &residual),
        )
        .then(|| Key {
            scope: scope.clone(),
            clauses: residual.clone(),
        });
        let mut remaining = scope;
        let mut prefix = C::one(self.bits);
        let mut scanned = 0;
        loop {
            self.check()?;
            if residual.iter().any(Vec::is_empty) {
                return self.save(key, Outcome::unsat(self.bits)).map(Step::Done);
            }
            let Some(lit) = residual.iter().find(|c| c.len() == 1).map(|c| c[0]) else {
                break;
            };
            // Only the explicit large-input route uses the index. A third
            // live unit and substantial residual amortize construction.
            if self.large && scanned >= 2 && residual.len() >= 4096 {
                let Some(closed) = indexed_units(residual, self.p.n, self.deadline)? else {
                    return self.save(key, Outcome::unsat(self.bits)).map(Step::Done);
                };
                let mut assigned = vec![false; self.p.n + 1];
                for lit in closed.forced {
                    self.check()?;
                    let v = lit.unsigned_abs() as usize;
                    prefix = prefix.mul(self.weight(v, lit > 0), self.bits);
                    assigned[v] = true;
                }
                remaining.retain(|&v| !assigned[v]);
                residual = closed.clauses;
                break;
            }
            scanned += 1;
            let v = lit.unsigned_abs() as usize;
            prefix = prefix.mul(self.weight(v, lit > 0), self.bits);
            remaining.retain(|&x| x != v);
            assign_inplace(&mut residual, lit);
        }
        // Every residual literal belongs to remaining. The scratch arrays
        // never escape enter into a continuation frame. Reset only live scope.
        for &v in &remaining {
            self.frequency[v] = 0;
        }
        for clause in &residual {
            for &lit in clause {
                self.frequency[lit.unsigned_abs() as usize] += 1;
            }
        }
        let mut active = Vec::new();
        for v in remaining {
            if self.frequency[v] == 0 {
                #[cfg(test)]
                if let Some(t) = &mut self.trace { t.free += 1; }
                let w = &self.p.weights[v - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative, self.bits), self.bits);
            } else {
                active.push(v);
            }
        }
        if residual.is_empty() {
            return self
                .save(
                    key,
                    Outcome {
                        sat: true,
                        count: prefix,
                    },
                )
                .map(Step::Done);
        }
        let mut parts =
            components_with_parent(&mut residual, &active, &mut self.parent).into_iter();
        if let Some((scope, clauses)) = parts.next() {
            #[cfg(test)]
            if let Some(t) = &mut self.trace { t.splits += 1; }
            push(
                stack,
                Frame::Parts {
                    key,
                    prefix,
                    value: C::one(self.bits),
                    remaining: parts,
                },
            )?;
            return Ok(Step::Enter(clauses, scope));
        }
        #[cfg(test)]
        if let Some(t) = &mut self.trace { t.branches += 1; }
        let v = *active
            .iter()
            .max_by_key(|&&v| self.frequency[v])
            .ok_or("no active pivot")?;
        let child: Vec<_> = active.into_iter().filter(|&x| x != v).collect();
        // Queue a compact negative child; donate the owned buffers to the
        // positive child, which is visited immediately.
        let no = assign(&residual, -(v as i32));
        assign_inplace(&mut residual, v as i32);
        let yes = residual;
        push(
            stack,
            Frame::Yes {
                key,
                prefix,
                var: v,
                no_clauses: no,
                no_scope: child.clone(),
            },
        )?;
        Ok(Step::Enter(yes, child))
    }
    fn count(&mut self, clauses: Vec<Clause>, scope: Vec<usize>) -> Result<Outcome, String> {
        // Heap-resident continuations preserve the recursive control's
        // positive-before-negative and component aggregation order.
        let mut stack = Vec::new();
        let mut peak_frames = 0;
        let mut step = Step::Enter(clauses, scope);
        loop {
            self.check()?;
            peak_frames = peak_frames.max(stack.len());
            step = match step {
                Step::Enter(clauses, scope) => self.enter(clauses, scope, &mut stack)?,
                Step::Done(out) => match stack.pop() {
                    None => {
                        eprintln!("RAWMOCS_GENERAL_COMPLETE max_frames={peak_frames}");
                        return Ok(out);
                    }
                    Some(Frame::Yes {
                        key,
                        prefix,
                        var,
                        no_clauses,
                        no_scope,
                    }) => {
                        push(
                            &mut stack,
                            Frame::No {
                                key,
                                prefix,
                                var,
                                yes: out,
                            },
                        )?;
                        Step::Enter(no_clauses, no_scope)
                    }
                    Some(Frame::No {
                        key,
                        prefix,
                        var,
                        yes,
                    }) => {
                        let y = self.weight(var, true).mul(&yes.count, self.bits);
                        let n = self.weight(var, false).mul(&out.count, self.bits);
                        Step::Done(self.save(
                            key,
                            Outcome {
                                sat: yes.sat || out.sat,
                                count: prefix.mul(&y.add(&n, self.bits), self.bits),
                            },
                        )?)
                    }
                    Some(Frame::Parts {
                        key,
                        prefix,
                        mut value,
                        mut remaining,
                    }) => {
                        if !out.sat {
                            Step::Done(self.save(key, Outcome::unsat(self.bits))?)
                        } else {
                            value = value.mul(&out.count, self.bits);
                            if let Some((scope, clauses)) = remaining.next() {
                                push(
                                    &mut stack,
                                    Frame::Parts {
                                        key,
                                        prefix,
                                        value,
                                        remaining,
                                    },
                                )?;
                                Step::Enter(clauses, scope)
                            } else {
                                Step::Done(self.save(
                                    key,
                                    Outcome {
                                        sat: true,
                                        count: prefix.mul(&value, self.bits),
                                    },
                                )?)
                            }
                        }
                    }
                },
            };
        }
    }
}

pub(crate) fn solve(p: &Problem, bits: u32, deadline: Instant) -> Result<Outcome, String> {
    solve_mode(p, bits, deadline, false, None)
}

pub(crate) fn solve_large(mut p: Problem, bits: u32, deadline: Instant) -> Result<Outcome, String> {
    // The large route is terminal: the parser's clause vector is no
    // longer needed after solve. Weights and the declared scope remain
    // immutable for the entire search, as in the borrowed route.
    let root = std::mem::take(&mut p.clauses);
    solve_mode(&p, bits, deadline, true, Some(root))
}

fn solve_mode(
    p: &Problem,
    bits: u32,
    deadline: Instant,
    large: bool,
    root: Option<Vec<Clause>>,
) -> Result<Outcome, String> {
    let mut engine = Engine {
        p,
        bits,
        deadline,
        cache: HashMap::new(),
        cache_items: 0,
        #[cfg(test)]
        trace: None,
        #[cfg(test)]
        work_nodes: 0,
        #[cfg(test)]
        work_limit: usize::MAX,
        large,
        frequency: vec![0; p.n + 1],
        parent: vec![0; p.n + 1],
    };
    let clauses = root.unwrap_or_else(|| p.clauses.clone());
    engine.count(clauses, (1..=p.n).collect())
}

#[cfg(test)]
fn components(clauses: &mut Vec<Clause>, scope: &[usize], n: usize) -> Vec<Component> {
    components_with_parent(clauses, scope, &mut vec![0; n + 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_emitted;
    use std::time::Duration;

    #[test]
    fn owned_large_root_matches_borrowed_root() {
        for bits in [100, 200, 333, 432] {
            for raw in [
                "p cnf 4 2\nc t wmc\nc p weight 1 -0.5 0.25 0\n1 2 0\n-1 3 0\n",
                "p cnf 4 2\nc t wmc\n1 2 0\n3 4 0\n",
                "p cnf 1 2\nc t wmc\n1 0\n-1 0\n",
                "p cnf 0 0\nc t wmc\n",
                "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",
            ] {
                let deadline = Instant::now() + Duration::from_secs(30);
                let p = parse_emitted(raw, bits).unwrap();
                let a = solve_mode(&p, bits, deadline, true, None).unwrap();
                let b = solve_large(p, bits, deadline).unwrap();
                assert_eq!(a.sat, b.sat);
                assert_eq!(a.count.re, b.count.re);
                assert_eq!(a.count.im, b.count.im);
            }
        }
    }

    #[test]
    fn ordered_index_matches_scan_on_normalized_residuals() {
        let mut seed = 1731_u32;
        let deadline = Instant::now() + Duration::from_secs(30);
        for _ in 0..2048 {
            let mut clauses = Vec::new();
            for _ in 0..16 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let width = ((seed >> 16) % 5) as usize;
                let mut clause = Vec::new();
                for _ in 0..width {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let v = ((seed >> 16) % 8 + 1) as i32;
                    clause.push(if seed & 256 == 0 { v } else { -v });
                }
                clauses.push(clause);
            }
            let mut scan = normalize(clauses);
            let indexed = indexed_units(scan.clone(), 8, deadline).unwrap();
            let mut forced = Vec::new();
            let expected = loop {
                if scan.iter().any(Vec::is_empty) {
                    break None;
                }
                if let Some(lit) = scan.iter().find(|c| c.len() == 1).map(|c| c[0]) {
                    forced.push(lit);
                    assign_inplace(&mut scan, lit);
                } else {
                    break Some((scan, forced));
                }
            };
            assert_eq!(indexed.map(|c| (c.clauses, c.forced)), expected);
        }
    }

    #[test]
    fn large_mode_branch_and_root_counts() {
        for bits in [100, 200, 333, 432] {
            for root in [true, false] {
                let mut raw = if root {
                    "p cnf 8192 8192\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight 8192 0.25 -0.125 0\n1 0\n".to_owned()
                } else {
                    "p cnf 8192 8206\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight 8192 0.25 -0.125 0\n".to_owned()
                };
                if !root {
                    for v in 2..=9 {
                        raw.push_str(&format!("1 {v} 0\n-1 {v} 0\n"));
                    }
                }
                for v in (if root { 1 } else { 2 })..8192 {
                    raw.push_str(&format!("-{v} {} 0\n", v + 1));
                }
                let deadline = Instant::now() + Duration::from_secs(30);
                let p = crate::parse_large(std::io::Cursor::new(&raw), bits, deadline).unwrap();
                let a = solve(&p, bits, deadline).unwrap();
                let b = solve_large(p, bits, deadline).unwrap();
                assert!(a.sat && b.sat);
                assert_eq!(a.count.re, b.count.re);
                assert_eq!(a.count.im, b.count.im);
                let expected = if root {
                    (-0.09375, 0.125)
                } else {
                    (0.15625, 0.0)
                };
                assert_eq!(b.count.re, rug::Float::with_val(bits, expected.0));
                assert_eq!(b.count.im, rug::Float::with_val(bits, expected.1));
            }
        }
        let mut raw = "p cnf 8192 8193\nc t wmc\n1 0\n".to_owned();
        for v in 1..8192 {
            raw.push_str(&format!("-{v} {} 0\n", v + 1));
        }
        raw.push_str("-8192 0\n");
        let deadline = Instant::now() + Duration::from_secs(30);
        let p = crate::parse_large(std::io::Cursor::new(&raw), 100, deadline).unwrap();
        let b = solve_large(p, 100, deadline).unwrap();
        assert!(!b.sat);
        assert_eq!(b.count.re, 0);
        assert_eq!(b.count.im, 0);
    }

    #[test]
    fn stable_compaction_matches_copying_assignment() {
        let mut seed = 1729_u32;
        for _ in 0..1024 {
            let mut clauses = Vec::new();
            for _ in 0..12 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let width = ((seed >> 16) % 4) as usize;
                let mut clause = Vec::new();
                for _ in 0..width {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let v = ((seed >> 16) % 6 + 1) as i32;
                    clause.push(if seed & 0x100 == 0 { v } else { -v });
                }
                clauses.push(clause);
            }
            let clauses = normalize(clauses);
            for lit in (-6..=6).filter(|&v| v != 0) {
                let mut candidate = clauses.clone();
                assign_inplace(&mut candidate, lit);
                assert_eq!(candidate, assign(&clauses, lit));
            }
        }
    }

    #[test]
    fn reused_parent_ignores_stale_entries_across_scopes() {
        let mut parent = vec![4096; 4097];
        for scope in [
            vec![1, 2, 3, 4],
            vec![100, 102, 4095, 4096],
            vec![2, 3, 102, 4096],
        ] {
            let mut clauses = vec![
                vec![scope[0] as i32, scope[1] as i32],
                vec![-(scope[2] as i32), scope[3] as i32],
            ];
            let mut reference = clauses.clone();
            assert_eq!(
                components_with_parent(&mut clauses, &scope, &mut parent),
                components(&mut reference, &scope, 4096)
            );
            assert!(clauses.is_empty());
        }
    }

    #[test]
    fn component_transfer_preserves_order_and_connected_residual() {
        let mut clauses = vec![vec![1, 2], vec![-3, 4], vec![5, 6]];
        let parts = components(&mut clauses, &[1, 2, 3, 4, 5, 6], 6);
        assert_eq!(
            parts,
            vec![
                (vec![1, 2], vec![vec![1, 2]]),
                (vec![3, 4], vec![vec![-3, 4]]),
                (vec![5, 6], vec![vec![5, 6]]),
            ]
        );
        assert!(clauses.is_empty());
        let mut connected = vec![vec![1, 2], vec![2, 3]];
        let original = connected.clone();
        assert!(components(&mut connected, &[1, 2, 3], 3).is_empty());
        assert_eq!(connected, original);
    }

    #[test]
    fn borrowed_key_matches_owned_exactly() {
        let mut map = HashMap::new();
        let scope = vec![1, 2, 3];
        let clauses = vec![vec![-1, 2], vec![1, 3]];
        map.insert(
            Key {
                scope: scope.clone(),
                clauses: clauses.clone(),
            },
            17,
        );
        let query = KeyQuery {
            scope: &scope,
            clauses: &clauses,
        };
        let view: &dyn KeyView = &query;
        assert_eq!(map.get(view), Some(&17));
        for (s, c) in [
            (vec![1, 2], clauses.clone()),
            (vec![2, 1, 3], clauses.clone()),
            (scope.clone(), vec![vec![1, 3], vec![-1, 2]]),
            (scope.clone(), vec![vec![-1, 2, 1, 3]]),
            (scope.clone(), vec![vec![1, 2], vec![1, 3]]),
        ] {
            let q = KeyQuery {
                scope: &s,
                clauses: &c,
            };
            let v: &dyn KeyView = &q;
            assert_eq!(map.get(v), None);
        }
        assert!(cache_can_admit(9999, 399999, 1));
        assert!(!cache_can_admit(10000, 0, 1));
        assert!(!cache_can_admit(9999, 399999, 2));
    }

    #[test]
    fn optimized_general_matches_independent_reference() {
        for bits in [100, 200, 333, 432] {
            for raw in [
                "p cnf 4 3\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight 2 0.25 -0.125 0\n1 0\n-1 2 0\n3 4 0\n",
                "p cnf 2 2\nc t wmc\nc p weight 2 -1 0 0\n1 0\n1 2 0\n",
                "p cnf 2 3\nc t wmc\n1 0\n-1 2 0\n-2 0\n",
                "p cnf 6 3\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight -3 0.25 -0.125 0\n1 2 0\n-3 4 0\n5 6 0\n",
            ] {
                let p = parse_emitted(raw, bits).unwrap();
                let deadline = Instant::now() + Duration::from_secs(30);
                let candidate = solve(&p, bits, deadline).unwrap();
                let exact = crate::reference(&p, raw, bits).unwrap();
                assert_eq!(exact.sat, candidate.sat);
                assert_eq!(exact.count.re, candidate.count.re);
                assert_eq!(exact.count.im, candidate.count.im);
            }
        }
    }
}

#[cfg(test)]
mod reversible_index_research;
