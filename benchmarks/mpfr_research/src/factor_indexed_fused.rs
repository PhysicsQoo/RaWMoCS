//! Opt-in, bounded dense indexed fused sum-product. Original Trail is the
//! fallback on any cap. Unit closure owns each forced representative once.
use crate::{factor_variable_elimination::VeStats, C, Clause, Outcome, Problem};
use std::collections::BTreeSet;
use std::time::Instant;

const CELLS: usize = 300_000;
const LOOKUPS: usize = 12_000_000;
const WIDTH: usize = 17; // separator size, not bag size

#[derive(Clone)]
enum Cell { No, One, Zero, Value(C) }
impl Cell {
    fn new(c: C) -> Self {
        if c.re == 0 && c.im == 0 { Self::Zero }
        else if c.re == 1 && c.im == 0 { Self::One }
        else { Self::Value(c) }
    }
    fn sat(&self) -> bool { !matches!(self, Self::No) }
    fn value(&self, bits: u32) -> C {
        match self {
            Self::No | Self::Zero => C::zero(bits),
            Self::One => C::one(bits),
            Self::Value(c) => c.clone(),
        }
    }
    fn mul(&self, rhs: &Self, bits: u32) -> Self {
        match (self, rhs) {
            (Self::No, _) | (_, Self::No) => Self::No,
            (Self::Zero, _) | (_, Self::Zero) => Self::Zero,
            (Self::One, x) | (x, Self::One) => x.clone(),
            (Self::Value(a), Self::Value(b)) => Self::new(a.mul(b, bits)),
        }
    }
    fn add(&self, rhs: &Self, bits: u32) -> Self {
        match (self, rhs) {
            (Self::No, x) | (x, Self::No) => x.clone(),
            (Self::Zero, x) | (x, Self::Zero) => x.clone(),
            _ => Self::new(self.value(bits).add(&rhs.value(bits), bits)),
        }
    }
}
struct Factor { vars: Vec<usize>, values: Vec<Cell> }
fn deadline(start: Instant) -> Result<(), &'static str> {
    if start.elapsed().as_secs() >= 8 { Err("indexed_deadline") } else { Ok(()) }
}
fn order(scope: &[usize], clauses: &[Clause], n: usize, start: Instant, stats: &mut VeStats, peak_bag: &mut Vec<usize>)
    -> Result<Vec<usize>, &'static str> {
    let mut adj = vec![BTreeSet::<usize>::new(); n+1];
    let mut active: BTreeSet<_> = scope.iter().copied().collect();
    for c in clauses {
        let vs: BTreeSet<_> = c.iter().map(|l| l.unsigned_abs() as usize).collect();
        for &v in &vs { adj[v].extend(vs.iter().copied().filter(|&u| u != v)); }
    }
    let mut result = Vec::with_capacity(scope.len());
    while !active.is_empty() {
        deadline(start)?;
        let mut candidates: Vec<_> = active.iter().copied().collect();
        candidates.sort_unstable_by_key(|&v| (adj[v].len(), v));
        let v = candidates.into_iter().take(16).min_by_key(|&v| {
            let near: Vec<_> = adj[v].iter().copied().collect();
            let mut fill = 0;
            for (i, &a) in near.iter().enumerate() {
                for &b in &near[i+1..] { fill += usize::from(!adj[a].contains(&b)); }
            }
            (fill, near.len(), v)
        }).unwrap();
        let near = std::mem::take(&mut adj[v]);
        if near.len() + 1 > stats.width {
            stats.width = near.len() + 1;
            *peak_bag = near.iter().copied().chain(std::iter::once(v)).collect();
        }
        if near.len() > WIDTH { return Err("indexed_width"); }
        active.remove(&v);
        for &u in &near {
            adj[u].remove(&v);
            adj[u].extend(near.iter().copied().filter(|&w| w != u));
        }
        result.push(v);
    }
    Ok(result)
}
fn push(f: Factor, buckets: &mut [Vec<Factor>], rank: &[usize],
        scalar: &mut Cell, live: &mut usize, bits: u32) -> Result<(), &'static str> {
    if f.vars.is_empty() {
        *scalar = scalar.mul(&f.values[0], bits);
    } else {
        *live = live.saturating_add(f.values.len());
        if *live > CELLS { return Err("indexed_live_cells"); }
        let k = f.vars.iter().map(|&v| rank[v]).min().unwrap();
        buckets[k].push(f);
    }
    Ok(())
}
struct Prepared {
    clauses: Vec<Clause>,
    remaining: Vec<usize>,
    scalar: Cell,
    plan: Vec<usize>,
    peak_bag: Vec<usize>,
}
impl Prepared {
    fn contradiction() -> Self {
        Self { clauses: vec![], remaining: vec![], scalar: Cell::No, plan: vec![], peak_bag: vec![] }
    }
}
fn prepare(p: &Problem, scope: &[usize], bits: u32, start: Instant, stats: &mut VeStats,
           condition: Option<i32>) -> Result<Prepared, &'static str> {
    if p.n > 4096 || scope.len() > 2048 || p.clauses.len() > 6000 || p.weights.len() < p.n {
        return Err("indexed_source");
    }
    let mut present = vec![false; p.n+1];
    for &v in scope {
        if v == 0 || v > p.n || present[v] { return Err("indexed_scope"); }
        present[v] = true;
    }
    let mut cs = Vec::new();
    for c in &p.clauses {
        if c.iter().any(|&l| l==0 || l.unsigned_abs() as usize > p.n || !present[l.unsigned_abs() as usize]) {
            return Err("indexed_clause_scope");
        }
        let mut c = c.clone();
        c.sort_unstable(); c.dedup();
        if c.iter().any(|&l| c.binary_search(&(-l)).is_ok()) { continue; }
        if c.len()>12 { return Err("indexed_clause_width"); }
        cs.push(c);
    }
    if let Some(l) = condition {
        let v = l.unsigned_abs() as usize;
        if v == 0 || v > p.n || !present[v] { return Err("indexed_condition_scope"); }
        cs.push(vec![l]);
    }
    cs.sort_unstable(); cs.dedup();
    let mut assigned = vec![0_i8; p.n+1];
    let mut scalar = Cell::One;
    let mut closure_work = 0_usize;
    loop {
        deadline(start)?;
        if cs.iter().any(Vec::is_empty) { return Ok(Prepared::contradiction()); }
        let units: Vec<_> = cs.iter().filter(|c| c.len()==1).map(|c| c[0]).collect();
        if units.is_empty() { break; }
        for l in units {
            let v=l.unsigned_abs() as usize; let sign=if l>0 {1} else {-1};
            if assigned[v] == -sign { return Ok(Prepared::contradiction()); }
            if assigned[v]==0 {
                assigned[v]=sign;
                let w=&p.weights[v-1];
                scalar=scalar.mul(&Cell::new(if sign>0 {w.positive.clone()} else {w.negative.clone()}),bits);
            }
        }
        let mut next=Vec::new();
        for mut c in cs {
            closure_work=closure_work.saturating_add(c.len());
            if closure_work>4_000_000 { return Err("indexed_unit_work"); }
            if c.iter().any(|&l| assigned[l.unsigned_abs() as usize] == (if l>0 {1} else {-1})) { continue; }
            c.retain(|l| assigned[l.unsigned_abs() as usize]==0);
            next.push(c);
        }
        cs=next;
    }
    cs.sort_unstable(); cs.dedup();
    let remaining: Vec<_>=scope.iter().copied().filter(|&v| assigned[v]==0).collect();
    let plan_start=Instant::now();
    stats.minfill_enabled=true;
    let mut peak_bag = Vec::new();
    let plan=order(&remaining,&cs,p.n,start,stats,&mut peak_bag)?;
    stats.minfill_plan_ns=plan_start.elapsed().as_nanos();
    stats.minfill_plan_width=stats.width;
    Ok(Prepared { clauses: cs, remaining, scalar, plan, peak_bag })
}
fn execute(p: &Problem, bits: u32, start: Instant, stats: &mut VeStats,
           prepared: Prepared) -> Result<Cell, &'static str> {
    let Prepared { clauses: cs, remaining, mut scalar, plan, .. } = prepared;
    if !scalar.sat() { return Ok(Cell::No); }
    let mut rank=vec![usize::MAX;p.n+1];
    for (i,&v) in plan.iter().enumerate() {rank[v]=i;}
    let mut buckets: Vec<Vec<Factor>>=(0..plan.len()).map(|_|Vec::new()).collect();
    let mut live=0;
    for &v in &remaining {
        let w=&p.weights[v-1];
        push(Factor{vars:vec![v],values:vec![Cell::new(w.negative.clone()),Cell::new(w.positive.clone())]},
            &mut buckets,&rank,&mut scalar,&mut live,bits)?;
    }
    for c in &cs {
        let mut vars: Vec<_>=c.iter().map(|l|l.unsigned_abs() as usize).collect();
        vars.sort_unstable(); vars.dedup();
        let size=1_usize<<vars.len();
        if live.saturating_add(size)>CELLS {return Err("indexed_live_cells");}
        let positions: Vec<_>=c.iter().map(|&l|(vars.binary_search(&(l.unsigned_abs() as usize)).unwrap(),l>0)).collect();
        let mut values=Vec::new();
        values.try_reserve_exact(size).map_err(|_|"indexed_allocation")?;
        for mask in 0..size {
            values.push(if positions.iter().any(|&(j,s)| ((mask>>j)&1==1)==s) {Cell::One} else {Cell::No});
        }
        push(Factor{vars,values},&mut buckets,&rank,&mut scalar,&mut live,bits)?;
    }
    stats.factors=stats.factors.saturating_add(remaining.len()+cs.len());
    stats.cells=stats.cells.saturating_add(live);
    for (k,&v) in plan.iter().enumerate() {
        deadline(start)?;
        let factors=std::mem::take(&mut buckets[k]);
        let mut rest: Vec<_>=factors.iter().flat_map(|f| f.vars.iter().copied()).filter(|&u|u!=v).collect();
        rest.sort_unstable(); rest.dedup();
        if factors.is_empty() || rest.len()>WIDTH {return Err("indexed_bucket");}
        let size=1_usize<<rest.len();
        if live.saturating_add(size)>CELLS {return Err("indexed_live_cells");}
        stats.products=stats.products.saturating_add(size.saturating_mul(factors.len()).saturating_mul(4));
        if stats.products>LOOKUPS {return Err("indexed_lookups");}
        stats.cells=stats.cells.saturating_add(size);
        let mut pivots=Vec::with_capacity(factors.len());
        let mut flips=Vec::with_capacity(factors.len());
        for f in &factors {
            let mut by_bit=vec![0_usize;rest.len()];
            let mut pivot=0;
            for (i,&x) in f.vars.iter().enumerate() {
                if x==v {pivot=1<<i;} else {by_bit[rest.binary_search(&x).unwrap()]=1<<i;}
            }
            let mut acc=0;
            for mask in &mut by_bit {acc^=*mask;*mask=acc;}
            pivots.push(pivot);flips.push(by_bit);
        }
        // Incrementally maintain local indices. SAT support is evaluated
        // before skipping any numerical multiplication (including zero).
        let mut indices=vec![0_usize;factors.len()];
        let mut values=Vec::new();
        values.try_reserve_exact(size).map_err(|_|"indexed_allocation")?;
        for assignment in 0..size {
            if assignment%256==0 {deadline(start)?;}
            let mut sum=Cell::No;
            for positive in [false,true] {
                let local=|i:usize| indices[i] | (if positive {pivots[i]} else {0});
                if factors.iter().enumerate().any(|(i,f)|!f.values[local(i)].sat()) {continue;}
                let mut product=Cell::One;
                for (i,f) in factors.iter().enumerate() {
                    product=product.mul(&f.values[local(i)],bits);
                    if matches!(&product,Cell::Zero) {break;}
                }
                sum=sum.add(&product,bits);
            }
            stats.zero_sat+=usize::from(matches!(&sum,Cell::Zero));
            values.push(sum);
            if assignment+1<size {
                let bit=(assignment+1).trailing_zeros() as usize;
                for (i,index) in indices.iter_mut().enumerate() {*index^=flips[i][bit];}
            }
        }
        live-=factors.iter().map(|f|f.values.len()).sum::<usize>();
        drop(factors);
        push(Factor{vars:rest,values},&mut buckets,&rank,&mut scalar,&mut live,bits)?;
        stats.eliminated+=1;
    }
    deadline(start)?;
    Ok(scalar)
}
pub(crate) fn count(p:&Problem,scope:&[usize],bits:u32)->(Option<Outcome>,VeStats) {
    let mut stats=VeStats{eligible:true,source_vars:p.n,scope_vars:scope.len(),source_clauses:p.clauses.len(),
        source_max_clause_width:p.clauses.iter().map(Vec::len).max().unwrap_or(0),..VeStats::default()};
    let start=Instant::now();
    let answer=match prepare(p,scope,bits,start,&mut stats,None)
        .and_then(|prepared| execute(p,bits,start,&mut stats,prepared)) {
        Ok(x)=>Some(Outcome{sat:x.sat(),count:x.value(bits)}),
        Err(reason)=>{stats.capped=true;stats.cap_reason=reason;None}
    };
    (answer,stats)
}

