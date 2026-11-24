//! Bounded source-CNF cardinality research route. No production integration.
use super::{C, Outcome, Problem};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

type Set = BTreeSet<i32>;
type Pair = (usize, usize);
type Poly = BTreeMap<Pair, Vec<C>>;
type Graph = BTreeMap<usize, BTreeSet<usize>>;
type PhaseKey = (String, String);
type PhaseEdges = BTreeMap<PhaseKey, Vec<(usize, usize, usize)>>;
type Gate = (i32, [i32; 2]);
type Extraction = (Vec<Gate>, Vec<i32>, BTreeMap<usize, bool>, BTreeSet<Set>);

mod definitional;

const MAX_CELLS: usize = 500_000;
const MAX_WORK: usize = 40_000_000;

#[derive(Default)]
struct Trace {
    gates: usize,
    free: usize,
    cliques: Vec<usize>,
    width: usize,
    peak: usize,
    work: usize,
}

fn var(l: i32) -> usize { l.unsigned_abs() as usize }
fn idx(l: i32) -> usize { 2 * (var(l)-1) + usize::from(l < 0) }
fn weight(p: &Problem, l: i32) -> &C {
    let w = &p.weights[var(l)-1];
    if l > 0 { &w.positive } else { &w.negative }
}
fn clean(c: &Set) -> bool { !c.iter().any(|l| c.contains(&-l)) }
fn reach(g: &[Vec<usize>], root: usize) -> Vec<bool> {
    let mut seen = vec![false;g.len()];
    let mut stack = vec![root];
    seen[root] = true;
    while let Some(v) = stack.pop() {
        for &u in &g[v] {
            if !seen[u] { seen[u]=true; stack.push(u); }
        }
    }
    seen
}
fn extract(p: &Problem, source_cap: usize) -> Result<Extraction, String> {
    if p.n > source_cap || p.clauses.len()>10_000 { return Err("cardinality source cap".into()); }
    let clauses: BTreeSet<Set> = p.clauses.iter().map(|c| c.iter().copied().collect())
        .filter(clean).collect();
    let mut occ = vec![BTreeSet::new(); p.n+1];
    for c in &clauses {
        for &l in c { occ[var(l)].insert(c.clone()); }
    }
    let mut gates = Vec::new();
    let mut removed = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    for c in &clauses {
        if c.len()!=3 {continue;}
        for &o in c {
            let a:Vec<_>=c.iter().filter(|&&l|l!=o).map(|&l|-l).collect();
            let defs=BTreeSet::from([c.clone(),Set::from([-o,a[0]]),Set::from([-o,a[1]])]);
            if occ[var(o)] == defs {
                if !outputs.insert(var(o)) {return Err("ambiguous private definition".into());}
                gates.push((o,[a[0],a[1]]));
                removed.extend(defs);
            }
        }
    }
    if gates.iter().any(|(_,a)|a.iter().any(|&l|outputs.contains(&var(l)))) {
        return Err("dependent private definitions".into());
    }
    let rest:Vec<_>=clauses.difference(&removed).cloned().collect();
    let mut g=vec![Vec::new();2*p.n];
    let mut rev=g.clone();
    for c in &rest {
        if c.len()!=2 {continue;}
        let a:Vec<_>=c.iter().copied().collect();
        for (u,v) in [(idx(-a[0]),idx(a[1])),(idx(-a[1]),idx(a[0]))] {
            g[u].push(v);rev[v].push(u);
        }
    }
    let mut rep=vec![0;2*p.n];
    for v in 1..=p.n {
        if outputs.contains(&v) || rep[2*(v-1)]!=0 {continue;}
        let root=2*(v-1);
        let f=reach(&g,root);let b=reach(&rev,root);
        if f[root+1]&&b[root+1] {return Err("contradictory SCC".into());}
        for i in 0..rep.len() {
            if f[i]&&b[i] {
                if rep[i]!=0||rep[i^1]!=0 {return Err("SCC ownership".into());}
                rep[i]=v as i32;rep[i^1] = -(v as i32);
            }
        }
    }
    let mut residual:BTreeSet<Set>=rest.iter().map(|c|c.iter().map(|&l|rep[idx(l)]).collect())
        .filter(clean).collect();
    let mut units=BTreeMap::new();
    loop {
        if residual.contains(&Set::new()) {return Err("contradictory residual".into());}
        let Some(l)=residual.iter().find(|c|c.len()==1).and_then(|c|c.first()).copied() else {break;};
        if units.insert(var(l),l>0).is_some() {return Err("unit ownership".into());}
        residual=residual.into_iter().filter(|c|!c.contains(&l)).map(|mut c|{c.remove(&-l);c}).collect();
    }
    Ok((gates,rep,units,residual))
}

