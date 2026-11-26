//! Research-only primitive; not a solver route.
//! Stable source IDs preserve order within a closure. At a new solver
//! node the current residual must be re-ranked/canonicalized separately.
use super::{assign_inplace, indexed_units, normalize, occurrence_slot};
use crate::Clause;
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, BinaryHeap};
use std::time::{Duration, Instant};

// Sparse-set frontier: O(1) membership, insertion and swap removal.
// Canonical ordering is restored explicitly by canonical_ids; dense
// storage order is NOT the unit propagation order or cache identity.
#[derive(Debug)]
enum Frontier {
    Tree(BTreeSet<usize>),
    Dense { ids: Vec<usize>, positions: Vec<usize> },
    Bitmap { words: Vec<u64>, count: usize },
}
impl Frontier {
    fn new(m: usize, dense: bool) -> Self {
        if dense {
            Self::Dense { ids: (0..m).collect(), positions: (0..m).collect() }
        } else {
            Self::Tree((0..m).collect())
        }
    }
    fn bitmap(m: usize) -> Self {
        let mut words = vec![u64::MAX; m.div_ceil(64)];
        if m % 64 != 0 { *words.last_mut().unwrap() = (1_u64 << (m % 64)) - 1; }
        Self::Bitmap { words, count: m }
    }
    fn insert(&mut self, cid: usize) {
        match self {
            Self::Tree(set) => { set.insert(cid); }
            Self::Bitmap { words, count } => {
                let bit = 1_u64 << (cid % 64);
                assert_eq!(words[cid / 64] & bit, 0);
                words[cid / 64] |= bit;
                *count += 1;
            }
            Self::Dense { ids, positions } => {
                assert_eq!(positions[cid], usize::MAX);
                positions[cid] = ids.len();
                ids.push(cid);
            }
        }
    }
    fn remove(&mut self, cid: usize) {
        match self {
            Self::Tree(set) => { set.remove(&cid); }
            Self::Bitmap { words, count } => {
                let bit = 1_u64 << (cid % 64);
                assert_ne!(words[cid / 64] & bit, 0);
                words[cid / 64] &= !bit;
                *count -= 1;
            }
            Self::Dense { ids, positions } => {
                let pos = positions[cid];
                assert_ne!(pos, usize::MAX);
                assert_eq!(ids.swap_remove(pos), cid);
                if pos < ids.len() { positions[ids[pos]] = pos; }
                positions[cid] = usize::MAX;
            }
        }
    }
    fn ids(&self) -> Vec<usize> {
        match self {
            Self::Tree(set) => set.iter().copied().collect(),
            Self::Dense { ids, .. } => ids.clone(),
            Self::Bitmap { words, count } => {
                let mut ids = Vec::with_capacity(*count);
                for (i, &word) in words.iter().enumerate() {
                    let mut bits = word;
                    while bits != 0 {
                        ids.push(i * 64 + bits.trailing_zeros() as usize);
                        bits &= bits - 1;
                    }
                }
                ids
            }
        }
    }
    fn is_empty(&self) -> bool {
        match self {
            Self::Tree(set) => set.is_empty(),
            Self::Dense { ids, .. } => ids.is_empty(),
            Self::Bitmap { count, .. } => *count == 0,
        }
    }
    fn verify(&self, satisfied: &[usize]) {
        if let Self::Bitmap { count, .. } = self { assert_eq!(*count, self.ids().len()); }
        let mut ids = self.ids();
        ids.sort_unstable();
        assert_eq!(ids, (0..satisfied.len()).filter(|&i| satisfied[i] == 0).collect::<Vec<_>>());
        if let Self::Dense { ids, positions } = self {
            for (i, &cid) in ids.iter().enumerate() { assert_eq!(positions[cid], i); }
            for (cid, &p) in positions.iter().enumerate() {
                if satisfied[cid] != 0 { assert_eq!(p, usize::MAX); }
            }
        }
    }
}

#[derive(Debug)]
struct RankScratch {
    ranks: Vec<usize>,
    stamps: Vec<u32>,
    epoch: u32,
}
impl RankScratch {
    fn new(m: usize) -> Self {
        Self { ranks: vec![0; m], stamps: vec![0; m], epoch: 0 }
    }
    fn begin(&mut self) {
        if self.epoch == u32::MAX {
            self.stamps.fill(0);
            self.epoch = 1;
        } else {
            self.epoch += 1;
        }
    }
    fn set(&mut self, cid: usize, rank: usize) {
        self.ranks[cid] = rank;
        self.stamps[cid] = self.epoch;
    }
    fn get(&self, cid: usize) -> usize {
        if self.stamps[cid] == self.epoch { self.ranks[cid] } else { usize::MAX }
    }
}

#[derive(Debug)]
struct TrailIndex {
    clauses: Vec<Clause>,
    occurrence: Vec<Vec<usize>>,
    assigned: Vec<i8>,
    live: Vec<usize>,
    satisfied: Vec<usize>,
    trail: Vec<i32>,
    active: Frontier,
    transitions: bool,
    units: BTreeSet<usize>,
    conflicts: BTreeSet<usize>,
    touches: u64,
    rank_scratch: Option<RankScratch>,
    maintain_source_units: bool,
    fuse_ranked_updates: bool,
}

impl TrailIndex {
    // Input must be normalized once, with no duplicated literals or
    // tautologies. Keep stable IDs even when clauses become satisfied.
    fn new(clauses: Vec<Clause>, n: usize) -> Self {
        Self::with_mode(clauses, n, false, false)
    }
    fn with_mode(clauses: Vec<Clause>, n: usize, dense: bool, transitions: bool) -> Self {
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
        let active = Frontier::new(m, dense);
        Self {
            clauses, occurrence, assigned: vec![0; n + 1], live,
            satisfied: vec![0; m], trail: Vec::new(), active, transitions, units, conflicts, touches: 0, rank_scratch: None, maintain_source_units: true, fuse_ranked_updates: false,
        }
    }

