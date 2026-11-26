//! Exact-local elimination and bounded prejoin contraction.
//! Source-pinned kernel: 379b47242cbd9b4a1e597afc6377cbaf0ac13bce, factor_prejoin.rs.
//! Diagnostic command surface removed; numerical/planner kernel unchanged.
#[cfg(test)]
use crate::parse_emitted;
use crate::{Outcome, Problem, C};
use rug::{ops::Pow, Assign, Float, Integer, Rational};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

const WIDTH: usize = 19;
const RESIDUAL_CAP: usize = 256;
const WORK: usize = 40_000_000;
const LIVE: usize = 2_000_000;
const LOCAL_WORK: usize = 1_000_000;
const EXACT_BITS: u32 = 16_384;
type Scope = BTreeSet<usize>;
type R = (Rational, Rational);
type Cell<T> = Option<T>;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Factor<T> {
    scope: Vec<usize>,
    values: Vec<Cell<T>>,
}
#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct Cost {
    width: usize,
    work: usize,
    peak: usize,
    joins: usize,
}
fn deadline(d: Instant) -> Result<(), String> {
    if Instant::now() >= d {
        Err("prejoin deadline".into())
    } else {
        Ok(())
    }
}
pub(crate) fn decimal(s: &str) -> Result<Rational, String> {
    if s.len() > 128 {
        return Err("exact decimal length".into());
    }
    let mut p = s.split(['e', 'E']);
    let a = p.next().ok_or("decimal")?;
    let e = p
        .next()
        .map_or(Ok(0), str::parse::<i32>)
        .map_err(|_| "decimal exponent")?;
    if p.next().is_some() || !(-256..=256).contains(&e) {
        return Err("decimal exponent".into());
    }
    let neg = a.starts_with('-');
    let a = a.strip_prefix(['-', '+']).unwrap_or(a);
    let (w, f) = a.split_once('.').unwrap_or((a, ""));
    if (w.is_empty() && f.is_empty()) || !w.bytes().chain(f.bytes()).all(|b| b.is_ascii_digit()) {
        return Err("decimal digits".into());
    }
    let mut n = Integer::from_str_radix(&format!("{w}{f}"), 10).map_err(|_| "decimal integer")?;
    if neg {
        n = -n;
    }
    let scale = e - f.len() as i32;
    let d = if scale < 0 {
        Integer::from(10).pow((-scale) as u32)
    } else {
        n *= Integer::from(10).pow(scale as u32);
        Integer::from(1)
    };
    Ok(Rational::from((n, d)))
}
fn one() -> R {
    (Rational::from(1), Rational::from(0))
}
fn checked(x: R) -> Result<R, String> {
    if [&x.0, &x.1].iter().any(|r| {
        r.numer()
            .significant_bits()
            .max(r.denom().significant_bits())
            > EXACT_BITS
    }) {
        return Err("exact local bit cap".into());
    }
    Ok(x)
}
fn rmul_full(a: &R, b: &R) -> Result<R, String> {
    checked((
        Rational::from(&a.0 * &b.0) - Rational::from(&a.1 * &b.1),
        Rational::from(&a.0 * &b.1) + Rational::from(&a.1 * &b.0),
    ))
}
// Exact axes and Boolean support depend on exact rational equality.
// Shortcuts only for algebraic identities, never rounded MPFR comparisons.
fn rmul(a: &R, b: &R) -> Result<R, String> {
    if (a.0 == 0 && a.1 == 0) || (b.0 == 0 && b.1 == 0) {
        return Ok((Rational::from(0), Rational::from(0)));
    }
    if a.0 == 1 && a.1 == 0 {
        return checked(b.clone());
    }
    if b.0 == 1 && b.1 == 0 {
        return checked(a.clone());
    }
    if a.0 == -1 && a.1 == 0 {
        return checked((-b.0.clone(), -b.1.clone()));
    }
    if b.0 == -1 && b.1 == 0 {
        return checked((-a.0.clone(), -a.1.clone()));
    }
    if a.0 == 0 && a.1 == 1 {
        return checked((-b.1.clone(), b.0.clone()));
    }
    if b.0 == 0 && b.1 == 1 {
        return checked((-a.1.clone(), a.0.clone()));
    }
    if a.0 == 0 && a.1 == -1 {
        return checked((b.1.clone(), -b.0.clone()));
    }
    if b.0 == 0 && b.1 == -1 {
        return checked((a.1.clone(), -a.0.clone()));
    }
    // Non-unit real/imaginary axes require only two rational products.
    // Keep exact support and the same final bit-cap check as the full formula.
    if a.1 == 0 {
        return checked((Rational::from(&a.0 * &b.0), Rational::from(&a.0 * &b.1)));
    }
    if b.1 == 0 {
        return checked((Rational::from(&a.0 * &b.0), Rational::from(&a.1 * &b.0)));
    }
    if a.0 == 0 {
        return checked((-Rational::from(&a.1 * &b.1), Rational::from(&a.1 * &b.0)));
    }
    if b.0 == 0 {
        return checked((-Rational::from(&a.1 * &b.1), Rational::from(&a.0 * &b.1)));
    }
    rmul_full(a, b)
}
fn radd(a: Cell<R>, b: Cell<R>) -> Result<Cell<R>, String> {
    match (a, b) {
        (Some(a), Some(b)) => Ok(Some(checked((a.0 + b.0, a.1 + b.1))?)),
        (a, None) | (None, a) => Ok(a),
    }
}
fn reduce_axes<T: PartialEq>(mut f: Factor<T>) -> Factor<T> {
    for j in (0..f.scope.len()).rev() {
        let bit = 1 << j;
        if (0..f.values.len())
            .filter(|m| m & bit == 0)
            .all(|m| f.values[m] == f.values[m | bit])
        {
            f.scope.remove(j);
            f.values = f
                .values
                .into_iter()
                .enumerate()
                .filter_map(|(m, x)| (m & bit == 0).then_some(x))
                .collect();
        }
    }
    f
}
fn rscalar(s: Cell<R>, f: Cell<R>) -> Result<Cell<R>, String> {
    match (s, f) {
        (Some(a), Some(b)) => Ok(Some(rmul(&a, &b)?)),
        _ => Ok(None),
    }
}
fn mask(scope: &[usize], out: &[usize], m: usize, pivot: usize, positive: bool) -> usize {
    scope.iter().enumerate().fold(0, |a, (j, u)| {
        let set = if *u == pivot {
            positive
        } else {
            m & (1 << out.binary_search(u).expect("scope subset")) != 0
        };
        a | (usize::from(set) << j)
    })
}
type Local = (Vec<Factor<R>>, Scope, Cell<R>, Cost);
// Exact multiplicity of each live primal edge; a shared edge disappears
// only when its last supporting factor is removed.
type EdgeCounts = Vec<BTreeMap<usize, usize>>;
fn edge_add(edges: &mut EdgeCounts, scope: &[usize]) {
    for (i, &v) in scope.iter().enumerate() {
        for &u in &scope[i + 1..] {
            *edges[v].entry(u).or_default() += 1;
            *edges[u].entry(v).or_default() += 1;
        }
    }
}
fn edge_remove(edges: &mut EdgeCounts, scope: &[usize]) -> Result<(), String> {
    for (i, &v) in scope.iter().enumerate() {
        for &u in &scope[i + 1..] {
            for (a, b) in [(v, u), (u, v)] {
                let count = edges[a].get_mut(&b).ok_or("local edge missing")?;
                *count = count.checked_sub(1).ok_or("local edge underflow")?;
                if *count == 0 {
                    edges[a].remove(&b);
                }
            }
        }
    }
    Ok(())
}
fn local(p: &Problem, raw: &str, d: Instant) -> Result<Local, String> {
    local_frontier(p, raw, d, true, true)
}
fn local_frontier(
    p: &Problem,
    raw: &str,
    d: Instant,
    incremental: bool,
    widen: bool,
) -> Result<Local, String> {
    local_frontier_probe::<false, true>(p, raw, d, incremental, widen, None)
}
// Native-only phase attribution. Prefix replay is a research diagnostic and
// must never be accepted as a completed residual or WMC result.
fn local_frontier_probe<const PROFILE: bool, const ADJ: bool>(
    p: &Problem,
    raw: &str,
    d: Instant,
    incremental: bool,
    widen: bool,
    step_cap: Option<usize>,
) -> Result<Local, String> {
    let started = if PROFILE { Some(Instant::now()) } else { None };
    let mut phase = started;
    let mut times = [0u128; 6];
    let tick = |phase: &mut Option<Instant>, slot: &mut u128| {
        if PROFILE {
            let now = Instant::now();
            *slot += now
                .duration_since(phase.replace(now).expect("profile clock"))
                .as_nanos();
        }
    };
    let mut steps = 0usize;
    if p.clauses.len() > 16_000 {
        return Err("source clause cap".into());
    }
    let mut weights = vec![[one(), one()]; p.n];
    for line in raw.lines() {
        let a: Vec<_> = line.split_whitespace().collect();
        if a.len() == 7 && a[..3] == ["c", "p", "weight"] {
            let l: i32 = a[3].parse().map_err(|_| "weight literal")?;
            weights[l.unsigned_abs() as usize - 1][usize::from(l > 0)] =
                checked((decimal(a[4])?, decimal(a[5])?))?;
        }
    }
    let scopes: Vec<Vec<usize>> = p
        .clauses
        .iter()
        .map(|c| {
            c.iter()
                .map(|l| l.unsigned_abs() as usize)
                .collect::<Scope>()
                .into_iter()
                .collect()
        })
        .collect();
    if scopes.iter().any(|s| s.len() > 9) {
        return Err("clause width cap".into());
    }
    let initial = 2 * p.n + scopes.iter().map(|s| 1usize << s.len()).sum::<usize>();
    if initial > LIVE {
        return Err("initial cells cap".into());
    }
    let mut fs = Vec::new();
    let mut scalar = Some(one());
    let insert =
        |fs: &mut Vec<Factor<R>>, scalar: &mut Cell<R>, f: Factor<R>| -> Result<(), String> {
            let f = reduce_axes(f);
            if f.scope.is_empty() {
                *scalar = rscalar(scalar.take(), f.values.into_iter().next().unwrap())?;
            } else {
                fs.push(f);
            }
            Ok(())
        };
    for (c, scope) in p.clauses.iter().zip(scopes) {
        deadline(d)?;
        let values = (0..1usize << scope.len())
            .map(|m| {
                c.iter()
                    .any(|l| {
                        (m & (1 << scope.binary_search(&(l.unsigned_abs() as usize)).unwrap()) != 0)
                            == (*l > 0)
                    })
                    .then(one)
            })
            .collect();
        insert(&mut fs, &mut scalar, Factor { scope, values })?;
    }
    for (i, [a, b]) in weights.into_iter().enumerate() {
        insert(
            &mut fs,
            &mut scalar,
            Factor {
                scope: vec![i + 1],
                values: vec![Some(a), Some(b)],
            },
        )?;
    }
    tick(&mut phase, &mut times[0]);
    let mut nodes: Scope = (1..=p.n).collect();
    let mut cost = Cost {
        peak: initial,
        ..Cost::default()
    };
    // Stable factor IDs preserve the old vector's survivor/insertion order.
    let mut tables: BTreeMap<usize, Factor<R>> = fs.into_iter().enumerate().collect();
    let mut next = tables.len();
    let mut owners = vec![Scope::new(); p.n + 1];
    let mut edges: EdgeCounts = if ADJ {
        vec![BTreeMap::new(); p.n + 1]
    } else {
        Vec::new()
    };
    let mut live = 0usize;
    for (&id, f) in &tables {
        live += f.values.len();
        for &v in &f.scope {
            owners[v].insert(id);
        }
        if ADJ {
            edge_add(&mut edges, &f.scope);
        }
    }
    let neighbors = |v: usize,
                     owners: &[Scope],
                     tables: &BTreeMap<usize, Factor<R>>,
                     edges: &EdgeCounts|
     -> Scope {
        if ADJ {
            edges[v].keys().copied().collect()
        } else {
            owners[v]
                .iter()
                .flat_map(|id| tables[id].scope.iter().copied())
                .filter(|&u| u != v)
                .collect()
        }
    };
    let key =
        |v: usize, owners: &[Scope], tables: &BTreeMap<usize, Factor<R>>, edges: &EdgeCounts| {
            let degree = if ADJ {
                edges[v].len()
            } else {
                neighbors(v, owners, tables, edges).len()
            };
            (degree, owners[v].len(), usize::MAX - v)
        };
    // Exact ordered-set queue: no stale entries, one key per active variable.
    let mut queue: BTreeSet<_> = nodes
        .iter()
        .map(|&v| key(v, &owners, &tables, &edges))
        .collect();
    let mut keys: Vec<_> = (0..=p.n)
        .map(|v| {
            if v == 0 {
                None
            } else {
                Some(key(v, &owners, &tables, &edges))
            }
        })
        .collect();
    let mut local_width = 4;
    tick(&mut phase, &mut times[1]);
    while !nodes.is_empty() {
        if step_cap.is_some_and(|cap| steps >= cap) {
            break;
        }
        deadline(d)?;
        let k = if incremental {
            *queue.first().ok_or("local queue empty")?
        } else {
            nodes
                .iter()
                .map(|&v| key(v, &owners, &tables, &edges))
                .min()
                .ok_or("local pivot")?
        };
        let v = usize::MAX - k.2;
        let nb = neighbors(v, &owners, &tables, &edges);
        if nb.len() > local_width {
            // Preserve every previously admitted width-4 route. Widen only
            // if the old residual cap would otherwise reject the problem.
            if widen && local_width == 4 && nodes.len() > RESIDUAL_CAP {
                local_width = 6;
            }
            if nb.len() > local_width {
                break;
            }
        }
        if incremental {
            for u in std::iter::once(v).chain(nb.iter().copied()) {
                if !queue.remove(&keys[u].take().ok_or("local missing key")?) {
                    return Err("local queue drift".into());
                }
            }
        }
        let out: Vec<_> = nb.iter().copied().collect();
        let size = 1usize << out.len();
        cost.work += 2 * size * owners[v].len();
        cost.peak = cost.peak.max(live + 2 * size);
        cost.width = cost.width.max(out.len());
        if cost.work > LOCAL_WORK || cost.peak > LIVE {
            return Err("local budget".into());
        }
        tick(&mut phase, &mut times[2]);
        let ids = std::mem::take(&mut owners[v]);
        let mut bucket = Vec::with_capacity(ids.len());
        for id in ids {
            let f = tables.remove(&id).ok_or("local factor owner")?;
            live -= f.values.len();
            if ADJ {
                edge_remove(&mut edges, &f.scope)?;
            }
            for &u in &f.scope {
                if u != v && !owners[u].remove(&id) {
                    return Err("local incidence drift".into());
                }
            }
            bucket.push(f);
        }
        if ADJ && !edges[v].is_empty() {
            return Err("local pivot edge drift".into());
        }
        tick(&mut phase, &mut times[3]);
        let mut vals = Vec::with_capacity(size);
        for m in 0..size {
            let mut pair = [None, None];
            for (j, positive) in [false, true].into_iter().enumerate() {
                let mut x = Some(one());
                for f in &bucket {
                    let b = &f.values[mask(&f.scope, &out, m, v, positive)];
                    x = match (x, b) {
                        (Some(a), Some(b)) => Some(rmul(&a, b)?),
                        _ => None,
                    };
                    if x.is_none() {
                        break;
                    }
                }
                pair[j] = x;
            }
            let [a, b] = pair;
            vals.push(radd(a, b)?);
        }
        tick(&mut phase, &mut times[4]);
        let f = reduce_axes(Factor {
            scope: out,
            values: vals,
        });
        if f.scope.is_empty() {
            scalar = rscalar(scalar.take(), f.values.into_iter().next().unwrap())?;
        } else {
            live += f.values.len();
            for &u in &f.scope {
                owners[u].insert(next);
            }
            if ADJ {
                edge_add(&mut edges, &f.scope);
            }
            tables.insert(next, f);
            next += 1;
        }
        nodes.remove(&v);
        if incremental {
            for u in nb {
                let k = key(u, &owners, &tables, &edges);
                keys[u] = Some(k);
                if !queue.insert(k) {
                    return Err("local queue duplicate".into());
                }
            }
        }
        steps += 1;
        tick(&mut phase, &mut times[5]);
    }
    if PROFILE {
        tick(&mut phase, &mut times[2]);
        eprintln!("LOCAL_PROFILE_NS init={} index={} select={} detach={} contract={} reinsert={} steps={} remaining={} work={} peak={}",times[0],times[1],times[2],times[3],times[4],times[5],steps,nodes.len(),cost.work,cost.peak);
    }
    if step_cap.is_none() && nodes.len() > RESIDUAL_CAP {
        return Err("residual variable cap".into());
    }
    Ok((tables.into_values().collect(), nodes, scalar, cost))
}
fn restrict(
    fs: &[Factor<R>],
    scalar: &Cell<R>,
    cuts: &[usize],
    m: usize,
) -> Result<(Vec<Factor<R>>, Cell<R>), String> {
    let mut out = Vec::new();
    let mut scalar = scalar.clone();
    for f in fs {
        let mut f = f.clone();
        for (i, v) in cuts.iter().enumerate() {
            if let Ok(j) = f.scope.binary_search(v) {
                let bit = 1 << j;
                let value = m & (1 << i) != 0;
                f.scope.remove(j);
                f.values = f
                    .values
                    .into_iter()
                    .enumerate()
                    .filter_map(|(a, x)| ((a & bit != 0) == value).then_some(x))
                    .collect();
            }
        }
        let f = reduce_axes(f);
        if f.scope.is_empty() {
            scalar = rscalar(scalar, f.values.into_iter().next().unwrap())?;
        } else {
            out.push(f);
        }
    }
    Ok((out, scalar))
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Op {
    ids: Vec<usize>,
    dst: usize,
    out: Vec<usize>,
    pivot: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Plan {
    ops: Vec<Op>,
    cost: Cost,
    extra: usize,
}
// Dense indices are local to a plan; original variable IDs and ordering remain
// unchanged. A mask represents the same undirected graph as the ordered sets.
fn mask_indices(mut m: u64) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if m == 0 {
            return None;
        }
        let i = m.trailing_zeros() as usize;
        m &= m - 1;
        Some(i)
    })
}
#[derive(Clone, Copy)]
struct Mask<const N: usize>([u64; N]);
impl<const N: usize> Mask<N> {
    fn empty() -> Self {
        Self([0; N])
    }
    fn insert(&mut self, i: usize) {
        self.0[i / 64] |= 1u64 << (i % 64);
    }
    fn remove(&mut self, i: usize) {
        self.0[i / 64] &= !(1u64 << (i % 64));
    }
    fn count(self) -> usize {
        self.0.iter().map(|w| w.count_ones() as usize).sum()
    }
    fn indices(self) -> impl Iterator<Item = usize> {
        self.0
            .into_iter()
            .enumerate()
            .flat_map(|(k, w)| mask_indices(w).map(move |i| 64 * k + i))
    }
    fn union(&mut self, b: Self) {
        for (a, b) in self.0.iter_mut().zip(b.0) {
            *a |= b;
        }
    }
    fn union_count(self, b: Self) -> usize {
        self.0
            .into_iter()
            .zip(b.0)
            .map(|(a, b)| (a | b).count_ones() as usize)
            .sum()
    }
    fn difference_count(self, b: Self) -> usize {
        self.0
            .into_iter()
            .zip(b.0)
            .map(|(a, b)| (a & !b).count_ones() as usize)
            .sum()
    }
}
fn fill_words<const N: usize>(graph: &[Mask<N>], nb: Mask<N>) -> usize {
    let mut remaining = nb;
    let mut fill = 0;
    for i in nb.indices() {
        remaining.remove(i);
        fill += remaining.difference_count(graph[i]);
    }
    fill
}
#[cfg(test)]
fn fill_count(graph: &[u64], nb: u64) -> usize {
    let graph: Vec<_> = graph.iter().map(|&w| Mask([w])).collect();
    fill_words(&graph, Mask([nb]))
}
#[derive(Default, Debug, PartialEq, Eq)]
struct Rejections {
    width: usize,
    work: usize,
    live: usize,
    both: usize,
    accepted: usize,
    aggregate_work: usize,
    aggregate_live: usize,
    aggregate_both: usize,
    min_rejected_width: Option<usize>,
    min_rejected_work: Option<usize>,
    min_rejected_live: Option<usize>,
}
fn record_min(slot: &mut Option<usize>, v: usize) {
    *slot = Some(slot.map_or(v, |old| old.min(v)));
}
fn plan(
    fs: &[Factor<R>],
    nodes: &Scope,
    policy: (Option<usize>, bool),
    retained: usize,
    d: Instant,
    rejected: &mut Rejections,
) -> Result<Option<Plan>, String> {
    if nodes.len() <= 64 {
        plan_words::<1>(fs, nodes, policy, retained, d, rejected)
    } else {
        plan_words::<4>(fs, nodes, policy, retained, d, rejected)
    }
}
fn plan_words<const N: usize>(
    fs: &[Factor<R>],
    nodes: &Scope,
    policy: (Option<usize>, bool),
    retained: usize,
    d: Instant,
    rejected: &mut Rejections,
) -> Result<Option<Plan>, String> {
    plan_words_impl::<N, true>(fs, nodes, policy, retained, d, rejected)
}
fn plan_words_impl<const N: usize, const MASK_PAIRS: bool>(
    fs: &[Factor<R>],
    nodes: &Scope,
    policy: (Option<usize>, bool),
    retained: usize,
    d: Instant,
    rejected: &mut Rejections,
) -> Result<Option<Plan>, String> {
    let mut scopes: BTreeMap<usize, Scope> = fs
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f.scope.iter().copied().collect()))
        .collect();
    let vars: Vec<_> = nodes.iter().copied().collect();
    if vars.len() > 64 * N || vars.len() > RESIDUAL_CAP {
        return Err("plan mask cap".into());
    }
    let mut graph = vec![Mask::<N>::empty(); vars.len()];
    let mut active = Mask::<N>::empty();
    for i in 0..vars.len() {
        active.insert(i);
    }
    for s in scopes.values() {
        let mut m = Mask::<N>::empty();
        for v in s {
            m.insert(vars.binary_search(v).map_err(|_| "plan owner")?);
        }
        for i in m.indices() {
            let mut nb = m;
            nb.remove(i);
            graph[i].union(nb);
        }
    }
    let mut live = fs.iter().map(|f| f.values.len()).sum::<usize>();
    let extra = retained + live;
    let mut cost = Cost {
        peak: live + extra,
        ..Cost::default()
    };
    let mut ops = Vec::new();
    let mut next = scopes.len();
    while active.count() != 0 {
        deadline(d)?;
        let i = active
            .indices()
            .min_by_key(|&i| {
                let nb = graph[i];
                let degree = nb.count();
                let fill = if policy.0.is_some() {
                    fill_words(&graph, nb)
                } else {
                    0
                };
                let v = vars[i];
                (
                    policy.0.map_or(degree, |p| fill + p * degree),
                    degree,
                    if policy.1 { usize::MAX - v } else { v },
                )
            })
            .ok_or("plan pivot")?;
        let v = vars[i];
        let neighbors = graph[i];
        let nb: Scope = neighbors.indices().map(|j| vars[j]).collect();
        if nb.len() > WIDTH {
            rejected.width += 1;
            record_min(&mut rejected.min_rejected_width, nb.len());
            return Ok(None);
        }
        cost.width = cost.width.max(nb.len());
        active.remove(i);
        graph[i] = Mask::empty();
        let mut bucket: Vec<_> = scopes
            .iter()
            .filter(|(_, s)| s.contains(&v))
            .map(|(&i, s)| (i, s.clone()))
            .collect();
        for (i, _) in &bucket {
            scopes.remove(i);
        }
        // One exact mask per bucket scope; pair ranking is unchanged. The
        // sidecar follows the original survivor order and append-only dst IDs.
        let mut pair_masks: Vec<Mask<N>> = Vec::new();
        if MASK_PAIRS && bucket.len() > 2 {
            for (_, s) in &bucket {
                let mut m = Mask::<N>::empty();
                for u in s {
                    m.insert(vars.binary_search(u).map_err(|_| "pair owner")?);
                }
                pair_masks.push(m);
            }
        }
        while bucket.len() > 2 {
            let mut best = None;
            for i in 0..bucket.len() {
                for j in i + 1..bucket.len() {
                    let n = if MASK_PAIRS {
                        pair_masks[i].union_count(pair_masks[j])
                    } else {
                        bucket[i].1.union(&bucket[j].1).count()
                    };
                    if n < nb.len() && best.is_none_or(|k| (n, i, j) < k) {
                        best = Some((n, i, j));
                    }
                }
            }
            let Some((n, i, j)) = best else {
                break;
            };
            let size = 1usize << n;
            if live + size + extra > LIVE {
                break;
            }
            let (ib, b) = bucket.remove(j);
            let (ia, a) = bucket.remove(i);
            if MASK_PAIRS {
                let b = pair_masks.remove(j);
                let mut a = pair_masks.remove(i);
                a.union(b);
                pair_masks.push(a);
            }
            let u: Scope = a.union(&b).copied().collect();
            cost.work += 2 * size;
            cost.peak = cost.peak.max(live + size + extra);
            cost.joins += 1;
            live = live + size - (1 << a.len()) - (1 << b.len());
            ops.push(Op {
                ids: vec![ia, ib],
                dst: next,
                out: u.iter().copied().collect(),
                pivot: None,
            });
            bucket.push((next, u));
            next += 1;
        }
        let size = 1usize << nb.len();
        cost.work += 2 * size * bucket.len();
        cost.peak = cost.peak.max(live + size + extra);
        if cost.work > WORK || cost.peak > LIVE {
            match (cost.work > WORK, cost.peak > LIVE) {
                (true, true) => rejected.both += 1,
                (true, false) => rejected.work += 1,
                (false, true) => rejected.live += 1,
                _ => unreachable!(),
            }
            if cost.work > WORK {
                record_min(&mut rejected.min_rejected_work, cost.work);
            }
            if cost.peak > LIVE {
                record_min(&mut rejected.min_rejected_live, cost.peak);
            }
            return Ok(None);
        }
        live = live + size - bucket.iter().map(|(_, s)| 1usize << s.len()).sum::<usize>();
        ops.push(Op {
            ids: bucket.iter().map(|(i, _)| *i).collect(),
            dst: next,
            out: nb.iter().copied().collect(),
            pivot: Some(v),
        });
        scopes.insert(next, nb.clone());
        next += 1;
        for j in neighbors.indices() {
            graph[j].remove(i);
            let mut fill = neighbors;
            fill.remove(j);
            graph[j].union(fill);
        }
    }
    rejected.accepted += 1;
    Ok(Some(Plan { ops, cost, extra }))
}
type Selected = (Vec<usize>, Vec<Plan>, Cost);
// Bounded deterministic extension: only the degree-ranked top 12 variables
// are eligible for three cuts (at most 220 candidates / eight leaves each).
// Width, aggregate work, peak and shared deadline checks remain unchanged.
fn triples(high: &[usize]) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    for (i, &a) in high.iter().enumerate() {
        for (j, &b) in high.iter().enumerate().skip(i + 1) {
            for &c in high.iter().skip(j + 1) {
                out.push(vec![a, b, c]);
            }
        }
    }
    out
}