// Scope-only replay of the actual bucket schedule: counts both input and
// output live cells, and four potential lookups per factor/output cell.
// No numerical factors allocated; saturation is a conservative rejection.
#[derive(Clone, Copy, Debug, Default)]
struct Estimate { work: usize, peak: usize, width: usize }
impl Estimate {
    fn fits(self) -> bool { self.work <= LOOKUPS && self.peak <= CELLS }
}
fn estimate(p: &Prepared, n: usize, start: Instant) -> Result<Estimate, &'static str> {
    if !p.scalar.sat() { return Ok(Estimate::default()); }
    let mut rank = vec![usize::MAX; n+1];
    for (i,&v) in p.plan.iter().enumerate() { rank[v]=i; }
    let mut buckets: Vec<Vec<Vec<usize>>> = (0..p.plan.len()).map(|_| Vec::new()).collect();
    let mut live = 0_usize;
    for &v in &p.remaining { buckets[rank[v]].push(vec![v]); live += 2; }
    for c in &p.clauses {
        let mut vars: Vec<_> = c.iter().map(|l|l.unsigned_abs() as usize).collect();
        vars.sort_unstable(); vars.dedup();
        live = live.saturating_add(1_usize << vars.len());
        let k = vars.iter().map(|&v|rank[v]).min().ok_or("indexed_estimate_scope")?;
        buckets[k].push(vars);
    }
    let mut e = Estimate { peak: live, ..Estimate::default() };
    for (k,&v) in p.plan.iter().enumerate() {
        deadline(start)?;
        let factors = std::mem::take(&mut buckets[k]);
        let mut rest: Vec<_> = factors.iter().flatten().copied().filter(|&x|x!=v).collect();
        rest.sort_unstable(); rest.dedup();
        if factors.is_empty() || rest.len()>WIDTH { return Err("indexed_estimate_width"); }
        let size=1_usize<<rest.len();
        e.width=e.width.max(rest.len()+1);
        e.peak=e.peak.max(live.saturating_add(size));
        e.work=e.work.saturating_add(size.saturating_mul(factors.len()).saturating_mul(4));
        live=live.checked_sub(factors.iter().map(|f|1_usize<<f.len()).sum::<usize>())
            .ok_or("indexed_estimate_live")?;
        if !rest.is_empty() {
            let next=rest.iter().map(|&u|rank[u]).min().unwrap();
            buckets[next].push(rest);
            live=live.saturating_add(size);
        }
    }
    Ok(e)
}
fn qualifies(root: Estimate, neg: Estimate, pos: Estimate) -> bool {
    let work=neg.work.saturating_add(pos.work);
    neg.fits() && pos.fits() && work<=LOOKUPS && neg.width.max(pos.width)<root.width
        && work.saturating_mul(5)<=root.work.saturating_mul(4)
}
#[derive(Default, Debug)]
pub(crate) struct ConditionStats {
    pub(crate) selected: usize,
    pub(crate) probes: usize,
    pub(crate) branches_completed: usize,
    pub(crate) root_work: usize,
    pub(crate) root_peak: usize,
    pub(crate) chosen_work: usize,
    pub(crate) chosen_peak: usize,
    pub(crate) planning_ns: u128,
}
fn conditioned(p: &Problem, scope: &[usize], bits: u32, start: Instant,
    stats: &mut VeStats, trace: &mut ConditionStats) -> Result<Cell, &'static str> {
    let root=prepare(p,scope,bits,start,stats,None)?;
    let base=estimate(&root,p.n,start)?;
    trace.root_work=base.work;
    trace.root_peak=base.peak;
    // Do not add expensive planning to the already-small QNN-like path.
    // Probe only a wide, costly bucket; if both branches do not qualify,
    // keep the unsplit root (or fall back before allocating factor tables).
    let mut best: Option<((usize,usize,usize), Prepared, Prepared)> = None;
    if base.width>=17 && (base.peak>=100_000 || base.work>=4_000_000) {
        let mut degree=vec![BTreeSet::<usize>::new();p.n+1];
        for c in &root.clauses {
            let vars: BTreeSet<_>=c.iter().map(|l|l.unsigned_abs() as usize).collect();
            for &v in &vars { degree[v].extend(vars.iter().copied().filter(|&u|u!=v)); }
        }
        let mut candidates=root.peak_bag.clone();
        candidates.sort_unstable_by_key(|&v|(std::cmp::Reverse(degree[v].len()),v));
        for v in candidates.into_iter().take(6) {
            deadline(start)?;
            trace.probes+=1;
            let mut ns=VeStats::default();
            let mut ps=VeStats::default();
            let Ok(neg)=prepare(p,scope,bits,start,&mut ns,Some(-(v as i32))) else { continue };
            let Ok(pos)=prepare(p,scope,bits,start,&mut ps,Some(v as i32)) else { continue };
            let a=estimate(&neg,p.n,start)?;
            let b=estimate(&pos,p.n,start)?;
            if !qualifies(base,a,b) { continue; }
            let key=(a.work.saturating_add(b.work),a.peak.max(b.peak),v);
            if best.as_ref().is_none_or(|(previous,_,_)|key<*previous) {
                best=Some((key,neg,pos));
            }
        }
    }
    trace.planning_ns=start.elapsed().as_nanos();
    if let Some(((work,peak,v),neg,pos))=best {
        trace.selected=v; trace.chosen_work=work; trace.chosen_peak=peak;
        // The two prepared cofactors each include the conditioned literal's
        // weight exactly once in their unit-closure scalar. Both must finish.
        // Shared start and counters enforce the total deadline/work cap.
        drop(root);
        let negative=execute(p,bits,start,stats,neg)?;
        trace.branches_completed=1;
        let positive=execute(p,bits,start,stats,pos)?;
        trace.branches_completed=2;
        deadline(start)?;
        Ok(negative.add(&positive,bits))
    } else if base.fits() {
        execute(p,bits,start,stats,root)
    } else {
        Err("indexed_condition_no_admissible_plan")
    }
}
pub(crate) fn count_conditioned(p:&Problem,scope:&[usize],bits:u32)
    ->(Option<Outcome>,VeStats,ConditionStats) {
    let mut stats=VeStats{eligible:true,source_vars:p.n,scope_vars:scope.len(),
        source_clauses:p.clauses.len(),
        source_max_clause_width:p.clauses.iter().map(Vec::len).max().unwrap_or(0),..VeStats::default()};
    let mut trace=ConditionStats::default();
    let answer=match conditioned(p,scope,bits,Instant::now(),&mut stats,&mut trace) {
        Ok(x)=>Some(Outcome{sat:x.sat(),count:x.value(bits)}),
        Err(reason)=>{stats.capped=true;stats.cap_reason=reason;None}
    };
    (answer,stats,trace)
}