    // Ranked closure has its own queue in canonical rank order. It
    // does not need the source-ID queue, which is only a test oracle.
    fn ranked_only(&mut self, enabled: bool) {
        assert!(self.trail.is_empty());
        self.maintain_source_units = !enabled;
        self.units.clear();
        if !enabled {
            self.units.extend(self.live.iter().enumerate()
                .filter_map(|(i, &k)| (k == 1 && self.satisfied[i] == 0).then_some(i)));
        }
    }

    fn class(&self, cid: usize) -> u8 {
        if self.satisfied[cid] != 0 { 2 }
        else if self.live[cid] == 0 { 0 }
        else if self.live[cid] == 1 { 1 }
        else { 2 }
    }

    fn refresh(&mut self, cid: usize, before: u8) {
        let after = self.class(cid);
        if self.transitions {
            if before == after { return; }
            match before {
                0 => { self.conflicts.remove(&cid); }
                1 if self.maintain_source_units => { self.units.remove(&cid); }
                _ => {}
            }
            match after {
                0 => { self.conflicts.insert(cid); }
                1 if self.maintain_source_units => { self.units.insert(cid); }
                _ => {}
            }
            return;
        }
        if self.maintain_source_units {
            if self.satisfied[cid] == 0 && self.live[cid] == 1 {
                self.units.insert(cid);
            } else {
                self.units.remove(&cid);
            }
        }
        if self.satisfied[cid] == 0 && self.live[cid] == 0 {
            self.conflicts.insert(cid);
        } else {
            self.conflicts.remove(&cid);
        }
    }

    fn checkpoint(&self) -> usize { self.trail.len() }

    fn apply(&mut self, lit: i32) -> bool {
        self.apply_with_units(lit, |_| {})
    }