fn choose(
    fs: &[Factor<R>],
    nodes: &Scope,
    scalar: &Cell<R>,
    local: &Cost,
    d: Instant,
) -> Result<Selected, String> {
    let three = std::env::var("RAWMOCS_RESEARCH_THREE_CUT").as_deref() == Ok("1");
    let base = choose_search(fs, nodes, scalar, local, d, None, three, false)?;
    // Explicit branch-only ablation. Never relax the existing hard budgets.
    // Explore peak-first leaves and zero/one cuts at <=125% of base work.
    let mode = std::env::var("RAWMOCS_RESEARCH_PEAK_FIRST").unwrap_or_default();
    let gated = mode == "2";
    // The low-cost mode only considers high-cost zero-cut baseline plans.
    // Thresholds and shortlist length are research hypotheses, not defaults.
    if base.0.is_empty()
        && (mode == "1" || (gated && base.2.peak >= 262_144 && base.2.work >= 1_000_000))
    {
        let max_work = if gated {
            base.2.work
        } else {
            WORK.min(base.2.work.saturating_mul(5) / 4)
        };
        let max_peak = if gated { base.2.peak / 2 + 1 } else { base.2.peak };
        match choose_search(fs, nodes, scalar, local, d, Some((max_work, max_peak)), false, gated) {
            Ok(candidate) => {
                eprintln!("PREJOIN_RESEARCH_PEAK base={} candidate={} base_work={} candidate_work={} gated={gated} cuts={:?}",
                          base.2.peak, candidate.2.peak, base.2.work, candidate.2.work, candidate.0);
                return Ok(candidate);
            }
            Err(e) if e == "no bounded prejoin plan" || e == "prejoin deadline" => {
                eprintln!("PREJOIN_RESEARCH_PEAK no_improvement reason={e}");
            }
            Err(e) => return Err(e),
        }
    }
    if gated {
        eprintln!("PREJOIN_RESEARCH_GATE baseline_cuts={} peak={} work={} retained_baseline=true", base.0.len(), base.2.peak, base.2.work);
    }
    Ok(base)
}

