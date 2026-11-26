//! Research-only primitive; not a solver route.
//! Stable source IDs preserve order within a closure. At a new solver
//! node the current residual must be re-ranked/canonicalized separately.
use super::{assign_inplace, indexed_units, normalize, occurrence_slot};
use crate::Clause;
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, BinaryHeap};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct TrailIndex {
    clauses: Vec<Clause>,
    occurrence: Vec<Vec<usize>>,
    assigned: Vec<i8>,
    live: Vec<usize>,
    satisfied: Vec<usize>,
    trail: Vec<i32>,
    active: BTreeSet<usize>,
    units: BTreeSet<usize>,
    conflicts: BTreeSet<usize>,
    touches: u64,
}

impl TrailIndex {
    // Input must be normalized once, with no duplicated literals or
    // tautologies. Keep stable IDs even when clauses become satisfied.
    fn new(clauses: Vec<Clause>, n: usize) -> Self {
        let mut occurrence = vec![Vec::new(); 2 * n];
        let live: Vec<_> = clauses.iter().map(Vec::len).collect();
        let units = live.iter().enumerate().filter_map(|(i, &k)| (k == 1).then_some(i)).collect();
        let conflicts = live.iter().enumerate().filter_map(|(i, &k)| (k == 0).then_some(i)).collect();
        for (i, clause) in clauses.iter().enumerate() {
            for &lit in clause {
                occurrence[occurrence_slot(lit)].push(i);
            }
        }
        let m = clauses.len();
        let active = (0..m).collect();
        Self {
            clauses, occurrence, assigned: vec![0; n + 1], live,
            satisfied: vec![0; m], trail: Vec::new(), active, units, conflicts, touches: 0,
        }
    }

    fn refresh(&mut self, cid: usize) {
        if self.satisfied[cid] == 0 && self.live[cid] == 1 {
            self.units.insert(cid);
        } else {
            self.units.remove(&cid);
        }
        if self.satisfied[cid] == 0 && self.live[cid] == 0 {
            self.conflicts.insert(cid);
        } else {
            self.conflicts.remove(&cid);
        }
    }

    fn checkpoint(&self) -> usize { self.trail.len() }

    fn apply(&mut self, lit: i32) -> bool {
        let v = lit.unsigned_abs() as usize;
        assert_eq!(self.assigned[v], 0);
        self.assigned[v] = if lit > 0 { 1 } else { -1 };
        self.trail.push(lit);
        // Complete every incidence update even on conflict. This makes
        // rollback exact without depending on a partially updated clause.
        let pos = occurrence_slot(lit);
        for k in 0..self.occurrence[pos].len() {
            let cid = self.occurrence[pos][k];
            self.live[cid] -= 1;
            self.satisfied[cid] += 1;
            if self.satisfied[cid] == 1 { self.active.remove(&cid); }
            self.refresh(cid);
            self.touches += 1;
        }
        let neg = occurrence_slot(-lit);
        for k in 0..self.occurrence[neg].len() {
            let cid = self.occurrence[neg][k];
            self.live[cid] -= 1;
            self.refresh(cid);
            self.touches += 1;
        }
        self.conflicts.is_empty()
    }

    fn rollback(&mut self, checkpoint: usize) {
        assert!(checkpoint <= self.trail.len());
        while self.trail.len() > checkpoint {
            let lit = self.trail.pop().unwrap();
            let pos = occurrence_slot(lit);
            for k in 0..self.occurrence[pos].len() {
                let cid = self.occurrence[pos][k];
                self.live[cid] += 1;
                self.satisfied[cid] -= 1;
                if self.satisfied[cid] == 0 { self.active.insert(cid); }
                self.refresh(cid);
                self.touches += 1;
            }
            let neg = occurrence_slot(-lit);
            for k in 0..self.occurrence[neg].len() {
                let cid = self.occurrence[neg][k];
                self.live[cid] += 1;
                self.refresh(cid);
                self.touches += 1;
            }
            self.assigned[lit.unsigned_abs() as usize] = 0;
        }
    }

