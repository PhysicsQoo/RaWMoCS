//! Minimal production port of the original MPFR chronological DPLL control.
//! Research source: 379b47242cbd9b4a1e597afc6377cbaf0ac13bce,
//! benchmarks/mpfr_research/src/main.rs (Engine::count, normalize,
//! assign, split_components). No experimental cache or branching switches.
use crate::{Clause, Outcome, Problem, C};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::time::Instant;

#[derive(Hash, Eq, PartialEq)]
struct Key {
    scope: Vec<usize>,
    clauses: Vec<Clause>,
}
type Component = (Vec<usize>, Vec<Clause>);

enum Step {
    Enter(Vec<Clause>, Vec<usize>),
    Done(Outcome),
}
enum Frame {
    Yes {
        key: Key,
        prefix: C,
        var: usize,
        no_clauses: Vec<Clause>,
        no_scope: Vec<usize>,
    },
    No {
        key: Key,
        prefix: C,
        var: usize,
        yes: Outcome,
    },
    Parts {
        key: Key,
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

struct Engine<'a> {
    p: &'a Problem,
    bits: u32,
    deadline: Instant,
    cache: HashMap<Key, Outcome>,
    cache_items: usize,
    adaptive: bool,
    inplace_scan: bool,
    inplace_branch: bool,
    move_components: bool,
    stats: Stats,
}

#[derive(Default)]
struct Stats {
    nodes: usize,
    cache_hits: usize,
    branches: usize,
    splits: usize,
    scan_units: usize,
    scan_clause_visits: usize,
    indexed_attempts: usize,
    indexed_complete: usize,
    indexed_conflicts: usize,
    indexed_units: usize,
    occurrence_touches: usize,
    root_indexed: usize,
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
// Stable compaction preserves the surviving clause and literal order of assign.
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
// The positive child is consumed first; keep the queued negative child
// compact, and donate the owned residual buffers only to the positive child.
fn branch_inplace(mut clauses: Vec<Clause>, lit: i32) -> (Vec<Clause>, Vec<Clause>) {
    let no = assign(&clauses, -lit);
    assign_inplace(&mut clauses, lit);
    (clauses, no)
}
// A normalized clause's index never changes relative to any surviving clause.
// Selecting the smallest live unit index reproduces the old scan order, so
// the weighted prefix keeps the same separately rounded MPFR multiplication.
struct Closure {
    clauses: Vec<Clause>,
    forced: Vec<i32>,
    touches: usize,
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
    let mut touches = 0usize;
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
            touches += 1;
            lengths[j] = usize::MAX;
        }
        for &j in &occurrences[occurrence_slot(-lit)] {
            touches += 1;
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
        touches,
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
fn components(clauses: &mut Vec<Clause>, scope: &[usize], n: usize, move_components: bool) -> Vec<Component> {
    let mut parent: Vec<_> = (0..=n).collect();
    for c in clauses.iter() {
        let first = c[0].unsigned_abs() as usize;
        for &lit in &c[1..] {
            let a = root(&mut parent, first);
            let b = root(&mut parent, lit.unsigned_abs() as usize);
            parent[b] = a;
        }
    }
    let mut groups: BTreeMap<usize, (Vec<usize>, Vec<Clause>)> = BTreeMap::new();
    for &v in scope {
        groups.entry(root(&mut parent, v)).or_default().0.push(v);
    }
    if groups.len() <= 1 {
        return Vec::new();
    }
    // Only consume the residual after proving there is a split. Both arms
    // keep the same DSU representatives, BTreeMap ordering and clause order.
    if move_components {
        for c in std::mem::take(clauses) {
            let r = root(&mut parent, c[0].unsigned_abs() as usize);
            groups.entry(r).or_default().1.push(c);
        }
    } else {
        for c in clauses.iter() {
            let r = root(&mut parent, c[0].unsigned_abs() as usize);
            groups.entry(r).or_default().1.push(c.clone());
        }
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
    fn save(&mut self, key: Key, out: Outcome) -> Result<Outcome, String> {
        if !out.count.re.is_finite() || !out.count.im.is_finite() {
            return Err("nonfinite general".into());
        }
        let items =
            key.scope.len() + key.clauses.len() + key.clauses.iter().map(Vec::len).sum::<usize>();
        // Cache admission is optional: exhaustion never stops search.
        if self.cache.len() < 10_000 && self.cache_items + items <= 400_000 {
            self.cache_items += items;
            self.cache.insert(key, out.clone());
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
        self.stats.nodes += 1;
        let mut residual = normalize(clauses);
        let key = Key {
            scope: scope.clone(),
            clauses: residual.clone(),
        };
        if let Some(hit) = self.cache.get(&key) {
            self.stats.cache_hits += 1;
            return Ok(Step::Done(hit.clone()));
        }
        let mut remaining = scope;
        let mut prefix = C::one(self.bits);
        let mut scanned = 0usize;
        loop {
            self.check()?;
            if residual.iter().any(Vec::is_empty) {
                return self.save(key, Outcome::unsat(self.bits)).map(Step::Done);
            }
            let Some(lit) = residual.iter().find(|c| c.len() == 1).map(|c| c[0]) else {
                break;
            };
            // Do not pay to build an index for the common short closure.
            // Only after two scan assignments and a third live unit, on a
            // sufficiently large residual, switch to ordered occurrence closure.
            if self.adaptive && scanned >= 2 && residual.len() >= 128 {
                self.stats.indexed_attempts += 1;
                self.stats.root_indexed += usize::from(stack.is_empty());
                let Some(closed) = indexed_units(residual, self.p.n, self.deadline)? else {
                    self.stats.indexed_conflicts += 1;
                    return self.save(key, Outcome::unsat(self.bits)).map(Step::Done);
                };
                self.stats.indexed_complete += 1;
                self.stats.indexed_units += closed.forced.len();
                self.stats.occurrence_touches += closed.touches;
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
            self.stats.scan_units += 1;
            self.stats.scan_clause_visits += residual.len();
            let v = lit.unsigned_abs() as usize;
            prefix = prefix.mul(self.weight(v, lit > 0), self.bits);
            remaining.retain(|&x| x != v);
            if self.inplace_scan {
                assign_inplace(&mut residual, lit);
            } else {
                residual = assign(&residual, lit);
            }
        }
        let mut frequency = vec![0_usize; self.p.n + 1];
        for clause in &residual {
            for &lit in clause {
                frequency[lit.unsigned_abs() as usize] += 1;
            }
        }
        let mut active = Vec::new();
        for v in remaining {
            if frequency[v] == 0 {
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
        let mut parts = components(&mut residual, &active, self.p.n, self.move_components).into_iter();
        if let Some((scope, clauses)) = parts.next() {
            self.stats.splits += 1;
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
        let v = *active
            .iter()
            .max_by_key(|&&v| frequency[v])
            .ok_or("no active pivot")?;
        let child: Vec<_> = active.into_iter().filter(|&x| x != v).collect();
        self.stats.branches += 1;
        let (yes, no) = if self.inplace_branch {
            branch_inplace(residual, v as i32)
        } else {
            (assign(&residual, v as i32), assign(&residual, -(v as i32)))
        };
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
    let adaptive = std::env::var("RAWMOCS_RESEARCH_DESCENDANT_UNITS").as_deref() == Ok("1");
    let report = std::env::var("RAWMOCS_RESEARCH_GENERAL_STATS").as_deref() == Ok("1");
    let inplace_scan = std::env::var("RAWMOCS_RESEARCH_INPLACE_SCAN").as_deref() == Ok("1");
    let inplace_branch = std::env::var("RAWMOCS_RESEARCH_INPLACE_BRANCH").as_deref() == Ok("1");
    let move_components = std::env::var("RAWMOCS_RESEARCH_MOVE_COMPONENTS").as_deref() == Ok("1");
    solve_with_options(p, bits, deadline, adaptive, inplace_scan, inplace_branch, move_components, report)
}

#[cfg(test)]
fn solve_with_adaptive(
    p: &Problem,
    bits: u32,
    deadline: Instant,
    adaptive: bool,
    report: bool,
) -> Result<Outcome, String> {
    solve_with_options(p, bits, deadline, adaptive, false, false, false, report)
}

fn solve_with_options(
    p: &Problem,
    bits: u32,
    deadline: Instant,
    adaptive: bool,
    inplace_scan: bool,
    inplace_branch: bool,
    move_components: bool,
    report: bool,
) -> Result<Outcome, String> {
    let mut engine = Engine {
        p,
        bits,
        deadline,
        cache: HashMap::new(),
        cache_items: 0,
        adaptive,
        inplace_scan,
        inplace_branch,
        move_components,
        stats: Stats::default(),
    };
    let result = engine.count(p.clauses.clone(), (1..=p.n).collect());
    if report {
        let s = &engine.stats;
        eprintln!(
            "GENERAL_RESEARCH_DESCENDANT complete={} nodes={} cache_hits={} branches={} splits={} scan_units={} scan_clause_visits={} indexed_attempts={} indexed_complete={} indexed_conflicts={} indexed_units={} occurrence_touches={} root_indexed={}",
            usize::from(result.is_ok()), s.nodes, s.cache_hits, s.branches, s.splits,
            s.scan_units, s.scan_clause_visits, s.indexed_attempts, s.indexed_complete,
            s.indexed_conflicts, s.indexed_units, s.occurrence_touches, s.root_indexed
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_emitted;
    use std::time::Duration;

    fn scan_units(mut clauses: Vec<Clause>) -> Option<(Vec<Clause>, Vec<i32>)> {
        let mut forced = Vec::new();
        loop {
            if clauses.iter().any(Vec::is_empty) {
                return None;
            }
            let Some(lit) = clauses.iter().find(|c| c.len() == 1).map(|c| c[0]) else {
                return Some((clauses, forced));
            };
            forced.push(lit);
            clauses = assign(&clauses, lit);
        }
    }

    #[test]
    fn indexed_root_preserves_scan_order_and_residual() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut seed = 1729_u32;
        for _ in 0..1024 {
            let mut clauses = vec![vec![1]];
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
                let mut compact = clauses.clone();
                assign_inplace(&mut compact, lit);
                assert_eq!(compact, assign(&clauses, lit));
                let (yes, no) = branch_inplace(clauses.clone(), lit);
                assert_eq!(yes, assign(&clauses, lit));
                assert_eq!(no, assign(&clauses, -lit));
            }
            let expected = scan_units(clauses.clone());
            let actual = indexed_units(clauses, 6, deadline)
                .unwrap()
                .map(|c| (c.clauses, c.forced));
            assert_eq!(actual, expected);
        }
        let chain = normalize(
            std::iter::once(vec![1])
                .chain((1..2048).map(|i| vec![-i, i + 1]))
                .collect(),
        );
        let actual = indexed_units(chain, 2048, deadline).unwrap().unwrap();
        assert_eq!(actual.forced, (1..=2048).collect::<Vec<_>>());
        assert!(actual.clauses.is_empty());
        assert_eq!(actual.touches, 4095);
    }

    #[test]
    fn component_move_preserves_order_and_connected_residual() {
        let mut seed = 2187_u32;
        for _ in 0..1024 {
            let mut clauses = Vec::new();
            for _ in 0..12 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let a = ((seed >> 16) % 8 + 1) as i32;
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let b = ((seed >> 16) % 8 + 1) as i32;
                clauses.push(vec![a, if seed & 0x100 == 0 { b } else { -b }]);
            }
            let clauses = normalize(clauses);
            let scope: Vec<usize> = (1..=8).filter(|&v| clauses.iter().any(|c| c.iter().any(|l| l.unsigned_abs() as usize == v))).collect();
            let mut reference = clauses.clone();
            let mut candidate = clauses.clone();
            let expected = components(&mut reference, &scope, 8, false);
            let actual = components(&mut candidate, &scope, 8, true);
            assert_eq!(actual, expected);
            assert_eq!(reference, clauses);
            if expected.is_empty() {
                assert_eq!(candidate, clauses);
            } else {
                assert!(candidate.is_empty());
            }
        }
        let mut clauses = vec![vec![1, 2], vec![-3, 4], vec![5, 6]];
        let parts = components(&mut clauses, &[1,2,3,4,5,6], 6, true);
        assert_eq!(parts.len(), 3);
        assert!(clauses.is_empty());
    }

    #[test]
    fn branch_driven_chain_preserves_signed_complex_counts() {
        // No root units; either pivot branch forces all other variables.
        let n = 512;
        let mut raw = format!("p cnf {n} {}\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight -1 0.25 -0.125 0\n", 2*(n-1));
        for i in 1..n {
            raw.push_str(&format!("-{i} {} 0\n{i} -{} 0\n", i+1, i+1));
        }
        for bits in [100, 200, 333, 432] {
            let p = parse_emitted(&raw, bits).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let old = solve_with_adaptive(&p, bits, deadline, false, false).unwrap();
            let new = solve_with_adaptive(&p, bits, deadline, true, false).unwrap();
            let compact = solve_with_options(&p, bits, deadline, false, true, false, false, false).unwrap();
            assert_eq!(old.sat, compact.sat);
            assert_eq!(old.count.re, compact.count.re);
            assert_eq!(old.count.im, compact.count.im);
            let donated = solve_with_options(&p, bits, deadline, false, true, true, false, false).unwrap();
            assert_eq!(old.sat, donated.sat);
            assert_eq!(old.count.re, donated.count.re);
            assert_eq!(old.count.im, donated.count.im);
            assert_eq!(old.count.re.is_sign_negative(), donated.count.re.is_sign_negative());
            assert_eq!(old.count.im.is_sign_negative(), donated.count.im.is_sign_negative());
            assert!(old.sat && new.sat);
            assert_eq!(old.count.re, new.count.re);
            assert_eq!(old.count.im, new.count.im);
            assert_eq!(new.count.re, -0.25);
            assert_eq!(new.count.im, 0.125);
            assert_eq!(old.count.re.is_sign_negative(), new.count.re.is_sign_negative());
            assert_eq!(old.count.im.is_sign_negative(), new.count.im.is_sign_negative());
        }
    }

    #[test]
    fn indexed_root_preserves_weighted_counts_at_all_precisions() {
        for bits in [100, 200, 333, 432] {
            for raw in [
                "p cnf 4 3\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight 2 0.25 -0.125 0\n1 0\n-1 2 0\n3 4 0\n",
                "p cnf 2 2\nc t wmc\nc p weight 2 -1 0 0\n1 0\n1 2 0\n",
                "p cnf 2 3\nc t wmc\n1 0\n-1 2 0\n-2 0\n",
                "p cnf 6 3\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight -3 0.25 -0.125 0\n1 2 0\n-3 4 0\n5 6 0\n",
            ] {
                let p = parse_emitted(raw, bits).unwrap();
                let deadline = Instant::now() + Duration::from_secs(30);
                let old = solve_with_adaptive(&p, bits, deadline, false, false).unwrap();
                let new = solve_with_adaptive(&p, bits, deadline, true, false).unwrap();
                let compact = solve_with_options(&p, bits, deadline, false, true, false, false, false).unwrap();
                assert_eq!(old.sat, compact.sat);
                assert_eq!(old.count.re, compact.count.re);
                assert_eq!(old.count.im, compact.count.im);
                assert_eq!(old.count.re.is_sign_negative(), compact.count.re.is_sign_negative());
                assert_eq!(old.count.im.is_sign_negative(), compact.count.im.is_sign_negative());
                let donated = solve_with_options(&p, bits, deadline, false, true, true, false, false).unwrap();
                assert_eq!(old.sat, donated.sat);
                assert_eq!(old.count.re, donated.count.re);
                assert_eq!(old.count.im, donated.count.im);
                assert_eq!(old.count.re.is_sign_negative(), donated.count.re.is_sign_negative());
                assert_eq!(old.count.im.is_sign_negative(), donated.count.im.is_sign_negative());
                let moved = solve_with_options(&p, bits, deadline, false, true, true, true, false).unwrap();
                assert_eq!(old.sat, moved.sat);
                assert_eq!(old.count.re, moved.count.re);
                assert_eq!(old.count.im, moved.count.im);
                assert_eq!(old.count.re.is_sign_negative(), moved.count.re.is_sign_negative());
                assert_eq!(old.count.im.is_sign_negative(), moved.count.im.is_sign_negative());
                let exact = crate::reference(&p, raw, bits).unwrap();
                assert_eq!(old.sat, new.sat);
                assert_eq!(old.count.re, new.count.re);
                assert_eq!(old.count.im, new.count.im);
                assert_eq!(exact.sat, new.sat);
                assert_eq!(exact.count.re, new.count.re);
                assert_eq!(exact.count.im, new.count.im);
            }
        }
    }
}