fn choose_search(
    fs: &[Factor<R>],
    nodes: &Scope,
    scalar: &Cell<R>,
    local: &Cost,
    d: Instant,
    memory_budget: Option<(usize, usize)>,
    three: bool,
    shortlist: bool,
) -> Result<Selected, String> {
    let retained = fs.iter().map(|f| f.values.len()).sum::<usize>();
    let mut high: Vec<_> = nodes.iter().copied().collect();
    high.sort_by_key(|v| {
        let nb: Scope = fs
            .iter()
            .filter(|f| f.scope.contains(v))
            .flat_map(|f| f.scope.iter().copied())
            .filter(|u| u != v)
            .collect();
        (usize::MAX - nb.len(), *v)
    });
    high.truncate(12);
    let mut memory_best: Option<Selected> = None;
    for n in 0..=if memory_budget.is_some() { 1 } else if three { 3 } else { 2 } {
        let candidates: Vec<Vec<usize>> = match n {
            0 => vec![vec![]],
            1 if shortlist => high.iter().take(8).map(|&v| vec![v]).collect(),
            1 => nodes.iter().map(|&v| vec![v]).collect(),
            2 => high
                .iter()
                .enumerate()
                .flat_map(|(i, &v)| high.iter().skip(i + 1).map(move |&u| vec![v, u]))
                .collect(),
            _ => triples(&high),
        };
        let mut best: Option<Selected> = None;
        let attempted = candidates.len();
        let mut admitted = 0usize;
        let mut rejected = Rejections::default();
        for cuts in candidates {
            deadline(d)?;
            let ns: Scope = nodes
                .iter()
                .copied()
                .filter(|v| !cuts.contains(v))
                .collect();
            let mut plans = Vec::new();
            let mut cost = local.clone();
            for m in 0..1usize << cuts.len() {
                let (f, _) = restrict(fs, scalar, &cuts, m)?;
                let mut options = Vec::new();
                for p in [Some(0), Some(4), None] {
                    for reverse in [true, false] {
                        if let Some(x) = plan(&f, &ns, (p, reverse), retained, d, &mut rejected)? {
                            options.push(x);
                        }
                    }
                }
                let min_work = options.iter().map(|x| x.cost.work).min();
                let leaf = if memory_budget.is_some() {
                    options.into_iter()
                        .filter(|x| min_work.is_some_and(|w| x.cost.work <= w.saturating_mul(5) / 4))
                        .min_by_key(|x| (x.cost.peak, x.cost.work, x.cost.width))
                } else {
                    options.into_iter().min_by_key(|x| (x.cost.work, x.cost.peak, x.cost.width))
                };
                let Some(x) = leaf else {
                    break;
                };
                cost.work += x.cost.work + retained;
                cost.peak = cost.peak.max(x.cost.peak);
                cost.width = cost.width.max(x.cost.width);
                cost.joins += x.cost.joins;
                plans.push(x);
            }
            if plans.len() == 1 << cuts.len() {
                match (cost.work > WORK, cost.peak > LIVE) {
                    (true, true) => rejected.aggregate_both += 1,
                    (true, false) => rejected.aggregate_work += 1,
                    (false, true) => rejected.aggregate_live += 1,
                    _ => (),
                }
            }
            if plans.len() == 1 << cuts.len() && cost.work <= WORK && cost.peak <= LIVE {
                admitted += 1;
                if let Some((max_work, max_peak)) = memory_budget {
                    if cost.work <= max_work && cost.peak < max_peak
                        && memory_best.as_ref().is_none_or(|b|
                            (cost.peak, cost.work, cost.width, &cuts)
                                < (b.2.peak, b.2.work, b.2.width, &b.0))
                    {
                        memory_best = Some((cuts, plans, cost));
                    }
                } else if best.as_ref().is_none_or(|b|
                    (cost.work, cost.peak, cost.width, &cuts)
                        < (b.2.work, b.2.peak, b.2.width, &b.0))
                {
                    best = Some((cuts, plans, cost));
                }
            }
        }
        eprintln!("PREJOIN_RESEARCH_LEVEL cuts={n} attempted={attempted} admitted={admitted} memory_mode={} shortlist={shortlist} leaf_rejections={rejected:?}", memory_budget.is_some());
        if let Some(b) = best {
            return Ok(b);
        }
    }
    memory_best.ok_or_else(|| "no bounded prejoin plan".into())
}
fn mpfr(x: Cell<R>, bits: u32) -> Cell<C> {
    x.map(|(a, b)| C {
        re: Float::with_val(bits, a),
        im: Float::with_val(bits, b),
    })
}
struct Scratch([Float; 4]);
impl Scratch {
    fn new(bits: u32) -> Self {
        Self(std::array::from_fn(|_| Float::new(bits)))
    }
    fn multiply(&mut self, a: &mut C, b: &C) {
        if (a.re == 0 && a.im == 0) || (b.re == 0 && b.im == 0) {
            a.re.assign(0);
            a.im.assign(0);
            return;
        }
        if b.re == 1 && b.im == 0 {
            return;
        }
        if a.re == 1 && a.im == 0 {
            a.re.assign(&b.re);
            a.im.assign(&b.im);
            return;
        }
        let [ac, bd, ad, bc] = &mut self.0;
        ac.assign(&a.re * &b.re);
        bd.assign(&a.im * &b.im);
        ad.assign(&a.re * &b.im);
        bc.assign(&a.im * &b.re);
        a.re.assign(&*ac - &*bd);
        a.im.assign(&*ad + &*bc);
    }
}
// Reuse only dead table entries, at the same precision. Support remains
// Option<C>; a numeric zero must never become an absent entry.
fn recycle_copy(pool: &mut Vec<C>, x: &C) -> C {
    if let Some(mut y) = pool.pop() {
        y.re.assign(&x.re);
        y.im.assign(&x.im);
        y
    } else {
        x.clone()
    }
}
// The dead pool owns independent same-precision allocations. Transfer the
// accumulator into the output and reuse the dead value as scratch. The next
// branch overwrites both components before reading them. Empty pool retains
// exactly the previous clone path; no precision or operation-order changes.
fn recycle_result(pool: &mut Vec<C>, x: &mut C, move_result: bool) -> C {
    if move_result {
        if let Some(mut y) = pool.pop() {
            debug_assert_eq!(y.re.prec(), x.re.prec());
            debug_assert_eq!(y.im.prec(), x.im.prec());
            std::mem::swap(&mut y, x);
            return y;
        }
        return x.clone();
    }
    recycle_copy(pool, x)
}
fn cadd(a: Cell<C>, b: Cell<C>) -> Cell<C> {
    match (a, b) {
        (Some(mut a), Some(b)) => {
            a.re += &b.re;
            a.im += &b.im;
            Some(a)
        }
        (a, None) | (None, a) => a,
    }
}
fn execute(
    fs: Vec<Factor<R>>,
    scalar: Cell<R>,
    p: &Plan,
    bits: u32,
    d: Instant,
) -> Result<Cell<C>, String> {
    let mut tables: BTreeMap<usize, Factor<C>> = fs
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            (
                i,
                Factor {
                    scope: f.scope,
                    values: f.values.into_iter().map(|x| mpfr(x, bits)).collect(),
                },
            )
        })
        .collect();
    let mut live = tables.values().map(|f| f.values.len()).sum::<usize>();
    let mut cost = Cost {
        peak: live + p.extra,
        ..Cost::default()
    };
    let mut scratch = Scratch::new(bits);
    // Bound additional retained cells by unused certified peak headroom.
    // The original replay cost remains unchanged; the pool is extra storage.
    let recycle_cap = 32_768.min(LIVE.saturating_sub(p.cost.peak));
    let mut recycle = Vec::new();
    let move_result = std::env::var("RAWMOCS_RESEARCH_RECYCLE_MOVE").as_deref() == Ok("1");
    let (mut left, mut right) = (C::one(bits), C::one(bits));
    for op in &p.ops {
        deadline(d)?;
        let bucket: Vec<_> = op
            .ids
            .iter()
            .map(|i| tables.remove(i).ok_or("execution owner"))
            .collect::<Result<_, _>>()?;
        let size = 1usize << op.out.len();
        cost.peak = cost.peak.max(live + size + p.extra);
        if op.pivot.is_some() {
            cost.width = cost.width.max(op.out.len());
            cost.work += 2 * size * bucket.len();
        } else {
            cost.joins += 1;
            cost.work += 2 * size;
        }
        if cost.work > WORK || cost.peak > LIVE || cost.width > WIDTH {
            return Err("execution cap".into());
        }
        let mut indices = vec![0; bucket.len()];
        let mut pivots = vec![0; bucket.len()];
        let mut flips = Vec::new();
        for (i, f) in bucket.iter().enumerate() {
            let mut b = vec![0usize; op.out.len()];
            for (j, &u) in f.scope.iter().enumerate() {
                if Some(u) == op.pivot {
                    pivots[i] = 1 << j;
                } else {
                    b[op.out.binary_search(&u).map_err(|_| "execution scope")?] = 1 << j;
                }
            }
            for j in 1..b.len() {
                b[j] ^= b[j - 1];
            }
            flips.push(b);
        }
        let mut values = Vec::with_capacity(size);
        for m in 0..size {
            if m % 4096 == 0 {
                deadline(d)?;
            }
            let mut branch = |positive: bool, x: &mut C| {
                x.re.assign(1);
                x.im.assign(0);
                for (i, f) in bucket.iter().enumerate() {
                    let Some(y) = &f.values[indices[i] | if positive { pivots[i] } else { 0 }]
                    else {
                        return false;
                    };
                    scratch.multiply(x, y);
                }
                true
            };
            let a = branch(false, &mut left);
            let b = op.pivot.is_some() && branch(true, &mut right);
            values.push(match (a, b) {
                (true, true) => {
                    left.re += &right.re;
                    left.im += &right.im;
                    Some(recycle_result(&mut recycle, &mut left, move_result))
                }
                (true, false) => Some(recycle_result(&mut recycle, &mut left, move_result)),
                (false, true) => Some(recycle_result(&mut recycle, &mut right, move_result)),
                _ => None,
            });
            if m + 1 < size {
                let bit = (m + 1).trailing_zeros() as usize;
                for (i, x) in indices.iter_mut().enumerate() {
                    *x ^= flips[i][bit];
                }
            }
        }
        live = live + size - bucket.iter().map(|f| f.values.len()).sum::<usize>();
        // No references to consumed factors survive the operation.
        // Do not scan an entire dead table once the bounded pool is full.
        for f in bucket {
            if recycle.len() == recycle_cap {
                break;
            }
            for x in f.values.into_iter().flatten() {
                recycle.push(x);
                if recycle.len() == recycle_cap {
                    break;
                }
            }
        }
        tables.insert(
            op.dst,
            Factor {
                scope: op.out.clone(),
                values,
            },
        );
    }
    if cost != p.cost {
        return Err("plan replay drift".into());
    }
    let mut out = mpfr(scalar, bits);
    for f in tables.into_values() {
        if !f.scope.is_empty() || f.values.len() != 1 {
            return Err("uneliminated".into());
        }
        out = match (out, f.values.into_iter().next().unwrap()) {
            (Some(mut a), Some(b)) => {
                scratch.multiply(&mut a, &b);
                Some(a)
            }
            _ => None,
        };
    }
    Ok(out)
}
#[cfg(test)]
pub(crate) fn solve(p: &Problem, raw: &str, bits: u32) -> Result<Outcome, String> {
    solve_until(p, raw, bits, Instant::now() + Duration::from_secs(45))
}
pub(crate) fn solve_until(
    p: &Problem,
    raw: &str,
    bits: u32,
    d: Instant,
) -> Result<Outcome, String> {
    let start = Instant::now();
    let planning_budget = (d.saturating_duration_since(start) * 2 / 3).min(Duration::from_secs(30));
    let pd = start + planning_budget;
    let planning_error = |e: String| {
        if e == "prejoin deadline" && Instant::now() < d {
            "prejoin planning budget".into()
        } else {
            e
        }
    };
    let (fs, nodes, scalar, local) = local(p, raw, pd).map_err(planning_error)?;
    let local_ns = start.elapsed().as_nanos();
    let phase = Instant::now();
    let (cuts, plans, cost) = choose(&fs, &nodes, &scalar, &local, pd).map_err(planning_error)?;
    let choose_ns = phase.elapsed().as_nanos();
    let plan_ns = start.elapsed().as_nanos();
    let mut out = None;
    let mut restrict_ns = 0;
    let mut execute_ns = 0;
    for (m, plan) in plans.iter().enumerate() {
        let phase = Instant::now();
        let (f, s) = restrict(&fs, &scalar, &cuts, m)?;
        restrict_ns += phase.elapsed().as_nanos();
        let phase = Instant::now();
        out = cadd(out, execute(f, s, plan, bits, d)?);
        execute_ns += phase.elapsed().as_nanos();
    }
    eprintln!("PREJOIN_PHASE_NS local={local_ns} choose={choose_ns} restrict={restrict_ns} execute={execute_ns}");
    eprintln!("PREJOIN_COMPLETE residual={} cuts={} leaves={} width={} work={} peak={} joins={} plan_ns={}",nodes.len(),cuts.len(),plans.len(),cost.width,cost.work,cost.peak,cost.joins,plan_ns);
    match out {
        None => Ok(Outcome::unsat(bits)),
        Some(count) if count.re.is_finite() && count.im.is_finite() => {
            Ok(Outcome { sat: true, count })
        }
        _ => Err("nonfinite prejoin".into()),
    }
}
#[cfg(test)]
const PAIR_PROBE_CAP: usize = 512;
#[cfg(test)]
fn pair_scan(raw: &str, use_mask: bool) -> Result<String, String> {
    let mut lines = raw.lines();
    let head: Vec<_> = lines
        .next()
        .ok_or("pair header")?
        .split_whitespace()
        .collect();
    if head.len() != 5 || head[0] != "PAIR_SNAPSHOT_V1" {
        return Err("pair header".into());
    }
    let nums = head[1..]
        .iter()
        .map(|s| s.parse::<usize>().map_err(|_| "pair integer"))
        .collect::<Result<Vec<_>, _>>()?;
    let (pivot, width, full, count) = (nums[0], nums[1], nums[2], nums[3]);
    if width > WIDTH || count < 3 || count != full.min(PAIR_PROBE_CAP) {
        return Err("pair snapshot cap".into());
    }
    let scopes: Vec<Scope> = lines
        .map(|l| {
            l.split_whitespace()
                .map(|s| s.parse::<usize>().map_err(|_| "pair scope integer"))
                .collect::<Result<Scope, _>>()
        })
        .collect::<Result<_, _>>()?;
    if scopes.len() != count
        || scopes
            .iter()
            .any(|s| s.len() > WIDTH + 1 || !s.contains(&pivot))
    {
        return Err("pair scope drift".into());
    }
    let vars: Vec<_> = scopes
        .iter()
        .flatten()
        .copied()
        .collect::<Scope>()
        .into_iter()
        .collect();
    if vars.len() > 64 {
        return Err("pair probe single-word cap".into());
    }
    let mut masks = Vec::new();
    if use_mask {
        for s in &scopes {
            let mut m = Mask::<1>::empty();
            for v in s {
                m.insert(vars.binary_search(v).map_err(|_| "pair mask index")?);
            }
            masks.push(m);
        }
    }
    let mut best = None;
    let mut sum = 0usize;
    for i in 0..count {
        for j in i + 1..count {
            let n = if use_mask {
                masks[i].union_count(masks[j])
            } else {
                scopes[i].union(&scopes[j]).count()
            };
            sum += n;
            if n < width && best.is_none_or(|k| (n, i, j) < k) {
                best = Some((n, i, j));
            }
        }
    }
    Ok(format!("PAIR_SCAN_V1 pivot={pivot} width={width} full={full} sampled={count} pairs={} cardinality_sum={sum} best={best:?}",count*(count-1)/2))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recycled_complex_copy_matches_clone_including_signed_zero() {
        for bits in [100, 333] {
            let mut pool = vec![C::one(bits), C::one(bits)];
            for x in [
                C {
                    re: Float::with_val(bits, -0.0),
                    im: Float::with_val(bits, 0),
                },
                C {
                    re: Float::with_val(bits, decimal("0.12345678901234567890123456789").unwrap()),
                    im: Float::with_val(bits, decimal("-1e-80").unwrap()),
                },
                C::one(bits),
            ] {
                let y = recycle_copy(&mut pool, &x);
                assert_eq!(y.re, x.re);
                assert_eq!(y.im, x.im);
                assert_eq!(y.re.prec(), bits);
                assert_eq!(y.im.prec(), bits);
                assert_eq!(y.re.is_sign_negative(), x.re.is_sign_negative());
                assert_eq!(y.im.is_sign_negative(), x.im.is_sign_negative());
            }
            assert!(pool.is_empty());
        }
    }
    #[test]
    fn recycle_move_preserves_precision_values_sign_and_independence() {
        for bits in [100, 200, 333, 432] {
            let mut pool = vec![C::one(bits), C::one(bits)];
            for mut x in [
                C {
                    re: Float::with_val(bits, -0.0),
                    im: Float::with_val(bits, 0),
                },
                C {
                    re: Float::with_val(bits, decimal("0.12345678901234567890123456789").unwrap()),
                    im: Float::with_val(bits, decimal("-1e-80").unwrap()),
                },
                C {
                    re: Float::with_val(bits, 0),
                    im: Float::with_val(bits, -0.0),
                },
                C::one(bits),
            ] {
                let reference = x.clone();
                let previous_len = pool.len();
                let y = recycle_result(&mut pool, &mut x, true);
                assert_eq!(pool.len(), previous_len.saturating_sub(1));
                assert_eq!(y.re, reference.re);
                assert_eq!(y.im, reference.im);
                assert_eq!(y.re.prec(), bits);
                assert_eq!(y.im.prec(), bits);
                assert_eq!(x.re.prec(), bits);
                assert_eq!(x.im.prec(), bits);
                assert_eq!(y.re.is_sign_negative(), reference.re.is_sign_negative());
                assert_eq!(y.im.is_sign_negative(), reference.im.is_sign_negative());
                // The original accumulator is immediately reset and reused.
                x.re.assign(1);
                x.im.assign(0);
                assert_eq!(y.re, reference.re);
                assert_eq!(y.im, reference.im);
                assert_eq!(y.re.is_sign_negative(), reference.re.is_sign_negative());
                assert_eq!(y.im.is_sign_negative(), reference.im.is_sign_negative());
            }
            assert!(pool.is_empty());
        }
    }
    #[test]
    fn multiword_plan_matches_single_word_and_crosses_boundaries() {
        let d = Instant::now() + Duration::from_secs(10);
        let make = |n: usize| -> Vec<Factor<R>> {
            (1..n)
                .map(|i| Factor {
                    scope: vec![i, i + 1],
                    values: vec![Some(one()); 4],
                })
                .collect()
        };
        for n in [1, 5, 64] {
            let fs = make(n);
            let nodes: Scope = (1..=n).collect();
            for p in [Some(0), Some(4), None] {
                for reverse in [true, false] {
                    assert_eq!(
                        plan_words::<1>(&fs, &nodes, (p, reverse), 0, d, &mut Rejections::default()).unwrap(),
                        plan_words::<4>(&fs, &nodes, (p, reverse), 0, d, &mut Rejections::default()).unwrap()
                    );
                }
            }
        }
        let mut m = Mask::<4>::empty();
        for i in [0, 63, 64, 127, 128, 191, 192, 255] {
            m.insert(i);
        }
        assert_eq!(
            m.indices().collect::<Vec<_>>(),
            vec![0, 63, 64, 127, 128, 191, 192, 255]
        );
        m.remove(64);
        assert_eq!(m.count(), 7);
        let g = vec![Mask::<4>::empty(); 256];
        assert_eq!(fill_words(&g, m), 21);
        let n = 170;
        let fs = make(n);
        let nodes: Scope = (1..=n).collect();
        let p = plan(&fs, &nodes, (Some(0), false), 0, d, &mut Rejections::default()).unwrap().unwrap();
        assert_eq!(p.ops.iter().filter(|op| op.pivot.is_some()).count(), n);
        assert_eq!(p.cost.width, 1);
        assert!(plan_words::<1>(&fs, &nodes, (Some(0), false), 0, d, &mut Rejections::default()).is_err());
    }
    #[test]
    fn counted_edges_preserve_shared_support() {
        let mut edges = vec![BTreeMap::new(); 5];
        edge_add(&mut edges, &[1, 2, 3]);
        edge_add(&mut edges, &[1, 2, 4]);
        assert_eq!(edges[1][&2], 2);
        edge_remove(&mut edges, &[1, 2, 3]).unwrap();
        assert_eq!(edges[1][&2], 1);
        assert!(!edges[1].contains_key(&3));
        assert!(edges[3].is_empty());
        edge_remove(&mut edges, &[1, 2, 4]).unwrap();
        assert!(edges.iter().all(BTreeMap::is_empty));
    }
    #[test]
    fn local_probe_preserves_complete_frontier_and_can_stop_early() {
        let raw = "p cnf 6 3\nc t wmc\nc p weight 1 -2 3 0\n1 2 0\n-2 3 0\n4 5 0\n";
        let p = parse_emitted(raw, 200).unwrap();
        let d = Instant::now() + Duration::from_secs(5);
        let full = local_frontier(&p, raw, d, true, true).unwrap();
        assert_eq!(
            full,
            local_frontier_probe::<true, true>(&p, raw, d, true, true, None).unwrap()
        );
        assert_eq!(
            full,
            local_frontier_probe::<false, false>(&p, raw, d, true, true, None).unwrap()
        );
        assert_eq!(
            full,
            local_frontier_probe::<false, true>(&p, raw, d, true, true, Some(6)).unwrap()
        );
        let prefix = local_frontier_probe::<false, true>(&p, raw, d, true, true, Some(2)).unwrap();
        assert_eq!(prefix.1.len(), 4);
        assert!(prefix.3.work <= full.3.work);
    }
    #[test]
    fn bounded_pair_scan_preserves_exact_pair_choice() {
        let raw = "PAIR_SNAPSHOT_V1 1 4 4 4\n1 2\n1 3\n1 2\n1 3 4\n";
        let a = pair_scan(raw, false).unwrap();
        let b = pair_scan(raw, true).unwrap();
        assert_eq!(a, b);
        assert!(a.contains("pairs=6"));
        assert!(a.contains("best=Some((2, 0, 2))"));
        assert!(pair_scan("PAIR_SNAPSHOT_V1 1 4 4 3\n1 2\n1 3\n1 2\n", true).is_err());
    }
    #[test]
    fn pair_mask_plans_match_ordered_set_reference() {
        // Full ordered plan, not just aggregate work. Duplicates and dense
        // buckets exercise repeated merges, tie breaks and appended masks.
        for n in [9usize, 70] {
            let nodes: Scope = (1..=n).collect();
            for seed in 0..12usize {
                let mut fs = Vec::new();
                for j in 0..(n * 2) {
                    let scope: Vec<_> = [
                        1 + (j + seed) % n,
                        1 + (j + 1 + seed) % n,
                        1 + (j + 3 + seed) % n,
                    ]
                    .into_iter()
                    .collect::<Scope>()
                    .into_iter()
                    .collect();
                    fs.push(Factor {
                        values: vec![Some(one()); 1 << scope.len()],
                        scope,
                    });
                }
                let d = Instant::now() + Duration::from_secs(10);
                for p in [Some(0), Some(4), None] {
                    for reverse in [true, false] {
                        let a =
                            plan_words_impl::<4, false>(&fs, &nodes, (p, reverse), 0, d, &mut Rejections::default()).unwrap();
                        let b =
                            plan_words_impl::<4, true>(&fs, &nodes, (p, reverse), 0, d, &mut Rejections::default()).unwrap();
                        assert_eq!(a, b);
                        if n <= 64 {
                            assert_eq!(
                                a,
                                plan_words_impl::<1, true>(&fs, &nodes, (p, reverse), 0, d, &mut Rejections::default())
                                    .unwrap()
                            );
                        }
                    }
                }
            }
        }
        let mut a = Mask::<4>::empty();
        let mut b = Mask::<4>::empty();
        for i in [0, 63, 64, 129, 255] {
            a.insert(i);
        }
        for i in [63, 65, 129, 200, 255] {
            b.insert(i);
        }
        assert_eq!(a.union_count(b), 7);
    }
    #[test]
    fn mask_fill_matches_set_reference() {
        // Every undirected graph on five vertices, every neighborhood.
        for edges in 0..1024u64 {
            let mut graph = [0u64; 5];
            let mut bit = 0;
            for i in 0..5 {
                for j in i + 1..5 {
                    if edges & (1 << bit) != 0 {
                        graph[i] |= 1 << j;
                        graph[j] |= 1 << i;
                    }
                    bit += 1;
                }
            }
            for nb in 0..32u64 {
                let mut expected = 0;
                for (i, &neighbors) in graph.iter().enumerate() {
                    for j in i + 1..5 {
                        if nb & (1 << i) != 0 && nb & (1 << j) != 0 && neighbors & (1 << j) == 0 {
                            expected += 1;
                        }
                    }
                }
                assert_eq!(fill_count(&graph, nb), expected);
            }
        }
        assert_eq!(
            mask_indices(u64::MAX).collect::<Vec<_>>(),
            (0..64).collect::<Vec<_>>()
        );
        assert_eq!(fill_count(&[0u64; 64], u64::MAX), 2016);
    }
    #[test]
    fn exact_rational_identity_shortcuts_match_full_formula() {
        let values = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (2, 3), (-3, 2)];
        let mut x: Vec<R> = values
            .into_iter()
            .map(|(r, i)| (Rational::from(r), Rational::from(i)))
            .collect();
        x.push((
            decimal("-1.0000000000000000").unwrap(),
            decimal("1.2246467991473532e-16").unwrap(),
        ));
        x.push((
            decimal("0.12345678901234567890123456789").unwrap(),
            decimal("-1e-80").unwrap(),
        ));
        x.push((decimal("1e80").unwrap(), decimal("1e-80").unwrap()));
        for v in [
            "2",
            "-3",
            "0.125",
            "-0.1234567890123456789",
            "1e80",
            "1e-80",
        ] {
            let q = decimal(v).unwrap();
            x.push((q.clone(), Rational::from(0)));
            x.push((Rational::from(0), q));
        }
        for a in &x {
            for b in &x {
                assert_eq!(rmul(a, b).unwrap(), rmul_full(a, b).unwrap());
            }
        }
        // A supported exact zero is still Some, not Boolean impossibility.
        let z = Some(rmul(&x[0], &x[7]).unwrap());
        assert!(z.is_some());
        assert_eq!(z, Some((Rational::from(0), Rational::from(0))));
        assert_eq!(rscalar(None, Some(one())).unwrap(), None);
    }
    #[test]
    fn incremental_frontier_matches_full_scan() {
        // Deterministic weighted fixtures include free axes, duplicate
        // factors, cancellation and dense residual scopes.
        for seed in 0..48usize {
            let n = 8;
            let mut raw = format!("p cnf {n} 7\nc t wmc\n");
            for v in 1..=n {
                raw.push_str(&format!(
                    "c p weight {v} {} {} 0\n",
                    (v + seed) % 5,
                    (v + 2 * seed) % 3
                ));
            }
            for j in 0..7 {
                for k in 0..(2 + (j + seed) % 5) {
                    let v = 1 + (seed + j + 3 * k) % n;
                    let l = if (seed + j + k) % 2 == 0 {
                        v as i32
                    } else {
                        -(v as i32)
                    };
                    raw.push_str(&format!("{l} "));
                }
                raw.push_str("0\n");
            }
            let p = parse_emitted(&raw, 100).unwrap();
            let d = Instant::now() + Duration::from_secs(5);
            assert_eq!(
                local_frontier(&p, &raw, d, false, false).unwrap(),
                local_frontier(&p, &raw, d, true, false).unwrap()
            );
        }
        // More than 256 variables with degree five, each in a six-variable
        // clause; the old frontier stops, adaptive width six eliminates.
        let n = 258;
        let mut raw = format!("p cnf {n} {}\nc t wmc\n", n / 6);
        for j in (1..=n).step_by(6) {
            for v in j..j + 6 {
                raw.push_str(&format!("{v} "));
            }
            raw.push_str("0\n");
        }
        let p = parse_emitted(&raw, 100).unwrap();
        let d = Instant::now() + Duration::from_secs(5);
        assert!(local_frontier(&p, &raw, d, true, false).is_err());
        let a = local_frontier(&p, &raw, d, true, true).unwrap();
        let b = local_frontier(&p, &raw, d, false, true).unwrap();
        assert_eq!(a, b);
        assert!(a.1.is_empty());
        assert_eq!(a.3.width, 5);
    }
    #[test]
    fn exact_local_support_and_free_axes() {
        for bits in [100, 333] {
            for (raw, sat, re, im) in [
                ("p cnf 3 0\nc t wmc\n", true, 8, 0),
                ("p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n", true, 0, 0),
                ("p cnf 1 2\nc t wmc\n1 0\n-1 0\n", false, 0, 0),
                (
                    "p cnf 2 1\nc t wmc\nc p weight 1 -2 3 0\n1 2 0\n",
                    true,
                    -3,
                    6,
                ),
            ] {
                let p = parse_emitted(raw, bits).unwrap();
                let a = solve(&p, raw, bits).unwrap();
                assert_eq!(a.sat, sat);
                assert_eq!(a.count.re, re);
                assert_eq!(a.count.im, im);
            }
        }
    }
    #[test]
    fn width_rejection_is_distinct_from_work_and_live() {
        let nodes: Scope = (1..=21).collect();
        let fs: Vec<_> = (1..=21)
            .flat_map(|a| (a + 1..=21).map(move |b| Factor {
                scope: vec![a, b],
                values: vec![Some(one()); 4],
            }))
            .collect();
        let mut rejected = Rejections::default();
        let d = Instant::now() + Duration::from_secs(5);
        assert!(plan(&fs, &nodes, (Some(0), false), 0, d, &mut rejected)
            .unwrap().is_none());
        assert_eq!(rejected.width, 1);
        assert_eq!(rejected.min_rejected_width, Some(20));
        assert_eq!(rejected.work + rejected.live + rejected.both + rejected.accepted, 0);
    }
    #[test]
    fn three_cut_candidates_are_unique_and_bounded() {
        let ranked: Vec<_> = (0..12).rev().collect();
        let got = triples(&ranked);
        assert_eq!(got.len(), 220);
        assert_eq!(got.iter().collect::<BTreeSet<_>>().len(), 220);
        assert_eq!(got[0], vec![11, 10, 9]);
        assert!(got.iter().all(|c| c.len() == 3 && c[0] > c[1] && c[1] > c[2]));
        assert!(triples(&[1, 2]).is_empty());
    }
    #[test]
    fn bounded_prejoin_replay_matches_exact_enumeration() {
        // Dense scope forces the interval executor rather than local elimination.
        let raw="p cnf 6 2\nc t wmc\nc p weight 1 -2 3 0\nc p weight -3 1 -1 0\n1 2 3 4 5 6 0\n-1 -2 3 -4 5 -6 0\n";
        for bits in [100, 333] {
            let p = parse_emitted(raw, bits).unwrap();
            let a = solve(&p, raw, bits).unwrap();
            let mut expected = Outcome::unsat(bits);
            for m in 0..64 {
                let yes = |l: i32| (m & (1 << (l.unsigned_abs() - 1)) != 0) == (l > 0);
                if p.clauses.iter().all(|c| c.iter().copied().any(yes)) {
                    expected.sat = true;
                    let mut x = C::one(bits);
                    for (i, w) in p.weights.iter().enumerate() {
                        x = x.mul(
                            if m & (1 << i) != 0 {
                                &w.positive
                            } else {
                                &w.negative
                            },
                            bits,
                        );
                    }
                    expected.count = expected.count.add(&x, bits);
                }
            }
            assert_eq!(a.sat, expected.sat);
            assert_eq!(a.count.re, expected.count.re);
            assert_eq!(a.count.im, expected.count.im);
        }
    }
}