    // The caller owns the checkpoint. On deadline or conflict the state
    // remains reversible; never expose a partial count.
    fn propagate(&mut self, deadline: Instant) -> Result<Option<Vec<i32>>, &'static str> {
        let mut forced = Vec::new();
        loop {
            if Instant::now() >= deadline { return Err("deadline"); }
            if !self.conflicts.is_empty() { return Ok(None); }
            let Some(&cid) = self.units.first() else { return Ok(Some(forced)); };
            let lit = *self.clauses[cid].iter().find(|&&l| self.assigned[l.unsigned_abs() as usize] == 0).expect("live unit");
            if !self.apply(lit) { return Ok(None); }
            forced.push(lit);
        }
    }

    fn cmp_reduced(&self, a: usize, b: usize) -> Ordering {
        let live = |lit: &&i32| self.assigned[lit.unsigned_abs() as usize] == 0;
        self.clauses[a].iter().filter(live).cmp(self.clauses[b].iter().filter(live))
    }

    // Only the active frontier is visited, not all original clauses.
    // Sorting is at node entry; never re-sort during ordered propagation.
    fn canonical_ids(&self) -> Vec<usize> {
        let mut ids: Vec<_> = self.active.iter().copied().collect();
        ids.sort_unstable_by(|&a, &b| self.cmp_reduced(a, b).then(a.cmp(&b)));
        ids.dedup_by(|a, b| self.cmp_reduced(*a, *b) == Ordering::Equal);
        ids
    }

    fn materialize_ids(&self, ids: &[usize]) -> Vec<Clause> {
        ids.iter().copied().filter(|&cid| self.satisfied[cid] == 0)
            .map(|cid| self.clauses[cid].iter().copied().filter(|l| self.assigned[l.unsigned_abs() as usize] == 0).collect()).collect()
    }

    // Re-rank current reduced clauses once, including duplicate collapse.
    // A representative and its omitted duplicates stay equivalent under
    // subsequent monotone assignments. All source counters still update.
    fn ranked_closure(&mut self, ids: &[usize], deadline: Instant) -> Result<Option<Vec<i32>>, &'static str> {
        if Instant::now() >= deadline { return Err("deadline"); }
        if !self.conflicts.is_empty() { return Ok(None); }
        let mut rank = vec![usize::MAX; self.clauses.len()];
        let mut units = BinaryHeap::new();
        for (i, &cid) in ids.iter().enumerate() {
            rank[cid] = i;
            if self.live[cid] == 1 { units.push(Reverse(i)); }
        }
        let mut forced = Vec::new();
        while let Some(Reverse(i)) = units.pop() {
            if Instant::now() >= deadline { return Err("deadline"); }
            let cid = ids[i];
            if self.satisfied[cid] != 0 || self.live[cid] != 1 { continue; }
            let lit = *self.clauses[cid].iter().find(|&&l| self.assigned[l.unsigned_abs() as usize] == 0).expect("live unit");
            if !self.apply(lit) { return Ok(None); }
            forced.push(lit);
            for &j in &self.occurrence[occurrence_slot(-lit)] {
                if rank[j] != usize::MAX && self.satisfied[j] == 0 && self.live[j] == 1 {
                    units.push(Reverse(rank[j]));
                }
            }
        }
        Ok(Some(forced))
    }

    fn residual(&self) -> Vec<Clause> {
        self.clauses.iter().enumerate().filter(|(cid, _)| self.satisfied[*cid] == 0)
            .map(|(_, c)| c.iter().copied().filter(|l| self.assigned[l.unsigned_abs() as usize] == 0).collect()).collect()
    }

    fn verify(&self) {
        let mut units = BTreeSet::new();
        let mut conflicts = BTreeSet::new();
        for (i, c) in self.clauses.iter().enumerate() {
            let live = c.iter().filter(|l| self.assigned[l.unsigned_abs() as usize] == 0).count();
            let sat = c.iter().filter(|&&l| {
                self.assigned[l.unsigned_abs() as usize] == if l > 0 { 1 } else { -1 }
            }).count();
            assert_eq!((self.live[i], self.satisfied[i]), (live, sat));
            if sat == 0 && live == 1 { units.insert(i); }
            if sat == 0 && live == 0 { conflicts.insert(i); }
        }
        assert_eq!(self.active, (0..self.clauses.len()).filter(|&i| self.satisfied[i] == 0).collect());
        assert_eq!(self.units, units);
        assert_eq!(self.conflicts, conflicts);
    }
}