    // Emit newly live units in the same negative-incidence order as
    // the separate ranked scan. Finish every update on conflict.
    fn apply_with_units(&mut self, lit: i32, mut on_unit: impl FnMut(usize)) -> bool {
        let v = lit.unsigned_abs() as usize;
        assert_eq!(self.assigned[v], 0);
        self.assigned[v] = if lit > 0 { 1 } else { -1 };
        self.trail.push(lit);
        // Complete every incidence update even on conflict. This makes
        // rollback exact without depending on a partially updated clause.
        let pos = occurrence_slot(lit);
        for k in 0..self.occurrence[pos].len() {
            let cid = self.occurrence[pos][k];
            let before = self.class(cid);
            self.live[cid] -= 1;
            self.satisfied[cid] += 1;
            if self.satisfied[cid] == 1 { self.active.remove(cid); }
            self.refresh(cid, before);
            self.touches += 1;
        }
        let neg = occurrence_slot(-lit);
        for k in 0..self.occurrence[neg].len() {
            let cid = self.occurrence[neg][k];
            let before = self.class(cid);
            self.live[cid] -= 1;
            self.refresh(cid, before);
            if self.satisfied[cid] == 0 && self.live[cid] == 1 {
                on_unit(cid);
            }
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
            let before = self.class(cid);
                self.live[cid] += 1;
                self.satisfied[cid] -= 1;
                if self.satisfied[cid] == 0 { self.active.insert(cid); }
                self.refresh(cid, before);
                self.touches += 1;
            }
            let neg = occurrence_slot(-lit);
            for k in 0..self.occurrence[neg].len() {
                let cid = self.occurrence[neg][k];
            let before = self.class(cid);
                self.live[cid] += 1;
                self.refresh(cid, before);
                self.touches += 1;
            }
            self.assigned[lit.unsigned_abs() as usize] = 0;
        }
    }

    // The caller owns the checkpoint. On deadline or conflict the state
    // remains reversible; never expose a partial count.
    fn propagate(&mut self, deadline: Instant) -> Result<Option<Vec<i32>>, &'static str> {
        assert!(self.maintain_source_units, "source-order oracle disabled");
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
        let mut ids = self.active.ids();
        ids.sort_unstable_by(|&a, &b| self.cmp_reduced(a, b).then(a.cmp(&b)));
        ids.dedup_by(|a, b| self.cmp_reduced(*a, *b) == Ordering::Equal);
        ids
    }

    // Reduce each active clause once instead of filtering its literals
    // on every sort comparison. Lexicographic order and stable ID tie
    // are identical to canonical_ids; keep one representative per key.
    fn materialized_boundary(&self) -> (Vec<usize>, Vec<Clause>) {
        let mut entries: Vec<_> = self.active.ids().into_iter().map(|cid| {
            let clause: Clause = self.clauses[cid].iter().copied()
                .filter(|l| self.assigned[l.unsigned_abs() as usize] == 0).collect();
            (clause, cid)
        }).collect();
        entries.sort_unstable();
        entries.dedup_by(|a,b| a.0 == b.0);
        let (key, ids) = entries.into_iter().unzip();
        (ids, key)
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
        // Temporarily move scratch out so apply can mutate the trail.
        // Restore it on success, conflict and deadline alike.
        let mut scratch = self.rank_scratch.take();
        let result = self.ranked_closure_impl(ids, deadline, scratch.as_mut());
        self.rank_scratch = scratch;
        result
    }

    fn ranked_closure_impl(&mut self, ids: &[usize], deadline: Instant, mut scratch: Option<&mut RankScratch>) -> Result<Option<Vec<i32>>, &'static str> {
        let mut rank = if scratch.is_some() { Vec::new() } else { vec![usize::MAX; self.clauses.len()] };
        if let Some(s) = scratch.as_mut() { s.begin(); }
        let mut units = BinaryHeap::new();
        for (i, &cid) in ids.iter().enumerate() {
            if let Some(s) = scratch.as_mut() { s.set(cid, i); } else { rank[cid] = i; }
            if self.live[cid] == 1 { units.push(Reverse(i)); }
        }
        let mut forced = Vec::new();
        while let Some(Reverse(i)) = units.pop() {
            if Instant::now() >= deadline { return Err("deadline"); }
            let cid = ids[i];
            if self.satisfied[cid] != 0 || self.live[cid] != 1 { continue; }
            let lit = *self.clauses[cid].iter().find(|&&l| self.assigned[l.unsigned_abs() as usize] == 0).expect("live unit");
            if self.fuse_ranked_updates {
                if !self.apply_with_units(lit, |j| {
                    let r = if let Some(s) = scratch.as_ref() { s.get(j) } else { rank[j] };
                    if r != usize::MAX { units.push(Reverse(r)); }
                }) { return Ok(None); }
            } else {
                if !self.apply(lit) { return Ok(None); }
                for &j in &self.occurrence[occurrence_slot(-lit)] {
                    let r = if let Some(s) = scratch.as_ref() { s.get(j) } else { rank[j] };
                    if r != usize::MAX && self.satisfied[j] == 0 && self.live[j] == 1 {
                        units.push(Reverse(r));
                    }
                }
            }
            forced.push(lit);
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
        self.active.verify(&self.satisfied);
        if self.maintain_source_units { assert_eq!(self.units, units); }
        else { assert!(self.units.is_empty()); }
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
        let mut state = TrailIndex::with_mode(source.clone(), 12, (case / 4) % 2 == 0, (case / 8) % 2 == 0);
        if (case / 16) % 2 == 0 { state.active = Frontier::bitmap(source.len()); }
        if (case / 32) % 2 == 0 { state.rank_scratch = Some(RankScratch::new(source.len())); }
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
                    assert_eq!(state.materialized_boundary(), (ids.clone(), canonical.clone()));
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

#[test]
fn frontier_modes_exact_nested_parity() {
    let source = normalize(vec![vec![-3,2],vec![1],vec![2],vec![-1,4],vec![-2,5],vec![3,4],vec![-4,5]]);
    let deadline = Instant::now() + Duration::from_secs(2);
    for dense in [false,true] {
        for transitions in [false,true] {
            let mut st = TrailIndex::with_mode(source.clone(),5,dense,transitions);
            for lit in [3,-3,4,-4] {
                st.apply(lit);
                let checkpoint = st.checkpoint();
                let canonical = normalize(st.residual());
                let ids = st.canonical_ids();
                assert_eq!(st.materialize_ids(&ids),canonical);
                let expected = scan(canonical);
                let actual = st.ranked_closure(&ids,deadline).unwrap().map(|f| (st.materialize_ids(&ids),f));
                assert_eq!(actual,expected);
                st.verify();
                st.rollback(checkpoint);
                st.verify();
                st.rollback(0);
                st.verify();
                assert_eq!(st.residual(),source);
            }
        }
    }
}

#[test]
#[ignore = "research-only balanced frontier/transition ablation"]
fn frontier_transition_fixed_work() {
    let n = 8192_i32;
    let mut source = Vec::new();
    for v in 2..=9 { source.push(vec![1,v]); source.push(vec![-1,v]); }
    for v in 2..n { source.push(vec![-v,v+1]); }
    let source = normalize(source);
    let deadline = Instant::now() + Duration::from_secs(60);
    let expected: Vec<_> = [1,-1].iter().map(|&lit| {
        let mut c = source.clone();
        assign_inplace(&mut c,lit);
        let closed = indexed_units(normalize(c),n as usize,deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    // Balanced cyclic and reverse orders. Four combinations of factors,
    // plus per-branch rebuild as an end-to-end diagnostic control.
    let orders = [
        [0,1,2,3,4], [1,2,3,4,0], [2,3,4,0,1], [3,4,0,1,2], [4,0,1,2,3],
        [4,3,2,1,0], [0,4,3,2,1], [1,0,4,3,2], [2,1,0,4,3], [3,2,1,0,4],
    ];
    for (trial,order) in orders.into_iter().enumerate() {
        for mode in order {
            let dense = mode == 2 || mode == 4;
            let transitions = mode == 3 || mode == 4;
            let start = Instant::now();
            let mut state = (mode != 0).then(|| TrailIndex::with_mode(source.clone(),n as usize,dense,transitions));
            let mut boundary_ns = 0_u128;
            let mut closure_ns = 0_u128;
            let mut work = 0;
            for _ in 0..32 {
                for (arm,lit) in [1,-1].into_iter().enumerate() {
                    let boundary = Instant::now();
                    let forced = if let Some(st) = state.as_mut() {
                        assert!(st.apply(lit));
                        let ids = st.canonical_ids();
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
            println!("FRONTIER_FIXED_WORK trial={trial} mode={mode} branches=64 forced={work} wall_ns={wall_ns} boundary_ns={boundary_ns} closure_ns={closure_ns}");
        }
    }
}

#[test]
#[ignore = "research-only ordered frontier ablation"]
fn ordered_frontier_fixed_work() {
    let n = 8192_i32;
    let mut source = Vec::new();
    for v in 2..=9 { source.push(vec![1,v]); source.push(vec![-1,v]); }
    for v in 2..n { source.push(vec![-v,v+1]); }
    let source = normalize(source);
    let deadline = Instant::now() + Duration::from_secs(60);
    let expected: Vec<_> = [1,-1].iter().map(|&lit| {
        let mut c = source.clone();
        assign_inplace(&mut c,lit);
        let closed = indexed_units(normalize(c),n as usize,deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    // Balanced cyclic and reverse orders. Four combinations of factors,
    // plus per-branch rebuild as an end-to-end diagnostic control.
    let orders = [
        [0,1,2,3,4], [1,2,3,4,0], [2,3,4,0,1], [3,4,0,1,2], [4,0,1,2,3],
        [4,3,2,1,0], [0,4,3,2,1], [1,0,4,3,2], [2,1,0,4,3], [3,2,1,0,4],
    ];
    for (trial,order) in orders.into_iter().enumerate() {
        for mode in order {
            let dense = mode == 1 || mode == 3;
            let transitions = true;
            let start = Instant::now();
            let mut state = (mode != 0).then(|| TrailIndex::with_mode(source.clone(),n as usize,dense,transitions));
            if mode == 2 || mode == 4 { state.as_mut().unwrap().active = Frontier::bitmap(source.len()); }
            let mut boundary_ns = 0_u128;
            let mut closure_ns = 0_u128;
            let mut work = 0;
            for _ in 0..32 {
                for (arm,lit) in [1,-1].into_iter().enumerate() {
                    let boundary = Instant::now();
                    let forced = if let Some(st) = state.as_mut() {
                        assert!(st.apply(lit));
                        let (ids, key) = if mode >= 3 { st.materialized_boundary() } else {
                            let ids = st.canonical_ids();
                            let key = st.materialize_ids(&ids);
                            (ids,key)
                        };
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
            println!("ORDERED_FRONTIER_FIXED_WORK trial={trial} mode={mode} branches=64 forced={work} wall_ns={wall_ns} boundary_ns={boundary_ns} closure_ns={closure_ns}");
        }
    }
}

#[test]
fn bitmap_frontier_partial_words_and_restore() {
    for m in [0,1,63,64,65,129,8193] {
        let mut f = Frontier::bitmap(m);
        let mut sat = vec![0; m];
        f.verify(&sat);
        for cid in (0..m).step_by(3) { f.remove(cid); sat[cid]=1; }
        f.verify(&sat);
        for cid in (0..m).step_by(3).rev() { f.insert(cid); sat[cid]=0; }
        f.verify(&sat);
    }
}

#[test]
fn rank_generation_wrap_and_stale_ids() {
    let mut r = RankScratch::new(129);
    r.begin();
    r.set(64, 3);
    assert_eq!(r.get(64), 3);
    r.begin();
    assert_eq!(r.get(64), usize::MAX);
    r.set(128, 2);
    r.epoch = u32::MAX;
    r.begin();
    assert_eq!(r.get(64), usize::MAX);
    assert_eq!(r.get(128), usize::MAX);
    r.set(1, 0);
    assert_eq!(r.get(1), 0);
}

#[test]
#[ignore = "research-only late-depth rank generation diagnostic"]
fn rank_generation_fixed_work() {
    // Large inactive prefix, and a small live chain. Set up the
    // parent once; do not charge repeated root construction to either arm.
    let n = 65536_i32;
    let mut source = Vec::new();
    for v in 2..=9 { source.push(vec![1,v]); source.push(vec![-1,v]); }
    for v in 2..512 { source.push(vec![-v,v+1]); }
    for v in 513..=n { source.push(vec![v]); }
    let source = normalize(source);
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut parent = source.clone();
    parent.retain(|c| !(c.len()==1 && c[0]>512));
    let expected: Vec<_> = [1,-1].iter().map(|&lit| {
        let mut c = parent.clone();
        assign_inplace(&mut c,lit);
        let closed = indexed_units(normalize(c),n as usize,deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    for (trial, reuse) in [false,true,true,false,false,true,true,false,false,true,true,false].into_iter().enumerate() {
        let mut state = TrailIndex::with_mode(source.clone(),n as usize,false,true);
        state.active = Frontier::bitmap(source.len());
        if reuse { state.rank_scratch = Some(RankScratch::new(source.len())); }
        for v in 513..=n { assert!(state.apply(v)); }
        let checkpoint = state.checkpoint();
        assert_eq!(normalize(state.residual()),parent);
        let start = Instant::now();
        let mut work = 0;
        for _ in 0..128 {
            for (arm,lit) in [1,-1].into_iter().enumerate() {
                assert!(state.apply(lit));
                let (ids,key) = state.materialized_boundary();
                let forced = state.ranked_closure(&ids,deadline).unwrap().unwrap();
                assert_eq!(forced,expected[arm]);
                assert!(state.active.is_empty());
                work += forced.len();
                state.rollback(checkpoint);
                std::hint::black_box(key);
            }
        }
        let ns = start.elapsed().as_nanos();
        state.verify();
        assert_eq!(normalize(state.residual()),parent);
        state.rollback(0);
        state.verify();
        assert_eq!(state.residual(),source);
        println!("RANK_GENERATION_FINAL trial={trial} reuse={reuse} source={} active={} branches=256 forced={work} wall_ns={ns}",source.len(),parent.len());
    }
}

#[test]
#[ignore = "research-only dense rank generation control"]
fn rank_generation_dense_fixed_work() {
    // Dense root control: no inactive prefix. Same canonical boundary,
    // forced order, and bitmap state as the sparse sibling benchmark.
    let n = 8192_i32;
    let mut source = Vec::new();
    for v in 2..=9 { source.push(vec![1,v]); source.push(vec![-1,v]); }
    for v in 2..n { source.push(vec![-v,v+1]); }
    let source = normalize(source);
    let deadline = Instant::now() + Duration::from_secs(90);
    let parent = source.clone();
    let expected: Vec<_> = [1,-1].iter().map(|&lit| {
        let mut c = parent.clone();
        assign_inplace(&mut c,lit);
        let closed = indexed_units(normalize(c),n as usize,deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    for (trial, reuse) in [false,true,true,false,false,true,true,false,false,true,true,false].into_iter().enumerate() {
        let mut state = TrailIndex::with_mode(source.clone(),n as usize,false,true);
        state.active = Frontier::bitmap(source.len());
        if reuse { state.rank_scratch = Some(RankScratch::new(source.len())); }
        let checkpoint = state.checkpoint();
        assert_eq!(normalize(state.residual()),parent);
        let start = Instant::now();
        let mut work = 0;
        for _ in 0..32 {
            for (arm,lit) in [1,-1].into_iter().enumerate() {
                assert!(state.apply(lit));
                let (ids,key) = state.materialized_boundary();
                let forced = state.ranked_closure(&ids,deadline).unwrap().unwrap();
                assert_eq!(forced,expected[arm]);
                assert!(state.active.is_empty());
                work += forced.len();
                state.rollback(checkpoint);
                std::hint::black_box(key);
            }
        }
        let ns = start.elapsed().as_nanos();
        state.verify();
        assert_eq!(normalize(state.residual()),parent);
        state.rollback(0);
        state.verify();
        assert_eq!(state.residual(),source);
        println!("RANK_GENERATION_DENSE_FINAL trial={trial} reuse={reuse} source={} active={} branches=64 forced={work} wall_ns={ns}",source.len(),parent.len());
    }
}

#[test]
fn ranked_only_fused_random_nested_rollback() {
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
        let mut state = TrailIndex::with_mode(source.clone(), 12, (case / 4) % 2 == 0, (case / 8) % 2 == 0);
        if (case / 16) % 2 == 0 { state.active = Frontier::bitmap(source.len()); }
        if (case / 32) % 2 == 0 { state.rank_scratch = Some(RankScratch::new(source.len())); }
        state.ranked_only((case / 64) % 2 == 0);
        state.fuse_ranked_updates = (case / 128) % 2 == 0;
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
                    let canonical = normalize(state.residual());
                    let ids = state.canonical_ids();
                    assert_eq!(state.materialize_ids(&ids), canonical);
                    assert_eq!(state.materialized_boundary(), (ids.clone(), canonical.clone()));
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
fn ranked_only_fused_conflict_and_deadline_restore() {
    let source = normalize(vec![vec![1,2],vec![-1,2],vec![-2,3],vec![-3]]);
    for ranked_only in [false,true] {
        for fused in [false,true] {
            let mut st = TrailIndex::with_mode(source.clone(),3,false,true);
            st.active = Frontier::bitmap(source.len());
            st.rank_scratch = Some(RankScratch::new(source.len()));
            st.ranked_only(ranked_only);
            st.fuse_ranked_updates = fused;
            for lit in [1,-1] {
                st.apply(lit);
                let checkpoint = st.checkpoint();
                let (ids,key) = st.materialized_boundary();
                assert_eq!(st.ranked_closure(&ids,Instant::now()),Err("deadline"));
                st.verify();
                let expected = scan(key);
                let actual = st.ranked_closure(&ids,Instant::now()+Duration::from_secs(2)).unwrap()
                    .map(|forced| (st.materialize_ids(&ids),forced));
                assert_eq!(actual,expected);
                st.verify();
                st.rollback(checkpoint);
                st.verify();
                st.rollback(0);
                st.verify();
                assert_eq!(st.residual(),source);
            }
        }
    }
}

#[test]
#[ignore = "research-only ranked queue factorial diagnostic"]
fn single_queue_sparse_fixed_work() {
    // Large inactive prefix, and a small live chain. Set up the
    // parent once; do not charge repeated root construction to either arm.
    let n = 65536_i32;
    let mut source = Vec::new();
    for v in 2..=9 { source.push(vec![1,v]); source.push(vec![-1,v]); }
    for v in 2..512 { source.push(vec![-v,v+1]); }
    for v in 513..=n { source.push(vec![v]); }
    let source = normalize(source);
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut parent = source.clone();
    parent.retain(|c| !(c.len()==1 && c[0]>512));
    let expected: Vec<_> = [1,-1].iter().map(|&lit| {
        let mut c = parent.clone();
        assign_inplace(&mut c,lit);
        let closed = indexed_units(normalize(c),n as usize,deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    let orders = [[0,1,2,3],[1,2,3,0],[2,3,0,1],[3,0,1,2],[3,2,1,0],[0,3,2,1],[1,0,3,2],[2,1,0,3]];
    for (trial,order) in orders.into_iter().enumerate() {
      for mode in order {
        let mut state = TrailIndex::with_mode(source.clone(),n as usize,false,true);
        state.active = Frontier::bitmap(source.len());
        state.rank_scratch = Some(RankScratch::new(source.len()));
        state.ranked_only(mode & 1 != 0);
        state.fuse_ranked_updates = mode & 2 != 0;
        for v in 513..=n { assert!(state.apply(v)); }
        let checkpoint = state.checkpoint();
        assert_eq!(normalize(state.residual()),parent);
        let start = Instant::now();
        let mut work = 0;
        for _ in 0..128 {
            for (arm,lit) in [1,-1].into_iter().enumerate() {
                assert!(state.apply(lit));
                let (ids,key) = state.materialized_boundary();
                let forced = state.ranked_closure(&ids,deadline).unwrap().unwrap();
                assert_eq!(forced,expected[arm]);
                assert!(state.active.is_empty());
                work += forced.len();
                state.rollback(checkpoint);
                std::hint::black_box(key);
            }
        }
        let ns = start.elapsed().as_nanos();
        state.verify();
        assert_eq!(normalize(state.residual()),parent);
        state.rollback(0);
        state.verify();
        assert_eq!(state.residual(),source);
        println!("SINGLE_QUEUE_FINAL cohort=sparse trial={trial} mode={mode} source={} active={} branches=256 forced={work} wall_ns={ns}",source.len(),parent.len());
    }
  }
}

#[test]
#[ignore = "research-only ranked queue factorial diagnostic"]
fn single_queue_dense_fixed_work() {
    // Dense root control: no inactive prefix. Same canonical boundary,
    // forced order, and bitmap state as the sparse sibling benchmark.
    let n = 8192_i32;
    let mut source = Vec::new();
    for v in 2..=9 { source.push(vec![1,v]); source.push(vec![-1,v]); }
    for v in 2..n { source.push(vec![-v,v+1]); }
    let source = normalize(source);
    let deadline = Instant::now() + Duration::from_secs(90);
    let parent = source.clone();
    let expected: Vec<_> = [1,-1].iter().map(|&lit| {
        let mut c = parent.clone();
        assign_inplace(&mut c,lit);
        let closed = indexed_units(normalize(c),n as usize,deadline).unwrap().unwrap();
        assert!(closed.clauses.is_empty());
        closed.forced
    }).collect();
    let orders = [[0,1,2,3],[1,2,3,0],[2,3,0,1],[3,0,1,2],[3,2,1,0],[0,3,2,1],[1,0,3,2],[2,1,0,3]];
    for (trial,order) in orders.into_iter().enumerate() {
      for mode in order {
        let mut state = TrailIndex::with_mode(source.clone(),n as usize,false,true);
        state.active = Frontier::bitmap(source.len());
        state.rank_scratch = Some(RankScratch::new(source.len()));
        state.ranked_only(mode & 1 != 0);
        state.fuse_ranked_updates = mode & 2 != 0;
        let checkpoint = state.checkpoint();
        assert_eq!(normalize(state.residual()),parent);
        let start = Instant::now();
        let mut work = 0;
        for _ in 0..32 {
            for (arm,lit) in [1,-1].into_iter().enumerate() {
                assert!(state.apply(lit));
                let (ids,key) = state.materialized_boundary();
                let forced = state.ranked_closure(&ids,deadline).unwrap().unwrap();
                assert_eq!(forced,expected[arm]);
                assert!(state.active.is_empty());
                work += forced.len();
                state.rollback(checkpoint);
                std::hint::black_box(key);
            }
        }
        let ns = start.elapsed().as_nanos();
        state.verify();
        assert_eq!(normalize(state.residual()),parent);
        state.rollback(0);
        state.verify();
        assert_eq!(state.residual(),source);
        println!("SINGLE_QUEUE_FINAL cohort=dense trial={trial} mode={mode} source={} active={} branches=64 forced={work} wall_ns={ns}",source.len(),parent.len());
    }
  }
}

 
// Bounded test-only full weighted search. A persistent incidence index
// belongs to exactly one component. Splits create independent children;
// a sibling branch instead rolls back to its parent checkpoint.
use super::{cache_can_admit, components_with_parent, key_items, Key, KeyQuery, KeyView};
use crate::{Outcome, Problem, C};
use std::collections::HashMap;

#[derive(Default, Debug)]
struct TrailStats {
    nodes: usize,
    hits: usize,
    splits: usize,
    indexes: usize,
    branches: usize,
    free: usize,
}
struct ScopedTrail<'a> {
    p: &'a Problem,
    bits: u32,
    deadline: Instant,
    cache: HashMap<Key, Outcome>,
    items: usize,
    stats: TrailStats,
    max_nodes: usize,
    trace: Option<super::WorkTrace>,
}
impl<'a> ScopedTrail<'a> {
    fn new(p: &'a Problem, bits: u32, deadline: Instant) -> Self {
        Self { p, bits, deadline, cache: HashMap::new(), items: 0,
            stats: TrailStats::default(), max_nodes: 100_000, trace: None }
    }
    fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline { return Err("research deadline".into()); }
        Ok(())
    }
    fn weight(&self, v: usize, positive: bool) -> &C {
        let w = &self.p.weights[v-1];
        if positive { &w.positive } else { &w.negative }
    }
    fn save(&mut self, key: Option<Key>, out: Outcome) -> Result<Outcome,String> {
        if !out.count.re.is_finite() || !out.count.im.is_finite() {
            return Err("nonfinite research".into());
        }
        if let Some(key) = key {
            let items = key_items(&key.scope,&key.clauses);
            if cache_can_admit(self.cache.len(),self.items,items) {
                self.items += items;
                self.cache.insert(key,out.clone());
            }
        }
        Ok(out)
    }
    fn fresh(&mut self, clauses: Vec<Clause>, scope: Vec<usize>, depth: usize) -> Result<Outcome,String> {
        self.check()?;
        let mut state = TrailIndex::with_mode(normalize(clauses),self.p.n,false,true);
        state.active = Frontier::bitmap(state.clauses.len());
        state.rank_scratch = Some(RankScratch::new(state.clauses.len()));
        state.ranked_only(true);
        state.fuse_ranked_updates = true;
        self.stats.indexes += 1;
        self.node(&mut state,scope,depth)
    }
    // Every node restores its entry checkpoint, including early
    // conflict, cache hit and error. Branch assignment belongs to caller.
    fn node(&mut self, state: &mut TrailIndex, scope: Vec<usize>, depth: usize) -> Result<Outcome,String> {
        let checkpoint = state.checkpoint();
        let result = self.enter(state,scope,depth);
        state.rollback(checkpoint);
        result
    }
    fn child(&mut self, state: &mut TrailIndex, scope: Vec<usize>, lit: i32, depth: usize) -> Result<Outcome,String> {
        let checkpoint = state.checkpoint();
        // Do not short-circuit here: the existing engine enters the
        // child, including its canonical key, even on a conflict.
        state.apply(lit);
        let result = self.node(state,scope,depth);
        state.rollback(checkpoint);
        result
    }
    fn enter(&mut self, state: &mut TrailIndex, scope: Vec<usize>, depth: usize) -> Result<Outcome,String> {
        self.check()?;
        if depth > 96 || self.stats.nodes >= self.max_nodes { return Err("research work limit".into()); }
        self.stats.nodes += 1;
        let (ids,clauses) = state.materialized_boundary();
        if let Some(t) = &mut self.trace { t.enter(&scope, &clauses); }
        let query = KeyQuery { scope: &scope, clauses: &clauses };
        let view: &dyn KeyView = &query;
        if let Some(hit) = self.cache.get(view) {
            self.stats.hits += 1;
            if let Some(t) = &mut self.trace { t.hits += 1; }
            return Ok(hit.clone());
        }
        let key = cache_can_admit(self.cache.len(),self.items,key_items(&scope,&clauses))
            .then(|| Key { scope: scope.clone(), clauses });
        let Some(forced) = state.ranked_closure(&ids,self.deadline)? else {
            return self.save(key,Outcome::unsat(self.bits));
        };
        let mut prefix = C::one(self.bits);
        for lit in forced {
            self.check()?;
            prefix = prefix.mul(self.weight(lit.unsigned_abs() as usize,lit>0),self.bits);
        }
        // Preserve the post-closure order and multiplicity; only the
        // next node's canonical boundary sorts and deduplicates.
        let mut residual = state.materialize_ids(&ids);
        let mut frequency = vec![0usize;self.p.n+1];
        for c in &residual {
            for &lit in c { frequency[lit.unsigned_abs() as usize] += 1; }
        }
        let mut active = Vec::new();
        for v in scope {
            if state.assigned[v] != 0 { continue; }
            if frequency[v] == 0 {
                self.stats.free += 1;
                if let Some(t) = &mut self.trace { t.free += 1; }
                let w = &self.p.weights[v-1];
                prefix = prefix.mul(&w.positive.add(&w.negative,self.bits),self.bits);
            } else { active.push(v); }
        }
        if residual.is_empty() {
            return self.save(key,Outcome { sat:true,count:prefix });
        }
        let parts = components_with_parent(&mut residual,&active,&mut vec![0;self.p.n+1]);
        if !parts.is_empty() {
            self.stats.splits += 1;
            if let Some(t) = &mut self.trace { t.splits += 1; }
            let mut value = C::one(self.bits);
            for (scope,clauses) in parts {
                let out = self.fresh(clauses,scope,depth+1)?;
                if !out.sat { return self.save(key,Outcome::unsat(self.bits)); }
                value = value.mul(&out.count,self.bits);
            }
            return self.save(key,Outcome { sat:true,count:prefix.mul(&value,self.bits) });
        }
        let v = *active.iter().max_by_key(|&&v|frequency[v]).ok_or("research pivot")?;
        let child: Vec<_> = active.into_iter().filter(|&x|x!=v).collect();
        self.stats.branches += 1;
        if let Some(t) = &mut self.trace { t.branches += 1; }
        let yes = self.child(state,child.clone(),v as i32,depth+1)?;
        let no = self.child(state,child,-(v as i32),depth+1)?;
        let y = self.weight(v,true).mul(&yes.count,self.bits);
        let n = self.weight(v,false).mul(&no.count,self.bits);
        self.save(key,Outcome { sat:yes.sat||no.sat,count:prefix.mul(&y.add(&n,self.bits),self.bits) })
    }
}

fn same_outcome(a: &Outcome,b: &Outcome) {
    assert_eq!(a.sat,b.sat);
    assert_eq!(a.count.re,b.count.re);
    assert_eq!(a.count.im,b.count.im);
}

#[test]
fn scoped_trail_full_wmc_randomized() {
    use crate::parse_emitted;
    let mut seed = 0x81a3b93d_u32;
    let mut next = || { seed = seed.wrapping_mul(1664525).wrapping_add(1013904223); seed >> 8 };
    let deadline = Instant::now()+Duration::from_secs(60);
    let mut totals = TrailStats::default();
    for case in 0..512 {
        let n = 8;
        let mut clauses = Vec::new();
        for _ in 0..(next()%15) {
            let mut c = Vec::new();
            let width = if case%8==0 { next()%5 } else { 2+next()%3 };
            for _ in 0..width {
                let v = (next()%n+1) as i32;
                c.push(if next()%2==0 {v} else {-v});
            }
            clauses.push(c);
        }
        let dyadic = case%2==0;
        let weights = if dyadic {
            ["0.5 0.25","-0.25 0.125","0 0","1 0","-1 0","0.125 -0.5"]
        } else {
            ["0.13 0.27","-0.29 0.31","0 0","1 0","-1 0","0.37 -0.41"]
        };
        let mut raw = format!("p cnf {n} {}\nc t wmc\n",clauses.len());
        for v in 1..=n as i32 {
            for lit in [v,-v] {
                raw.push_str(&format!("c p weight {lit} {} 0\n",weights[next() as usize%weights.len()]));
            }
        }
        for c in &clauses {
            for lit in c { raw.push_str(&format!("{lit} ")); }
            raw.push_str("0\n");
        }
        for bits in [100,200,333,432] {
            let p = parse_emitted(&raw,bits).unwrap();
            let mut engine = ScopedTrail::new(&p,bits,deadline);
            let actual = engine.fresh(p.clauses.clone(),(1..=p.n).collect(),0).unwrap();
            let baseline = super::solve(&p,bits,deadline).unwrap();
            same_outcome(&actual,&baseline);
            // Small dyadic counts are exactly representable at every
            // selected precision. Non-dyadic tests require exact
            // production MPFR operation-order parity instead.
            if dyadic { same_outcome(&actual,&crate::reference(&p,&raw,bits).unwrap()); }
            totals.nodes += engine.stats.nodes;
            totals.hits += engine.stats.hits;
            totals.splits += engine.stats.splits;
            totals.branches += engine.stats.branches;
            totals.free += engine.stats.free;
        }
    }
    assert!(totals.hits>0 && totals.splits>0 && totals.branches>0 && totals.free>0);
    println!("SCOPED_TRAIL_RANDOM_PASS counts=2048 independent_dyadic=1024 nodes={} hits={} splits={} branches={} free={}",totals.nodes,totals.hits,totals.splits,totals.branches,totals.free);
}

#[test]
fn scoped_trail_cache_scope_cancel_and_error_rollback() {
    use crate::parse_emitted;
    let raw = "p cnf 6 2\nc t wmc\nc p weight 5 -1 0 0\n1 2 0\n3 4 0\n";
    for bits in [100,200,333,432] {
        let p = parse_emitted(raw,bits).unwrap();
        let deadline = Instant::now()+Duration::from_secs(5);
        let mut e = ScopedTrail::new(&p,bits,deadline);
        let source = normalize(p.clauses.clone());
        let mut st = TrailIndex::with_mode(source.clone(),p.n,false,true);
        st.active = Frontier::bitmap(source.len());
        st.rank_scratch = Some(RankScratch::new(source.len()));
        st.ranked_only(true);
        st.fuse_ranked_updates = true;
        let a = e.node(&mut st,(1..=6).collect(),0).unwrap();
        assert!(a.sat);
        assert_eq!(a.count.re,0);
        assert_eq!(a.count.im,0);
        let hits = e.stats.hits;
        same_outcome(&a,&e.node(&mut st,(1..=6).collect(),0).unwrap());
        assert_eq!(e.stats.hits,hits+1);
        let b = e.node(&mut st,vec![1,2,3,4,6],0).unwrap();
        assert!(b.sat);
        assert_eq!(b.count.re,18);
        assert_eq!(b.count.im,0);
        assert!(e.stats.splits>0 && e.stats.indexes>0);
        st.verify();
        assert_eq!(st.residual(),source);
        assert!(st.trail.is_empty());
        let mut limited = ScopedTrail::new(&p,bits,deadline);
        limited.max_nodes=1;
        assert!(limited.node(&mut st,(1..=6).collect(),0).is_err());
        st.verify();
        assert!(st.trail.is_empty());
        assert_eq!(st.residual(),source);
        limited.deadline=Instant::now();
        assert!(limited.child(&mut st,vec![2,3,4,5,6],1,0).is_err());
        st.verify();
        assert!(st.trail.is_empty());
        assert_eq!(st.residual(),source);
    }
}

 
fn scoped_ablation_run(p: &Problem,bits: u32,reuse: bool,trace: bool)
    -> Result<(Outcome,Option<super::WorkTrace>,usize),String> {
    let deadline = Instant::now()+Duration::from_secs(12);
    if reuse {
        let mut engine = ScopedTrail::new(p,bits,deadline);
        engine.trace = trace.then(super::WorkTrace::default);
        let out = engine.fresh(p.clauses.clone(),(1..=p.n).collect(),0)?;
        Ok((out,engine.trace,engine.stats.indexes))
    } else {
        let mut engine = super::Engine {
            p,bits,deadline,cache: HashMap::new(),cache_items:0,
            large:true,frequency:vec![0;p.n+1],parent:vec![0;p.n+1],
            trace:trace.then(super::WorkTrace::default),
        };
        let out = engine.count(p.clauses.clone(),(1..=p.n).collect())?;
        Ok((out,engine.trace,0))
    }
}

#[test]
#[ignore = "research-only complete weighted search comparison"]
fn scoped_complete_ablation() {
    let digits: usize = std::env::var("RAWMOCS_TEST_DIGITS").unwrap().parse().unwrap();
    let bits = match digits {30=>100,130=>432,_=>panic!("unsupported cohort")};
    let original = std::fs::read_to_string(std::env::var("RAWMOCS_TEST_ORIGINAL").unwrap()).unwrap();
    // Chain with two sibling branches, signed-complex terminal weights.
    let n = 8192_i32;
    let mut chain = "p cnf 8192 8206\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight 8192 0.25 -0.125 0\n".to_owned();
    for v in 2..=9 { chain.push_str(&format!("1 {v} 0\n-1 {v} 0\n")); }
    for v in 2..n { chain.push_str(&format!("-{v} {} 0\n",v+1)); }
    // Independent binary components with one zero-weight polarity:
    // only the all-positive assignment has nonzero weight.
    let mut split = "p cnf 4096 2048\nc t wmc\n".to_owned();
    for v in 1..=4096 { split.push_str(&format!("c p weight -{v} 0 0 0\n")); }
    for v in (1..=4096).step_by(2) { split.push_str(&format!("{v} {} 0\n",v+1)); }
    for (name,raw) in [("chain8192",chain),("split4096",split),("vqe8",original)] {
        let p = crate::parse_large(std::io::Cursor::new(raw.as_bytes()),bits,Instant::now()+Duration::from_secs(12)).unwrap();
        // Trace runs are distinct from timing runs. The hash is a
        // diagnostic, not a formal search equivalence certificate.
        let (base,bt,_) = scoped_ablation_run(&p,bits,false,true).unwrap();
        let (candidate,ct,indexes) = scoped_ablation_run(&p,bits,true,true).unwrap();
        same_outcome(&base,&candidate);
        let a = bt.unwrap().summary();
        let b = ct.unwrap().summary();
        assert_eq!(a,b,"canonical node sequence mismatch for {name}");
        if name=="chain8192" {
            assert!(base.sat);
            assert_eq!(base.count.re,rug::Float::with_val(bits,0.15625));
            assert_eq!(base.count.im,0);
        } else if name=="split4096" {
            assert!(base.sat);
            assert_eq!(base.count.re,1);
            assert_eq!(base.count.im,0);
            assert!(a.3>0);
        }
        println!("SCOPED_WORK_TRACE case={name} digits={digits} fingerprint={:016x} nodes={} hits={} splits={} branches={} free={} indexes={indexes}",a.0,a.1,a.2,a.3,a.4,a.5);
        for (trial,reuse) in [false,true,true,false,false,true,true,false].into_iter().enumerate() {
            let start=Instant::now();
            let (out,_,_) = scoped_ablation_run(&p,bits,reuse,false).unwrap();
            let wall_ns=start.elapsed().as_nanos();
            same_outcome(&base,&out);
            let sat=if out.sat {"SATISFIABLE"} else {"UNSATISFIABLE"};
            let real=out.count.re.to_string_radix(10,Some(digits+4));
            let imag=out.count.im.to_string_radix(10,Some(digits+4));
            println!("SCOPED_FULL_FINAL case={name} digits={digits} trial={trial} reuse={reuse} wall_ns={wall_ns} sat={sat} real={real} imag={imag}");
        }
    }
}