fn phase_keys(raw: &str, n: usize) -> Vec<PhaseKey> {
    // Exact text identity is intentionally stricter than numerical equality.
    // Only the explicit decimal spellings below qualify as exact unity.
    let mut keys=vec![("1.0".into(),"0.0".into());2*n];
    for line in raw.lines() {
        let f:Vec<_>=line.split_whitespace().collect();
        if f.len()==7 && f[..3]==["c","p","weight"] {
            // The caller has already validated the entire source.
            if let Ok(l)=f[3].parse::<i32>() {
                if l!=0 && var(l)<=n {keys[idx(l)]=(f[4].into(),f[5].into());}
            }
        }
    }
    keys
}
fn unity(k: &PhaseKey) -> bool {
    matches!(k.0.as_str(),"1"|"1.0") && matches!(k.1.as_str(),"0"|"0.0")
}

struct Factor { scope: Vec<usize>, poly: Poly }
impl Factor {
    fn ordinary(scope:Vec<usize>,values:Vec<C>)->Self {
        Self{scope,poly:BTreeMap::from([((0,0),values)])}
    }
    fn cells(&self)->usize {self.poly.len()*(1usize<<self.scope.len())}
}
fn project(mask:usize, union:&[usize], scope:&[usize])->usize {
    scope.iter().enumerate().fold(0,|acc,(i,v)| {
        acc|(((mask>>union.binary_search(v).expect("scope subset"))&1)<<i)
    })
}
fn nonzero(x:&C)->bool {x.re!=0||x.im!=0}
fn multiply(a:Factor,b:Factor,bits:u32,trace:&mut Trace,start:Instant)->Result<Factor,String>{
    let scope:Vec<_>=a.scope.iter().chain(&b.scope).copied().collect::<BTreeSet<_>>().into_iter().collect();
    if scope.len()>16 {return Err("cardinality bag cap".into());}
    let size=1usize<<scope.len();
    let work=a.poly.len().saturating_mul(b.poly.len()).saturating_mul(size);
    trace.work=trace.work.saturating_add(work);
    if trace.work>MAX_WORK || start.elapsed().as_secs()>=105 {return Err("cardinality work/deadline cap".into());}
    let pa:Vec<_>=(0..size).map(|m|project(m,&scope,&a.scope)).collect();
    let pb:Vec<_>=(0..size).map(|m|project(m,&scope,&b.scope)).collect();
    let mut poly:Poly=BTreeMap::new();
    for (&(i,j),x) in &a.poly {
        for (&(k,l),y) in &b.poly {
            let key=(i+k,j+l);
            if key.0>12 || key.1>12 {return Err("cardinality degree cap".into());}
            if !poly.contains_key(&key) && (poly.len()+1)*size>MAX_CELLS {
                return Err("cardinality cells cap".into());
            }
            let values=poly.entry(key).or_insert_with(||vec![C::zero(bits);size]);
            for m in 0..size {
                if nonzero(&x[pa[m]]) && nonzero(&y[pb[m]]) {
                    let z=x[pa[m]].mul(&y[pb[m]],bits);
                    values[m]=values[m].add(&z,bits);
                }
            }
        }
    }
    trace.peak=trace.peak.max(poly.len()*size);
    Ok(Factor{scope,poly})
}
fn sum_out(mut f:Factor,v:usize,bits:u32)->Factor {
    let axis=f.scope.binary_search(&v).expect("bucket contains pivot");
    f.scope.remove(axis);
    let size=1usize<<f.scope.len();
    let low=(1usize<<axis)-1;
    for values in f.poly.values_mut() {
        let next=(0..size).map(|m|{
            let base=(m&low)|((m&!low)<<1);
            values[base].add(&values[base|(1<<axis)],bits)
        }).collect();
        *values=next;
    }
    f
}
fn edge(g:&mut Graph,a:usize,b:usize) {
    g.entry(a).or_default().insert(b);g.entry(b).or_default().insert(a);
}
fn minfill(g:&Graph,v:usize)->(usize,usize,usize) {
    let n=&g[&v];
    let missing=n.iter().enumerate().map(|(i,a)|n.iter().skip(i+1).filter(|b|!g[a].contains(*b)).count()).sum();
    (missing,n.len(),v)
}