fn scan(mut clauses: Vec<Clause>) -> Option<(Vec<Clause>, Vec<i32>)> {
    let mut forced = Vec::new();
    loop {
        if clauses.iter().any(Vec::is_empty) { return None; }
        let Some(lit) = clauses.iter().find(|c| c.len() == 1).map(|c| c[0]) else {
            return Some((clauses, forced));
        };
        forced.push(lit);
        assign_inplace(&mut clauses, lit);
    }
}

#[test]
fn reversible_index_random_nested_rollback() {
    let mut seed = 0x12345678_u32;
    let mut next = || { seed = seed.wrapping_mul(1664525).wrapping_add(1013904223); seed >> 8 };
    let deadline = Instant::now() + Duration::from_secs(30);
    for case in 0..2048 {
        let mut source = Vec::new();
        for _ in 0..18 {
            let mut c = Vec::new();
            for _ in 0..(if case % 4 == 0 { next() % 5 } else { 2 + next() % 3 }) {
                let v = (next() % 12 + 1) as i32;
                c.push(if next() % 2 == 0 { v } else { -v });
            }
            source.push(c);
        }
        let source = normalize(source);
        let mut state = TrailIndex::new(source.clone(), 12);
        for _ in 0..12 {
            let parent = state.checkpoint();
            let before = state.residual();
            // Alternate branches with nested assignments, including
            // contradictory parents. Roll back every closure separately.
            if let Some(v) = (1..=12).find(|&v| state.assigned[v] == 0) {
                for sign in [1, -1] {
                    let lit = sign * v as i32;
                    state.apply(lit);
                    state.verify();
                    let after = state.residual();
                    let expected = scan(after.clone());
                    let indexed = indexed_units(after, 12, deadline).unwrap().map(|x| (x.clauses, x.forced));
                    assert_eq!(indexed, expected);
                    let child = state.checkpoint();
                    let actual = state.propagate(deadline).unwrap().map(|forced| (state.residual(), forced));
                    assert_eq!(actual, expected);
                    state.verify();
                    state.rollback(child);
                    state.verify();
                    let canonical = normalize(state.residual());
                    let ids = state.canonical_ids();
                    assert_eq!(state.materialize_ids(&ids), canonical);
                    let expected = scan(canonical);
                    let actual = state.ranked_closure(&ids, deadline).unwrap()
                        .map(|forced| (state.materialize_ids(&ids), forced));
                    assert_eq!(actual, expected);
                    state.verify();
                    state.rollback(child);
                    state.verify();
                    state.rollback(parent);
                    state.verify();
                    assert_eq!(state.residual(), before);
                }
                state.apply(if next() % 2 == 0 { v as i32 } else { -(v as i32) });
            }
        }
        state.rollback(0);
        state.verify();
        assert_eq!(state.residual(), source);
        assert!(state.assigned.iter().all(|&x| x == 0));
    }
}

#[test]
fn reversible_index_deadline_and_conflict_restore() {
    let source = normalize(vec![vec![1], vec![-1, 2], vec![-2], vec![1, 3], vec![-1, -3]]);
    let mut state = TrailIndex::new(source.clone(), 3);
    assert_eq!(state.propagate(Instant::now()), Err("deadline"));
    state.verify();
    let deadline = Instant::now() + Duration::from_secs(2);
    assert_eq!(state.propagate(deadline).unwrap(), None);
    state.verify();
    state.rollback(0);
    state.verify();
    assert_eq!(state.residual(), source);
}

