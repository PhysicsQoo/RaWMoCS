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
    }
}