// Opt-in compact representation. This envelope is separate from the old
// indexed arm: 400k u32 table slots, 400k MPFR pool entries, 50m lookup
// upper bound, 2m nontrivial arithmetic calls, bounded per-bucket pair cache.
// These are cardinality limits, NOT a byte/RSS certificate. The native
// harness still enforces the identical 3 GiB / 30 s full-process budget.
mod value_reuse {
    use super::*;
    use std::collections::{HashMap, hash_map::DefaultHasher};
    use std::hash::{Hash, Hasher};

    const TABLE_CAP: usize = 400_000;
    const POOL_CAP: usize = 400_000;
    const WORK_CAP: usize = 50_000_000;
    const ARITH_CAP: usize = 2_000_000;
    const PAIR_CAP: usize = 160_000;
    const GC_SCAN_CAP: usize = 16_000_000;
    const GC_THRESHOLD: usize = 50_000;
    const NO: u32 = 0;
    const ONE: u32 = 1;
    const ZERO: u32 = 2;

    #[derive(Default, Debug)]
    pub(crate) struct ReuseStats {
        pub(crate) mul: usize,
        pub(crate) add: usize,
        pub(crate) nontrivial: usize,
        pub(crate) hits: usize,
        pub(crate) peak_pool: usize,
        pub(crate) gc: usize,
        pub(crate) gc_scans: usize,
        pub(crate) peak_live: usize,
        pub(crate) removed: usize,
    }
    struct Pool {
        values: Vec<C>,
        links: Vec<u32>,
        index: HashMap<u64, u32>,
        mul: HashMap<(u32,u32),u32>,
        add: HashMap<(u32,u32),u32>,
        stats: ReuseStats,
        bits: u32,
    }
    // OrdFloat supports exact hashing. Canonicalize the hash of signed zero
    // because the solver's numerical equality treats +0 and -0 as equal.
    // A hash hit alone NEVER merges two values: compare both MPFR components.
    fn fingerprint(c: &C) -> u64 {
        let mut h=DefaultHasher::new();
        for v in [&c.re,&c.im] {
            if v.is_zero() { 0_u8.hash(&mut h); }
            else { 1_u8.hash(&mut h); v.as_ord().hash(&mut h); }
        }
        h.finish()
    }
    impl Pool {
        fn new(bits: u32) -> Self {
            Self { values:vec![C::zero(bits),C::one(bits),C::zero(bits)],
                links:vec![0;3],index:HashMap::new(),mul:HashMap::new(),add:HashMap::new(),
                stats:ReuseStats{peak_pool:3,..ReuseStats::default()},bits }
        }
        fn intern(&mut self,c:C)->Result<u32,&'static str> {
            if !c.re.is_finite() || !c.im.is_finite() {return Err("intern_nonfinite");}
            if c.re==0 && c.im==0 {return Ok(ZERO);}
            if c.re==1 && c.im==0 {return Ok(ONE);}
            let hash=fingerprint(&c);
            let head=self.index.get(&hash).copied().unwrap_or(NO);
            let mut at=head;
            let mut probes=0;
            while at!=NO {
                let old=&self.values[at as usize];
                if old.re==c.re && old.im==c.im {return Ok(at);}
                probes+=1;
                if probes>=64 {return Err("intern_hash_chain");}
                at=self.links[at as usize];
            }
            if self.values.len()>=POOL_CAP {return Err("intern_pool");}
            let id=self.values.len() as u32;
            self.values.push(c);self.links.push(head);self.index.insert(hash,id);
            self.stats.peak_pool=self.stats.peak_pool.max(self.values.len());
            Ok(id)
        }
        fn clear_pairs(&mut self) { self.mul.clear();self.add.clear(); }
        fn op(&mut self,a:u32,b:u32,multiply:bool)->Result<u32,&'static str> {
            if multiply {
                if a==NO||b==NO {return Ok(NO);}
                if a==ZERO||b==ZERO {return Ok(ZERO);}
                if a==ONE {return Ok(b);}
                if b==ONE {return Ok(a);}
            } else {
                if a==NO||a==ZERO {return Ok(b);}
                if b==NO||b==ZERO {return Ok(a);}
            }
            self.stats.nontrivial+=1;
            let key=(a,b); // ordered pair: preserve this finite-precision operation
            let cached=if multiply {self.mul.get(&key)} else {self.add.get(&key)};
            if let Some(&id)=cached {self.stats.hits+=1;return Ok(id);}
            if self.stats.mul+self.stats.add>=ARITH_CAP {return Err("intern_arithmetic");}
            let x=&self.values[a as usize];let y=&self.values[b as usize];
            let c=if multiply {self.stats.mul+=1;x.mul(y,self.bits)}
                else {self.stats.add+=1;x.add(y,self.bits)};
            let id=self.intern(c)?;
            let memo=if multiply {&mut self.mul} else {&mut self.add};
            if memo.len()>=PAIR_CAP {memo.clear();}
            memo.insert(key,id);
            Ok(id)
        }
        // Only retained factors and the scalar own IDs across bucket
        // boundaries. Pair caches MUST be cleared before remapping.
        fn reclaim(&mut self,buckets:&mut [Vec<IdFactor>],scalar:&mut u32,start:Instant)
            ->Result<(),&'static str> {
            self.clear_pairs();
            let mut mark=vec![false;self.values.len()];
            mark[..3].fill(true);mark[*scalar as usize]=true;
            let scan=self.values.len().saturating_add(buckets.iter().flatten().map(|f|f.ids.len()).sum::<usize>());
            self.stats.gc_scans=self.stats.gc_scans.saturating_add(scan);
            if self.stats.gc_scans>GC_SCAN_CAP {return Err("intern_gc_work");}
            for f in buckets.iter().flatten() {for &id in &f.ids {mark[id as usize]=true;}}
            let old=std::mem::take(&mut self.values);
            self.links.clear();self.index.clear();
            let mut map=vec![NO;old.len()];
            for (i,c) in old.into_iter().enumerate() {
                if i&1023==0 {deadline(start)?;}
                if i<3 {self.values.push(c);self.links.push(NO);map[i]=i as u32;}
                else if mark[i] {map[i]=self.intern(c)?;}
            }
            *scalar=map[*scalar as usize];
            for f in buckets.iter_mut().flatten() {for id in &mut f.ids {*id=map[*id as usize];}}
            self.stats.gc+=1;
            Ok(())
        }
    }
    struct IdFactor {vars:Vec<usize>,ids:Vec<u32>}
    // Exact ID equality (not a numerical tolerance) proves independence
    // of a dimension. A removed dimension is still summed by its later
    // bucket, including a factor of two if no factor retains that variable.
    fn compress(f:&mut IdFactor)->usize {
        let mut removed=0;let mut j=0;
        while j<f.vars.len() {
            let stride=1_usize<<j;
            if f.ids.chunks_exact(2*stride).all(|x|x[..stride]==x[stride..]) {
                let count=f.ids.len()/(2*stride);
                for k in 0..count {let source=2*k*stride;f.ids.copy_within(source..source+stride,k*stride);}
                f.ids.truncate(count*stride);f.vars.remove(j);removed+=1;
            } else {j+=1;}
        }
        removed
    }
    fn put(mut f:IdFactor,buckets:&mut [Vec<IdFactor>],rank:&[usize],
        scalar:&mut u32,live:&mut usize,pool:&mut Pool)->Result<(),&'static str> {
        pool.stats.removed+=compress(&mut f);
        if f.vars.is_empty() {*scalar=pool.op(*scalar,f.ids[0],true)?;}
        else {
            *live=live.saturating_add(f.ids.len());
            if *live>TABLE_CAP {return Err("intern_live_cells");}
            let k=f.vars.iter().map(|&v|rank[v]).min().unwrap();
            buckets[k].push(f);
        }
        pool.stats.peak_live=pool.stats.peak_live.max(*live);
        Ok(())
    }
    fn execute_ids(p:&Problem,prepared:Prepared,stats:&mut VeStats,pool:&mut Pool,
                   start:Instant,threshold:usize)->Result<u32,&'static str> {
        let Prepared{clauses,remaining,scalar,plan,..}=prepared;
        if !scalar.sat() {return Ok(NO);}
        let bits=pool.bits;
        let mut scalar=pool.intern(scalar.value(bits))?;
        let mut rank=vec![usize::MAX;p.n+1];
        for (i,&v) in plan.iter().enumerate() {rank[v]=i;}
        let mut buckets:Vec<Vec<IdFactor>>=(0..plan.len()).map(|_|Vec::new()).collect();
        let mut live=0_usize;
        for &v in &remaining {
            let w=&p.weights[v-1];
            let ids=vec![pool.intern(w.negative.clone())?,pool.intern(w.positive.clone())?];
            put(IdFactor{vars:vec![v],ids},&mut buckets,&rank,&mut scalar,&mut live,pool)?;
        }
        for c in &clauses {
            let mut vars:Vec<_>=c.iter().map(|l|l.unsigned_abs() as usize).collect();
            vars.sort_unstable();vars.dedup();
            let size=1_usize<<vars.len();
            if live.saturating_add(size)>TABLE_CAP {return Err("intern_live_cells");}
            pool.stats.peak_live=pool.stats.peak_live.max(live+size);
            let positions:Vec<_>=c.iter().map(|&l|(vars.binary_search(&(l.unsigned_abs() as usize)).unwrap(),l>0)).collect();
            let mut ids=Vec::new();
            ids.try_reserve_exact(size).map_err(|_|"intern_allocation")?;
            for mask in 0..size {
                ids.push(if positions.iter().any(|&(j,s)|((mask>>j)&1==1)==s) {ONE} else {NO});
            }
            put(IdFactor{vars,ids},&mut buckets,&rank,&mut scalar,&mut live,pool)?;
        }
        stats.factors=remaining.len()+clauses.len();stats.cells=live;
        let mut next_gc=threshold;
        for (k,&v) in plan.iter().enumerate() {
            deadline(start)?;
            pool.clear_pairs();
            let factors=std::mem::take(&mut buckets[k]);
            if factors.is_empty() {
                // An unconstrained eliminated variable still has two values.
                let two=pool.intern(C::decimal(bits,"2","0").map_err(|_|"intern_two")?)?;
                scalar=pool.op(scalar,two,true)?;
            } else {
                let mut rest:Vec<_>=factors.iter().flat_map(|f|f.vars.iter().copied()).filter(|&u|u!=v).collect();
                rest.sort_unstable();rest.dedup();
                if rest.len()>WIDTH {return Err("intern_width");}
                let size=1_usize<<rest.len();
                let peak=live.saturating_add(size);
                if peak>TABLE_CAP {return Err("intern_live_cells");}
                pool.stats.peak_live=pool.stats.peak_live.max(peak);
                stats.products=stats.products.saturating_add(size.saturating_mul(factors.len()).saturating_mul(4));
                if stats.products>WORK_CAP {return Err("intern_lookups");}
                stats.cells=stats.cells.saturating_add(size);
                let mut pivots=Vec::with_capacity(factors.len());
                let mut flips=Vec::with_capacity(factors.len());
                for f in &factors {
                    let mut by_bit=vec![0_usize;rest.len()];let mut pivot=0;
                    for (i,&x) in f.vars.iter().enumerate() {
                        if x==v {pivot=1<<i;} else {by_bit[rest.binary_search(&x).unwrap()]=1<<i;}
                    }
                    let mut acc=0;for mask in &mut by_bit {acc^=*mask;*mask=acc;}
                    pivots.push(pivot);flips.push(by_bit);
                }
                let mut indices=vec![0_usize;factors.len()];
                let mut ids=Vec::new();ids.try_reserve_exact(size).map_err(|_|"intern_allocation")?;
                for assignment in 0..size {
                    if assignment&255==0 {deadline(start)?;}
                    let mut sum=NO;
                    for positive in [false,true] {
                        let local=|i:usize|indices[i]|if positive {pivots[i]} else {0};
                        // Boolean support is checked independently of any zero value.
                        if factors.iter().enumerate().any(|(i,f)|f.ids[local(i)]==NO) {continue;}
                        let mut product=ONE;
                        for (i,f) in factors.iter().enumerate() {
                            product=pool.op(product,f.ids[local(i)],true)?;
                            if product==ZERO {break;}
                        }
                        sum=pool.op(sum,product,false)?;
                    }
                    stats.zero_sat+=usize::from(sum==ZERO);ids.push(sum);
                    if assignment+1<size {
                        let bit=(assignment+1).trailing_zeros() as usize;
                        for (i,index) in indices.iter_mut().enumerate() {*index^=flips[i][bit];}
                    }
                }
                live-=factors.iter().map(|f|f.ids.len()).sum::<usize>();drop(factors);
                put(IdFactor{vars:rest,ids},&mut buckets,&rank,&mut scalar,&mut live,pool)?;
            }
            stats.eliminated+=1;
            if pool.values.len()>=next_gc {
                pool.reclaim(&mut buckets[k+1..],&mut scalar,start)?;
                next_gc=if threshold<GC_THRESHOLD {threshold}
                    else {pool.values.len().saturating_add(25_000).clamp(GC_THRESHOLD,350_000)};
            }
        }
        deadline(start)?;
        Ok(scalar)
    }
    pub(super) fn count_with_threshold(p:&Problem,scope:&[usize],bits:u32,threshold:usize)
        ->(Option<Outcome>,VeStats,ReuseStats) {
        let start=Instant::now();
        let mut stats=VeStats{eligible:true,source_vars:p.n,scope_vars:scope.len(),
            source_clauses:p.clauses.len(),source_max_clause_width:p.clauses.iter().map(Vec::len).max().unwrap_or(0),
            ..VeStats::default()};
        let mut pool=Pool::new(bits);
        let result=prepare(p,scope,bits,start,&mut stats,None)
            .and_then(|prepared|execute_ids(p,prepared,&mut stats,&mut pool,start,threshold));
        let answer=match result {
            Ok(id)=>Some(Outcome{sat:id!=NO,count:pool.values[id as usize].clone()}),
            Err(reason)=>{stats.capped=true;stats.cap_reason=reason;None}
        };
        (answer,stats,pool.stats)
    }
    pub(crate) fn count_interned(p:&Problem,scope:&[usize],bits:u32)
        ->(Option<Outcome>,VeStats,ReuseStats) {
        count_with_threshold(p,scope,bits,GC_THRESHOLD)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn exact_ids_and_support_survive_gc() {
            for bits in [100,333] {
                let mut pool=Pool::new(bits);
                let a=pool.intern(C::decimal(bits,"1.0000000000000000000000001","0").unwrap()).unwrap();
                let b=pool.intern(C::decimal(bits,"1.0000000000000000000000002","-0").unwrap()).unwrap();
                assert_ne!(a,b);assert_ne!(a,ONE);
                assert_eq!(pool.intern(C::decimal(bits,"1.0000000000000000000000001","-0").unwrap()).unwrap(),a);
                assert_eq!(pool.op(NO,ZERO,true).unwrap(),NO);
                assert_eq!(pool.op(ZERO,ONE,true).unwrap(),ZERO);
                let v=pool.op(a,b,true).unwrap();
                assert_eq!(pool.op(a,b,true).unwrap(),v);
                assert_eq!(pool.stats.hits,1);
                let expected=pool.values[v as usize].clone();
                let mut scalar=v;
                let mut buckets=vec![vec![IdFactor{vars:vec![1],ids:vec![a,b]}]];
                pool.reclaim(&mut buckets,&mut scalar,Instant::now()).unwrap();
                assert_eq!(pool.values[scalar as usize].re,expected.re);
                assert_eq!(pool.values[scalar as usize].im,expected.im);
                assert!(pool.mul.is_empty()&&pool.add.is_empty());
                assert_ne!(buckets[0][0].ids[0],buckets[0][0].ids[1]);
            }
        }
    }
}
pub(crate) use value_reuse::count_interned;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, weighted_equivalence};
    #[test]
    fn indexed_fused_oracle() {
        let fixtures=[
            "p cnf 4 4\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight -1 0.75 -0.5 0\n1 -2 0\n2 3 0\n-3 4 0\n-4 1 0\n",
            "p cnf 3 3\nc t wmc\nc p weight 1 1 0 0\nc p weight -1 -1 0 0\n-1 2 0\n1 -2 0\n3 0\n",
            "p cnf 2 2\nc t wmc\n1 0\n-1 0\n",
            "p cnf 3 1\nc t wmc\nc p weight 1 0 0 0\n1 0\n",
            "p cnf 5 3\nc t wmc\nc p weight -5 0.5 -0.25 0\n1 2 0\n-1 2 0\n3 4 0\n",
            "p cnf 3 1\nc t wmc\n0\n",
        ];
        for bits in [100,333] {
            for text in fixtures {
                let p=parse_emitted(text,bits).unwrap();
                let mut expected=Outcome::unsat(bits);
                for mask in 0..(1_usize<<p.n) {
                    if !p.clauses.iter().all(|c|c.iter().any(|&l| ((mask>>(l.unsigned_abs() as usize-1))&1==1)==(l>0))) {continue;}
                    expected.sat=true; let mut value=C::one(bits);
                    for (i,w) in p.weights.iter().enumerate().take(p.n) {
                        value=value.mul(if (mask>>i)&1==1 {&w.positive} else {&w.negative},bits);
                    }
                    expected.count=expected.count.add(&value,bits);
                }
                let (reduced,scope,_)=weighted_equivalence::reduce(&p,bits);
                for (problem,vars) in [(&p,(1..=p.n).collect::<Vec<_>>()),(&reduced,scope)] {
                    let (got,stats)=count(problem,&vars,bits);
                    let got=got.unwrap_or_else(||panic!("unexpected cap: {stats:?}"));
                    assert_eq!(got.sat,expected.sat);
                    assert_eq!(got.count.re,expected.count.re);
                    assert_eq!(got.count.im,expected.count.im);
                    let (routed,_,_)=count_conditioned(problem,&vars,bits);
                    let routed=routed.expect("small root must fit");
                    assert_eq!(routed.sat,expected.sat);
                    assert_eq!(routed.count.re,expected.count.re);
                    assert_eq!(routed.count.im,expected.count.im);
                    for threshold in [3,50_000] {
                        let (interned,st,_)=value_reuse::count_with_threshold(problem,&vars,bits,threshold);
                        let interned=interned.unwrap_or_else(||panic!("interned cap: {st:?}"));
                        assert_eq!(interned.sat,expected.sat);
                        assert_eq!(interned.count.re,expected.count.re);
                        assert_eq!(interned.count.im,expected.count.im);
                    }
                    if let Some(&v)=vars.first() {
                        let start=Instant::now();
                        let mut ns=VeStats::default();
                        let mut ps=VeStats::default();
                        let neg=prepare(problem,&vars,bits,start,&mut ns,Some(-(v as i32))).unwrap();
                        let pos=prepare(problem,&vars,bits,start,&mut ps,Some(v as i32)).unwrap();
                        let a=estimate(&neg,problem.n,start).unwrap();
                        let b=estimate(&pos,problem.n,start).unwrap();
                        let mut shared=VeStats::default();
                        let x=execute(problem,bits,start,&mut shared,neg).unwrap();
                        let y=execute(problem,bits,start,&mut shared,pos).unwrap();
                        let total=x.add(&y,bits);
                        assert!(shared.products<=a.work+b.work);
                        assert_eq!(total.sat(),expected.sat);
                        assert_eq!(total.value(bits).re,expected.count.re);
                        assert_eq!(total.value(bits).im,expected.count.im);
                    }
                }
            }
        }
    }
    #[test]
    fn conditioning_gate_counts_both_branches_and_live_peak() {
        let root=Estimate{work:10_000_000,peak:390_000,width:18};
        let a=Estimate{work:3_000_000,peak:100_000,width:16};
        let b=Estimate{work:4_900_000,peak:130_000,width:17};
        assert!(qualifies(root,a,b));
        assert!(!qualifies(root,a,Estimate{work:5_100_000,..b}));
        assert!(!qualifies(root,a,Estimate{peak:300_001,..b}));
        assert!(!qualifies(root,a,Estimate{width:18,..b}));
    }
    #[test]
    fn indexed_width_rejects_without_verdict() {
        let mut text=String::from("p cnf 19 171\nc t wmc\n");
        for i in 1..=19 {for j in i+1..=19 {text.push_str(&format!("{i} {j} 0\n"));}}
        let p=parse_emitted(&text,100).unwrap();
        let (answer,stats)=count(&p,&(1..=19).collect::<Vec<_>>(),100);
        assert!(answer.is_none());assert_eq!(stats.cap_reason,"indexed_width");
        let (answer,stats,_)=count_interned(&p,&(1..=19).collect::<Vec<_>>(),100);
        assert!(answer.is_none());assert_eq!(stats.cap_reason,"indexed_width");
    }
}
