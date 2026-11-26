//! Research-only primitive; not a solver route.
//! Stable source IDs preserve order within a closure. At a new solver
//! node the current residual must be re-ranked/canonicalized separately.
use super::{assign_inplace, indexed_units, normalize, occurrence_slot};
use crate::Clause;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct TrailIndex {
    clauses: Vec<Clause>,
    occurrence: Vec<Vec<usize>>,
    assigned: Vec<i8>,
    live: Vec<usize>,
    satisfied: Vec<usize>,
    trail: Vec<i32>,
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
        Self {
            clauses, occurrence, assigned: vec![0; n + 1], live,
            satisfied: vec![0; m], trail: Vec::new(), units, conflicts, touches: 0,
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
    for _ in 0..2048 {
        let mut source = Vec::new();
        for _ in 0..18 {
            let mut c = Vec::new();
            for _ in 0..(next() % 5) {
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
    let mut expected_forced = 0;
    for (trial, reuse) in [false, true, true, false].into_iter().enumerate() {
        let start = Instant::now();
        let mut state = reuse.then(|| TrailIndex::new(clauses.clone(), n as usize));
        let mut forced_count = 0;
        for _ in 0..rounds {
            for lit in [1, -1] {
                let mut copy = Vec::new();
                let forced = if let Some(state) = state.as_mut() {
                    assert!(state.apply(lit));
                    let forced = state.propagate(deadline).unwrap().unwrap();
                    assert!(state.units.is_empty() && state.conflicts.is_empty());
                    assert!(state.satisfied.iter().all(|&x| x > 0));
                    state.rollback(0);
                    forced
                } else {
                    copy.clone_from(&clauses);
                    assign_inplace(&mut copy, lit);
                    let closed = indexed_units(copy, n as usize, deadline).unwrap().unwrap();
                    assert!(closed.clauses.is_empty());
                    closed.forced
                };
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
