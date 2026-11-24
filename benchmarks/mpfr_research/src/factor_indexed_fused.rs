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
fn order(scope: &[usize], clauses: &[Clause], n: usize, start: Instant, stats: &mut VeStats)
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
        stats.width = stats.width.max(near.len()+1);
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
fn run(p: &Problem, scope: &[usize], bits: u32, start: Instant, stats: &mut VeStats)
    -> Result<Cell, &'static str> {
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
    cs.sort_unstable(); cs.dedup();
    let mut assigned = vec![0_i8; p.n+1];
    let mut scalar = Cell::One;
    let mut closure_work = 0_usize;
    loop {
        deadline(start)?;
        if cs.iter().any(Vec::is_empty) { return Ok(Cell::No); }
        let units: Vec<_> = cs.iter().filter(|c| c.len()==1).map(|c| c[0]).collect();
        if units.is_empty() { break; }
        for l in units {
            let v=l.unsigned_abs() as usize; let sign=if l>0 {1} else {-1};
            if assigned[v] == -sign { return Ok(Cell::No); }
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
    let plan=order(&remaining,&cs,p.n,start,stats)?;
    stats.minfill_plan_ns=plan_start.elapsed().as_nanos();
    stats.minfill_plan_width=stats.width;
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
    stats.factors=remaining.len()+cs.len(); stats.cells=live;
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
    let answer=match run(p,scope,bits,Instant::now(),&mut stats) {
        Ok(x)=>Some(Outcome{sat:x.sat(),count:x.value(bits)}),
        Err(reason)=>{stats.capped=true;stats.cap_reason=reason;None}
    };
    (answer,stats)
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
                }
            }
        }
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