fn count(p:&Problem,raw:&str,bits:u32,trace:&mut Trace)->Result<Outcome,String> {
    let start=Instant::now();
    let (gates,rep,units,residual)=extract(p,1024)?;
    if !residual.is_empty() {return Err("unclosed hard clauses".into());}
    let free:BTreeSet<_>=rep.iter().copied().filter(|&l|l!=0&&!units.contains_key(&var(l))).map(var).collect();
    trace.free=free.len();trace.gates=gates.len();
    let keys=phase_keys(raw,p.n);
    let mut groups:PhaseEdges=BTreeMap::new();
    let mut mapped=Vec::new();
    for (i,(o,a)) in gates.iter().enumerate() {
        let b=[rep[idx(a[0])],rep[idx(a[1])]];
        if b.iter().any(|&l|!free.contains(&var(l)))||var(b[0])==var(b[1]) {
            return Err("unsupported reduced gate".into());
        }
        mapped.push((*o,b));
        if b.iter().all(|&l|l>0)&&unity(&keys[idx(-o)]) {
            groups.entry(keys[idx(*o)].clone()).or_default().push((var(b[0]),var(b[1]),i));
        }
    }
    // A connected component of an exact phase class is admitted only
    // with every pair present exactly once. Other factors remain intact.
    let mut candidates=Vec::new();
    for es in groups.values() {
        let mut g=Graph::new();
        for &(a,b,_) in es {edge(&mut g,a,b);}
        let mut unseen:BTreeSet<_>=g.keys().copied().collect();
        while let Some(root)=unseen.pop_first() {
            let mut component=BTreeSet::from([root]);let mut stack=vec![root];
            while let Some(v)=stack.pop(){
                for &u in &g[&v] {if unseen.remove(&u){component.insert(u);stack.push(u);}}
            }
            let n=component.len();
            if !(3..=12).contains(&n) {continue;}
            let selected:Vec<_>=es.iter().filter(|(a,b,_)|component.contains(a)&&component.contains(b)).copied().collect();
            let pairs:BTreeSet<_>=selected.iter().map(|&(a,b,_)|(a.min(b),a.max(b))).collect();
            if selected.len()==n*(n-1)/2 && pairs.len()==selected.len() {
                candidates.push((component,selected));
            }
        }
    }
    candidates.sort_by_key(|(c,_)|(std::cmp::Reverse(c.len()),c.first().copied()));
    if candidates.len()<2 {return Err("missing uniform cliques".into());}
    let mut used=BTreeSet::new();let mut marked=BTreeMap::new();let mut phases=Vec::new();
    for (group,(c,es)) in candidates.into_iter().take(2).enumerate() {
        if c.iter().any(|v|marked.contains_key(v)) {return Err("overlapping cliques".into());}
        trace.cliques.push(c.len());
        for v in c {marked.insert(v,group);}
        phases.push(weight(p,mapped[es[0].2].0).clone());
        for (_,_,i) in es {used.insert(i);}
    }
    let mut unary:BTreeMap<_,_>=free.iter().map(|&v|(v,[C::one(bits),C::one(bits)])).collect();
    let mut scalar=C::one(bits);
    for v in 1..=p.n {
        let r=rep[2*(v-1)];
        if r==0 {continue;}
        if let Some(&x)=units.get(&var(r)) {
            scalar=scalar.mul(weight(p,if x==(r>0){v as i32}else{-(v as i32)}),bits);
        } else {
            let u=unary.get_mut(&var(r)).ok_or("missing unary owner")?;
            for (x,value) in u.iter_mut().enumerate() {
                *value=value.mul(weight(p,if (x==1)==(r>0){v as i32}else{-(v as i32)}),bits);
            }
        }
    }
    let mut graph:Graph=free.iter().map(|&v|(v,BTreeSet::new())).collect();
    let mut factors=Vec::new();
    for (i,(o,b)) in mapped.iter().enumerate() {
        if used.contains(&i){continue;}
        let scope=vec![var(b[0]).min(var(b[1])),var(b[0]).max(var(b[1]))];
        edge(&mut graph,scope[0],scope[1]);
        let vals=(0..4).map(|m|{
            let truth=b.iter().all(|&l|((m>>usize::from(var(l)==scope[1]))&1==1)==(l>0));
            weight(p,if truth{*o}else{-*o}).clone()
        }).collect();
        factors.push(Factor::ordinary(scope,vals));
    }
    for (v,[a,b]) in unary {
        if let Some(&group)=marked.get(&v) {
            let key=if group==0{(1,0)}else{(0,1)};
            factors.push(Factor{scope:vec![v],poly:BTreeMap::from([
                ((0,0),vec![a,C::zero(bits)]),(key,vec![C::zero(bits),b])])});
        } else {factors.push(Factor::ordinary(vec![v],vec![a,b]));}
    }
    while !graph.is_empty(){
        if start.elapsed().as_secs()>=105 {return Err("cardinality deadline".into());}
        let v=*graph.keys().min_by_key(|&&v|minfill(&graph,v)).ok_or("empty graph")?;
        let nb=graph.remove(&v).ok_or("missing pivot")?;
        trace.width=trace.width.max(nb.len());
        if nb.len()>15{return Err("cardinality width cap".into());}
        let (mut bucket,rest):(Vec<_>,Vec<_>)=factors.into_iter().partition(|f|f.scope.contains(&v));
        factors=rest;
        bucket.sort_by_key(|f|(f.poly.len(),f.scope.len()));
        let mut iter=bucket.into_iter();let mut a=iter.next().ok_or("empty pivot bucket")?;
        for b in iter {a=multiply(a,b,bits,trace,start)?;}
        factors.push(sum_out(a,v,bits));
        if factors.iter().map(Factor::cells).sum::<usize>()>2_000_000 {
            return Err("cardinality live cap".into());
        }
        for &u in &nb {
            let s=graph.get_mut(&u).ok_or("missing neighbor")?;
            s.remove(&v);s.extend(nb.iter().filter(|&&x|x!=u).copied());
        }
    }
    let mut iter=factors.into_iter();
    let mut f=iter.next().ok_or("empty final factor")?;
    for b in iter {f=multiply(f,b,bits,trace,start)?;}
    if !f.scope.is_empty(){return Err("uneliminated variables".into());}
    let mut powers=Vec::new();
    for phase in phases {
        let mut pows=vec![C::one(bits)];
        for k in 1..=66 {pows.push(pows[k-1].mul(&phase,bits));}
        powers.push(pows);
    }
    let mut total=C::zero(bits);
    for ((i,j),values) in f.poly {
        let a=i*i.saturating_sub(1)/2;
        let b=j*j.saturating_sub(1)/2;
        let term=values[0].mul(&powers[0][a],bits).mul(&powers[1][b],bits);
        total=total.add(&term,bits);
    }
    total=total.mul(&scalar,bits);
    if !total.re.is_finite()||!total.im.is_finite(){return Err("nonfinite cardinality result".into());}
    // Complete closure proves satisfiability, independently of numeric zero.
    Ok(Outcome{sat:true,count:total})
}