#[test]
fn reversible_index_complex_weight_order() {
    use crate::{parse_emitted, C};
    let raw = "p cnf 5 4\nc t wmc\nc p weight 1 0.13 -0.27 0\nc p weight 2 -0.29 0.31 0\nc p weight 3 0.37 0.41 0\nc p weight -4 -0.43 0.47 0\n1 0\n-1 2 0\n-2 3 0\n-3 -4 0\n";
    let deadline = Instant::now() + Duration::from_secs(2);
    for bits in [100, 200, 333, 432] {
        let p = parse_emitted(raw, bits).unwrap();
        let source = normalize(p.clauses.clone());
        let expected = scan(source.clone()).unwrap();
        let mut state = TrailIndex::new(source.clone(), p.n);
        let forced = state.propagate(deadline).unwrap().unwrap();
        assert_eq!(forced, expected.1);
        let product = |seq: &[i32]| {
            let mut count = C::one(bits);
            for &lit in seq {
                let w = &p.weights[lit.unsigned_abs() as usize - 1];
                count = count.mul(if lit > 0 { &w.positive } else { &w.negative }, bits);
            }
            count
        };
        let a = product(&forced);
        let b = product(&expected.1);
        assert_eq!(a.re, b.re);
        assert_eq!(a.im, b.im);
        state.rollback(0);
        state.verify();
        assert_eq!(state.residual(), source);
    }
}