pub(super) fn run(mut args:impl Iterator<Item=std::ffi::OsString>, use_definitional:bool)->Result<(),String>{
    let digits:u32=args.next().and_then(|x|x.into_string().ok()).ok_or("missing digits")?.parse().map_err(|_|"invalid digits")?;
    let bits=match digits{30=>100,100=>333,_=>return Err("unsupported precision".into())};
    let path=args.next().ok_or("missing source")?;
    if args.next().is_some(){return Err("unexpected argument".into());}
    let t=Instant::now();
    let raw=std::fs::read_to_string(path).map_err(|e|e.to_string())?;
    let read_ns=t.elapsed().as_nanos();
    let t=Instant::now();let p=super::parse_emitted(&raw,bits)?;let parse_ns=t.elapsed().as_nanos();
    let t=Instant::now();let mut trace=Trace::default();let answer=if use_definitional {definitional::count(&p,bits,&mut trace)?} else {count(&p,&raw,bits,&mut trace)?};
    let solve_ns=t.elapsed().as_nanos();let t=Instant::now();
    println!("s {}",if answer.sat {"SATISFIABLE"} else {"UNSATISFIABLE"});
    println!("c r mpfr {} {}",answer.count.re.to_string_radix(10,Some(digits as usize+4)),answer.count.im.to_string_radix(10,Some(digits as usize+4)));
    let output_ns=t.elapsed().as_nanos();
    eprintln!("RESEARCH_MPFR_PHASE_NS read={read_ns} parse={parse_ns} solve={solve_ns} output={output_ns} nodes=0 cache_hits=0 decompositions=0 digits={digits} bits={bits}");
    if use_definitional {eprintln!("RESEARCH_MPFR_DEFINITIONAL gates={} free={} width={} peak={} work={}",trace.gates,trace.free,trace.width,trace.peak,trace.work);}
    else {eprintln!("RESEARCH_MPFR_CARDINALITY gates={} free={} cliques={:?} width={} peak={} work={}",trace.gates,trace.free,trace.cliques,trace.width,trace.peak,trace.work);}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_cliques_exact_signed_complex_oracle(){
        for bits in [100,333] {
            for zero in [false,true] {
                let mut raw=String::from("p cnf 12 18\nc t wmc\n");
                if zero {raw.push_str("c p weight 1 0 0 0\nc p weight -1 0 0 0\n");}
                else {raw.push_str("c p weight 1 -2 3 0\nc p weight -1 4 -1 0\n");}
                for o in 7..=12 {raw.push_str(&format!("c p weight {o} -1 2 0\n"));}
                for (o,a,b) in [(7,1,2),(8,1,3),(9,2,3),(10,4,5),(11,4,6),(12,5,6)] {
                    raw.push_str(&format!("{o} -{a} -{b} 0\n-{o} {a} 0\n-{o} {b} 0\n"));
                }
                let p=super::super::parse_emitted(&raw,bits).unwrap();
                let mut trace=Trace::default();let got=count(&p,&raw,bits,&mut trace).unwrap();
                let mut expected=C::zero(bits);
                for mask in 0..(1usize<<p.n) {
                    let truth=|l:i32|((mask>>(var(l)-1))&1==1)==(l>0);
                    if p.clauses.iter().all(|c|c.iter().copied().any(truth)) {
                        let mut product=C::one(bits);
                        for v in 1..=p.n {
                            let l=v as i32;
                            product=product.mul(weight(&p,if truth(l){l}else{-l}),bits);
                        }
                        expected=expected.add(&product,bits);
                    }
                }
                assert!(got.sat);assert_eq!(trace.cliques,vec![3,3]);
                assert_eq!(got.count.re,expected.re);assert_eq!(got.count.im,expected.im);
            }
        }
    }
    #[test]
    fn rejects_unclosed_constraints(){
        let raw="p cnf 3 1\nc t wmc\n1 2 3 0\n";
        let p=super::super::parse_emitted(raw,100).unwrap();
        assert!(count(&p,raw,100,&mut Trace::default()).is_err());
    }
}