#[test]
#[ignore = "research-only fixed-work native diagnostic"]
fn reversible_index_fixed_work() {
    let n = 8192_i32;
    // Both pivot polarities force the same chain. Stable source order
    // only; this diagnostic does not model re-ranking at DPLL nodes.
    let mut clauses = Vec::new();
    for v in 2..=9 {
        clauses.push(vec![1, v]);
        clauses.push(vec![-1, v]);
    }
    for v in 2..n { clauses.push(vec![-v, v + 1]); }
    let clauses = normalize(clauses);
    let deadline = Instant::now() + Duration::from_secs(60);
    let rounds = 32;
    let expected: Vec<_> = [1, -1].iter().map(|&lit| {
        let mut copy = clauses.clone();
        assign_inplace(&mut copy, lit);
        let closed = indexed_units(copy, n as usize, deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    let mut expected_forced = 0;
    for (trial, reuse) in [false, true, true, false].into_iter().enumerate() {
        let start = Instant::now();
        let mut state = reuse.then(|| TrailIndex::new(clauses.clone(), n as usize));
        let mut forced_count = 0;
        for _ in 0..rounds {
            for (arm, lit) in [1, -1].into_iter().enumerate() {
                let forced = if let Some(state) = state.as_mut() {
                    assert!(state.apply(lit));
                    let forced = state.propagate(deadline).unwrap().unwrap();
                    assert!(state.units.is_empty() && state.conflicts.is_empty());
                    assert!(state.satisfied.iter().all(|&x| x > 0));
                    state.rollback(0);
                    forced
                } else {
                    let mut copy = clauses.clone();
                    assign_inplace(&mut copy, lit);
                    let closed = indexed_units(copy, n as usize, deadline).unwrap().unwrap();
                    assert!(closed.clauses.is_empty());
                    closed.forced
                };
                assert_eq!(forced, expected[arm]);
                assert_eq!(forced.len(), (n - 1) as usize);
                assert!(forced.iter().all(|&v| v > 1));
                forced_count += forced.len();
            }
        }
        if trial == 0 { expected_forced = forced_count; }
        assert_eq!(forced_count, expected_forced);
        let elapsed = start.elapsed().as_nanos();
        if let Some(state) = state.as_ref() { state.verify(); assert!(state.trail.is_empty()); }
        println!("TRAIL_FIXED_WORK trial={trial} reuse={reuse} branches={} forced={forced_count} wall_ns={elapsed} touches={}", 2 * rounds, state.as_ref().map_or(0, |s| s.touches));
    }
}

#[test]
fn canonical_rank_changes_after_assignment_and_dedup() {
    // [-3, 2] becomes [2]; [1] remains first after canonical
    // re-ranking, even though [-3, 2] had the earlier source ID.
    let source = normalize(vec![vec![-3, 2], vec![1], vec![2], vec![-1, 4], vec![-2, 5]]);
    let mut state = TrailIndex::new(source.clone(), 5);
    let deadline = Instant::now() + Duration::from_secs(2);
    assert!(state.apply(3));
    let checkpoint = state.checkpoint();
    let ids = state.canonical_ids();
    let canonical = normalize(state.residual());
    assert_eq!(state.materialize_ids(&ids), canonical);
    assert_eq!(ids.len(), 4);
    let expected = scan(canonical).unwrap();
    let actual = state.ranked_closure(&ids, deadline).unwrap().unwrap();
    assert_eq!(actual, expected.1);
    assert_eq!(actual[0], 1);
    assert_eq!(state.materialize_ids(&ids), expected.0);
    state.rollback(checkpoint);
    state.verify();
    state.rollback(0);
    assert_eq!(state.residual(), source);
}

#[test]
#[ignore = "research-only canonical boundary diagnostic"]
fn reversible_canonical_fixed_work() {
    let n = 8192_i32;
    let mut source = Vec::new();
    for v in 2..=9 { source.push(vec![1,v]); source.push(vec![-1,v]); }
    for v in 2..n { source.push(vec![-v,v+1]); }
    let source = normalize(source);
    let deadline = Instant::now() + Duration::from_secs(60);
    let expected: Vec<_> = [1,-1].iter().map(|&lit| {
        let mut c = source.clone();
        assign_inplace(&mut c,lit);
        let c = normalize(c);
        let closed = indexed_units(c,n as usize,deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    for (trial,reuse) in [false,true,true,false].into_iter().enumerate() {
        let start = Instant::now();
        let mut state = reuse.then(|| TrailIndex::new(source.clone(),n as usize));
        let mut boundary_ns = 0_u128;
        let mut closure_ns = 0_u128;
        let mut work = 0;
        for _ in 0..32 {
            for (arm,lit) in [1,-1].into_iter().enumerate() {
                let boundary = Instant::now();
                let forced = if let Some(st) = state.as_mut() {
                    assert!(st.apply(lit));
                    let ids = st.canonical_ids();
                    // Include exact canonical key materialization, even
                    // though this diagnostic does not run a cache lookup.
                    let key = st.materialize_ids(&ids);
                    boundary_ns += boundary.elapsed().as_nanos();
                    let closure = Instant::now();
                    let forced = st.ranked_closure(&ids,deadline).unwrap().unwrap();
                    assert!(st.active.is_empty());
                    st.rollback(0);
                    closure_ns += closure.elapsed().as_nanos();
                    std::hint::black_box(key);
                    forced
                } else {
                    let mut clauses = source.clone();
                    assign_inplace(&mut clauses,lit);
                    let clauses = normalize(clauses);
                    // Both arms include key materialization and keep
                    // the key alive through closure, not hashing.
                    let key = clauses.clone();
                    boundary_ns += boundary.elapsed().as_nanos();
                    let closure = Instant::now();
                    let closed = indexed_units(clauses,n as usize,deadline).unwrap().unwrap();
                    assert!(closed.clauses.is_empty());
                    closure_ns += closure.elapsed().as_nanos();
                    std::hint::black_box(key);
                    closed.forced
                };
                assert_eq!(forced,expected[arm]);
                work += forced.len();
            }
        }
        let wall_ns = start.elapsed().as_nanos();
        if let Some(st) = state.as_ref() { st.verify(); assert!(st.trail.is_empty()); }
        println!("CANONICAL_FIXED_WORK trial={trial} reuse={reuse} branches=64 forced={work} wall_ns={wall_ns} boundary_ns={boundary_ns} closure_ns={closure_ns}");
    }
}
