//! Source-native exact exponent factor compilation and affine cutset planning.
//! This is a research planner, not a completed weighted model count.
use super::{extract_with_arity, weight, var, idx, edge, minfill, Graph, Problem};
use rug::{Integer,Rational};
use crate::factor_indexed_fused::{affine_leaf,decode_affine_packed,TaylorStats};
use std::collections::{BTreeMap,BTreeSet};
use std::time::Instant;

#[derive(Clone,Debug)]
struct Factor {
    scope: Vec<usize>,
    values: Vec<Option<u16>>,
}
struct Model {
    free: BTreeSet<usize>,
    factors: Vec<Factor>,
    gates: usize,
    xor: usize,
    initial_free: usize,
    degree: u32,
    delta: Rational,
}
fn exponent(p:&Problem,l:i32)->Result<u16,String>{
    let c=weight(p,l);
    if c.im!=0{return Err("non-real carrier".into());}
    if c.re==1{Ok(0)}else if c.re==2{Ok(1)}else{Err("invalid carrier".into())}
}
fn at(scope:&[usize],mask:usize,v:usize)->bool {
    mask&(1<<scope.binary_search(&v).expect("scope owns variable"))!=0
}
fn truth(scope:&[usize],mask:usize,l:i32,units:&BTreeMap<usize,bool>)->bool{
    units.get(&var(l)).copied().unwrap_or_else(||at(scope,mask,var(l)))==(l>0)
}
fn scope_of(lits:&[i32],units:&BTreeMap<usize,bool>)->Vec<usize>{
    lits.iter().copied().map(var).filter(|v|!units.contains_key(v)).collect::<BTreeSet<_>>().into_iter().collect()
}
fn table(scope:Vec<usize>, f:impl Fn(usize,&[usize])->Option<u16>)->Result<Factor,String>{
    if scope.len()>10{return Err("affine local scope cap".into());}
    let values=(0..1usize<<scope.len()).map(|m|f(m,&scope)).collect();
    Ok(Factor{scope,values})
}
fn replace_scope(scope:&[usize],pivot:usize,others:&[usize])->Vec<usize>{
    if !scope.contains(&pivot){return scope.to_vec();}
    scope.iter().copied().filter(|&v|v!=pivot).chain(others.iter().copied()).collect::<BTreeSet<_>>().into_iter().collect()
}
fn substitute(f:&Factor,pivot:usize,others:&[usize],parity:bool)->Result<Factor,String>{
    if !f.scope.contains(&pivot){return Ok(f.clone());}
    table(replace_scope(&f.scope,pivot,others),|mask,s|{
        let x=others.iter().fold(parity,|acc,&v|acc^at(s,mask,v));
        let old=f.scope.iter().enumerate().fold(0,|acc,(i,&v)|acc|(usize::from(if v==pivot{x}else{at(s,mask,v)})<<i));
        f.values[old]
    })
}
fn compile(p:&Problem,raw:&str)->Result<Model,String>{
    let (p,delta,degree)=crate::factor_indexed_fused::exact_phase_carrier(p,raw)?;
    let (gates,rep,units,residual)=extract_with_arity(&p,2048,4)?;
    let mut free:BTreeSet<_>=rep.iter().copied().filter(|&l|l!=0&&!units.contains_key(&var(l))).map(var).collect();
    let initial_free=free.len();
    let mut factors=Vec::new();
    // Every original literal weight is owned exactly once, including fixed,
    // free, complemented-equivalent and private output variables.
    let mut owners=vec![false;p.n+1];
    for (o,args) in &gates {
        if std::mem::replace(&mut owners[var(*o)],true){return Err("duplicate gate owner".into());}
        let b:Vec<_>=args.iter().map(|&l|rep[idx(l)]).collect();
        if b.contains(&0){return Err("unowned gate input".into());}
        let yes=exponent(&p,*o)?;let no=exponent(&p,-*o)?;
        factors.push(table(scope_of(&b,&units),|m,s|Some(if b.iter().all(|&l|truth(s,m,l,&units)){yes}else{no}))?);
    }
    for v in 1..=p.n {
        if owners[v]{continue;}
        let r=rep[2*(v-1)];
        if r==0{return Err("missing literal owner".into());}
        owners[v]=true;
        let yes=exponent(&p,v as i32)?;let no=exponent(&p,-(v as i32))?;
        factors.push(table(scope_of(&[r],&units),|m,s|Some(if truth(s,m,r,&units){yes}else{no}))?);
    }
    if owners.iter().skip(1).any(|x|!*x)||factors.len()!=p.n{return Err("incomplete weight ownership".into());}
    // A complete set of four distinct forbidden ternary assignments
    // of one parity encodes exactly the opposite parity, not an approximation.
    let mut groups:BTreeMap<Vec<usize>,Vec<BTreeSet<i32>>>=BTreeMap::new();
    for clause in residual {
        let scope=scope_of(&clause.iter().copied().collect::<Vec<_>>(),&units);
        groups.entry(scope).or_default().push(clause);
    }
    let mut rows=Vec::new();
    for (scope,clauses) in groups {
        let forbidden:BTreeSet<usize>=clauses.iter().map(|c|scope.iter().enumerate().fold(0,|m,(i,&v)|m|(usize::from(c.contains(&-(v as i32)))<<i))).collect();
        let parity=forbidden.iter().next().map(|m|m.count_ones()%2);
        if scope.len()==3&&clauses.len()==4&&forbidden.len()==4&&forbidden.iter().all(|m|Some(m.count_ones()%2)==parity) {
            rows.push((scope,parity==Some(0)));
        }else{
            for c in clauses {
                factors.push(table(scope.clone(),|m,s|if c.iter().any(|&l|truth(s,m,l,&units)){Some(0)}else{None})?);
            }
        }
    }
    let mut xor=0;
    // Restrict this first compiler to leaf pivots: no silently dropping
    // dependent affine equations. If no unique pivot exists, fail closed.
    while !rows.is_empty(){
        let mut choices=Vec::new();
        for (ri,(scope,_)) in rows.iter().enumerate(){
            for &v in scope {
                if rows.iter().enumerate().any(|(j,(s,_))|j!=ri&&s.contains(&v)){continue;}
                let others:Vec<_>=scope.iter().copied().filter(|&u|u!=v).collect();
                let mut growth=0usize;let mut touched=0usize;
                for f in &factors {
                    if f.scope.contains(&v) {
                        touched+=1;
                        let new=replace_scope(&f.scope,v,&others);
                        growth=growth.saturating_add((1usize<<new.len()).saturating_sub(1usize<<f.scope.len()));
                    }
                }
                choices.push((growth,touched,v,ri));
            }
        }
        let (_,_,pivot,ri)=choices.into_iter().min().ok_or("affine dependency unsupported")?;
        let (scope,parity)=rows.remove(ri);
        let others:Vec<_>=scope.into_iter().filter(|&v|v!=pivot).collect();
        if !free.remove(&pivot)||others.iter().any(|v|!free.contains(v)){return Err("affine scope ownership".into());}
        factors=factors.iter().map(|f|substitute(f,pivot,&others,parity)).collect::<Result<_,_>>()?;
        xor+=1;
    }
    if factors.iter().any(|f|f.scope.iter().any(|v|!free.contains(v))){return Err("unowned affine factor scope".into());}
    // Constants are retained; remove only fully neutral exponent factors.
    // Variables remain in free, even if all their weights are unity.
    factors.retain(|f|f.values.iter().any(|v|*v!=Some(0)));
    Ok(Model{free,factors,gates:gates.len(),xor,initial_free,degree,delta})
}

#[derive(Clone,Debug)]
struct Plan { width:usize, cells:u64, lookups:u64, peak:u64, bag:Vec<usize>, order:Vec<usize> }
const TOTAL_LOOKUPS:u64=360_000_000;
const LIVE_CELLS:u64=400_000;
fn plan(model:&Model,fixed:&BTreeSet<usize>,start:Instant)->Result<Plan,String>{
    plan_weighted(model,fixed,start,0)
}
fn plan_weighted(model:&Model,fixed:&BTreeSet<usize>,start:Instant,penalty:usize)->Result<Plan,String>{
    let mut g:Graph=model.free.difference(fixed).map(|&v|(v,BTreeSet::new())).collect();
    let mut scopes:Vec<BTreeSet<usize>>=model.factors.iter().map(|f|f.scope.iter().copied().filter(|v|!fixed.contains(v)).collect()).filter(|s:&BTreeSet<_>|!s.is_empty()).collect();
    for s in &scopes {
        for (i,&a) in s.iter().enumerate(){for &b in s.iter().skip(i+1){edge(&mut g,a,b);}}
    }
    let mut result=Plan{width:0,cells:0,lookups:0,peak:0,bag:Vec::new(),order:Vec::new()};
    let mut live=scopes.iter().map(|s|1u64.checked_shl(s.len() as u32).unwrap_or(u64::MAX)).fold(0u64,u64::saturating_add);
    result.peak=live;
    while !g.is_empty(){
        if start.elapsed().as_secs()>=60{return Err("affine planning deadline".into());}
        let v=*g.keys().min_by_key(|&&v|{
            let (fill,degree,tie)=minfill(&g,v);
            (fill.saturating_add(penalty.saturating_mul(degree)),degree,tie)
        }).ok_or("empty graph")?;
        let nb=g.remove(&v).ok_or("missing pivot")?;
        result.order.push(v);
        if nb.len()>result.width{
            result.width=nb.len();
            result.bag=nb.iter().copied().chain(std::iter::once(v)).collect();
        }
        let size=1u64.checked_shl((nb.len()+1) as u32).ok_or("plan width overflow")?;
        result.cells=result.cells.saturating_add(size);
        let before=scopes.len();
        let mut removed=0u64;
        scopes.retain(|s|{
            if s.contains(&v){removed=removed.saturating_add(1u64.checked_shl(s.len() as u32).unwrap_or(u64::MAX));false}else{true}
        });
        let incident=before-scopes.len();
        result.lookups=result.lookups.saturating_add(size.saturating_mul(incident.max(1) as u64));
        if incident>0{
            result.peak=result.peak.max(live.saturating_add(size/2));
            live=live.saturating_sub(removed);
            if !nb.is_empty(){scopes.push(nb.clone());live=live.saturating_add(size/2);}
        }
        for &u in &nb{
            let s=g.get_mut(&u).ok_or("missing neighbor")?;
            s.remove(&v);s.extend(nb.iter().filter(|&&x|x!=u).copied());
        }
    }
    Ok(result)
}
fn choose(model:&Model,start:Instant)->Result<(Vec<usize>,Plan),String>{
    let mut fixed=BTreeSet::new();
    let mut ordered=Vec::new();
    let mut best=plan(model,&fixed,start)?;
    println!("AFFINE_PLAN depth=0 leaves=1 width={} cells={} lookups={}",best.width,best.cells,best.lookups);
    for depth in 1..=6{
        if best.width<=18{break;}
        let mut candidates=best.bag.clone();
        // Bound probes, prioritize variables occurring in more factors.
        candidates.sort_by_key(|v|(std::cmp::Reverse(model.factors.iter().filter(|f|f.scope.contains(v)).count()),*v));
        let mut selected=None;
        for v in candidates.into_iter().take(8){
            let mut trial=fixed.clone();trial.insert(v);
            let p=plan(model,&trial,start)?;
            let key=(p.width,p.cells,p.lookups,v);
            if selected.as_ref().is_none_or(|(old,_)|key<*old){selected=Some((key,p));}
        }
        let ((_,_,_,v),p)=selected.ok_or("no cutset pivot")?;
        fixed.insert(v);ordered.push(v);best=p;
        let leaves=1u64<<depth;
        println!("AFFINE_PLAN depth={} leaves={} width={} cells={} lookups={} pivot={}",depth,leaves,best.width,best.cells.saturating_mul(leaves),best.lookups.saturating_mul(leaves),v);
    }
    Ok((ordered,best))
}
// The original width-only plan is the seed, not a resource admission.
// Search one additional cutset variable from its maximal bag and a small
// deterministic degree-penalty portfolio. No circuit-specific IDs.
fn resource_choose(m:&Model,start:Instant)->Result<(Vec<usize>,Plan),String>{
    let (fixed,base)=choose(m,start)?;
    let key=|p:&Plan,depth:usize|{
        let work=p.lookups.saturating_mul(1u64<<depth);
        (p.width.saturating_sub(18),p.peak.saturating_sub(LIVE_CELLS),work.saturating_sub(TOTAL_LOOKUPS),work,p.peak)
    };
    if key(&base,fixed.len()).0==0&&base.peak<=LIVE_CELLS&&base.lookups.saturating_mul(1u64<<fixed.len())<=TOTAL_LOOKUPS{
        return Ok((fixed,base));
    }
    let mut best=(fixed.clone(),base.clone());
    if fixed.len()<6 {
        let mut candidates=base.bag.clone();candidates.sort_unstable();candidates.dedup();
        for v in candidates.into_iter().take(24){
            if fixed.contains(&v){continue;}
            let mut next=fixed.clone();next.push(v);
            let set=next.iter().copied().collect();
            for penalty in [0,1,3]{
                let p=plan_weighted(m,&set,start,penalty)?;
                if key(&p,next.len())<key(&best.1,best.0.len()){best=(next.clone(),p);}
            }
        }
    }
    eprintln!("AFFINE_RESOURCE_PLAN width={} peak_upper={} lookup_upper={} leaves={} cutset={:?}",
        best.1.width,best.1.peak,best.1.lookups.saturating_mul(1u64<<best.0.len()),1usize<<best.0.len(),best.0);
    Ok(best)
}
// Sum exponents of factors with the same support. This is multiplication
// of monomials, with no numerical rounding or weight duplication.
fn merged(factors:&[Factor])->Result<Vec<Factor>,String>{
    let mut groups:BTreeMap<Vec<usize>,Vec<Option<u16>>>=BTreeMap::new();
    for f in factors{
        let row=groups.entry(f.scope.clone()).or_insert_with(||vec![Some(0);f.values.len()]);
        for (a,b) in row.iter_mut().zip(&f.values){
            *a=match (*a,*b){
                (Some(a),Some(b))=>Some(a.checked_add(b).ok_or("affine exponent overflow")?),
                _=>None,
            };
        }
    }
    Ok(groups.into_iter().filter(|(_,v)|v.iter().any(|x|*x!=Some(0))).map(|(scope,values)|Factor{scope,values}).collect())
}
type Tables=Vec<(Vec<usize>,Vec<u16>)>;
fn conditioned(m:&Model,fixed:&[usize],mask:usize)->Result<Tables,String>{
    let values:BTreeMap<_,_>=fixed.iter().enumerate().map(|(i,&v)|(v,mask&(1<<i)!=0)).collect();
    let mut fs=Vec::new();
    for f in &m.factors{
        let scope:Vec<_>=f.scope.iter().copied().filter(|v|!values.contains_key(v)).collect();
        fs.push(table(scope,|i,s|{
            let old=f.scope.iter().enumerate().fold(0,|a,(j,&v)|a|(usize::from(values.get(&v).copied().unwrap_or_else(||at(s,i,v)))<<j));
            f.values[old]
        })?);
    }
    merged(&fs)?.into_iter().map(|f|Ok((f.scope,f.values.into_iter().collect::<Option<Vec<_>>>().ok_or("affine incomplete support")?))).collect()
}
fn contract(m:&Model,fixed:&[usize],p:&Plan,modbits:u32,phase:&Integer,start:Instant,trace:&mut TaylorStats)->Result<Integer,String>{
    if p.width>18||fixed.len()>6{return Err("affine plan cap".into());}
    let leaves=1usize<<fixed.len();trace.planned=leaves;
    // Condition before charging the upper bound: fixed inputs can turn
    // an exponent table into unity, and identical supports can merge.
    // Keep the original aggregate work budget, not the graph-only proxy.
    let mut prepared=Vec::with_capacity(leaves);let mut upper=0u64;let mut peak=0u64;
    for mask in 0..leaves{
        let tables=conditioned(m,fixed,mask)?;
        let mut scopes:Vec<BTreeSet<usize>>=tables.iter().filter(|(s,_)|!s.is_empty()).map(|(s,_)|s.iter().copied().collect()).collect();
        let mut live=scopes.iter().map(|s|1u64<<s.len()).sum::<u64>();
        peak=peak.max(live);
        for &v in &p.order{
            let mut union=BTreeSet::new();let mut count=0u64;let mut removed=0u64;
            scopes.retain(|s|{
                if s.contains(&v){union.extend(s.iter().copied());count+=1;removed+=1u64<<s.len();false}else{true}
            });
            if count==0{continue;}
            union.remove(&v);
            if union.len()>18{return Err("affine conditioned width cap".into());}
            let size=1u64<<union.len();
            upper=upper.saturating_add(2*size*count);
            peak=peak.max(live.saturating_add(size));
            if peak>LIVE_CELLS{return Err("affine preflight live cap".into());}
            live=live.saturating_sub(removed);
            if !union.is_empty(){scopes.push(union);live+=size;}
        }
        if !scopes.is_empty(){return Err("affine conditioned scope incomplete".into());}
        if upper>TOTAL_LOOKUPS{return Err("affine preflight lookup cap".into());}
        prepared.push(tables);
    }
    eprintln!("AFFINE_TAYLOR_PREFLIGHT conditioned_lookup_upper={upper} peak_live_upper={peak} leaves={leaves}");
    let mut sum=Integer::from(0);
    let n=m.free.iter().copied().max().unwrap_or(0);
    for tables in prepared{
        let v=affine_leaf(&tables,&p.order,n,modbits,phase,start,trace)?;
        sum+=v;sum.keep_signed_bits_mut(modbits);
        eprintln!("AFFINE_TAYLOR_LEAF completed={} planned={} lookups={} arithmetic={} peak_pool={} peak_live={} gc={}",trace.completed,trace.planned,trace.actual_work,trace.arithmetic,trace.peak_pool,trace.peak_live,trace.gc);
    }
    Ok(sum)
}
pub(crate) fn run_taylor(mut args:impl Iterator<Item=std::ffi::OsString>)->Result<(),String>{
    let digits:u32=args.next().and_then(|x|x.into_string().ok()).ok_or("missing digits")?.parse().map_err(|_|"invalid digits")?;
    let bits=match digits{30=>100,100=>333,_=>return Err("unsupported precision".into())};
    let file=args.next().ok_or("missing source")?;
    if args.next().is_some(){return Err("unexpected arguments".into());}
    let t=Instant::now();let raw=std::fs::read_to_string(file).map_err(|e|e.to_string())?;let read_ns=t.elapsed().as_nanos();
    let t=Instant::now();let p=super::super::parse_emitted(&raw,bits)?;let parse_ns=t.elapsed().as_nanos();
    let start=Instant::now();
    let mut m=compile(&p,&raw)?;
    // Current research route proves a bijection with all assignments to
    // the free variables. Non-affine residual clauses fail closed.
    if m.factors.iter().any(|f|f.values.iter().any(Option::is_none)){return Err("affine hard support not closed".into());}
    let before=m.factors.len();m.factors=merged(&m.factors)?;
    let n=Integer::from(1)<<(m.free.len() as u32);
    let k=if m.delta==0{1}else{(if bits==100{4}else{9}).min(m.degree+1)};
    let bound=(0..k).map(|j|&n*Integer::from(m.degree).binomial(j)).max().ok_or("affine coefficient bound")?;
    let base=bound.significant_bits()+1;
    let modbits=base.checked_mul(k).ok_or("affine packing overflow")?;
    let phase=(Integer::from(1)<<base)-1;
    let (fixed,plan)=resource_choose(&m,start)?;
    eprintln!("AFFINE_TAYLOR_PLAN factors_before={before} factors_after={} free={} width={} leaves={} lookup_upper={} terms={k} base_bits={base}",m.factors.len(),m.free.len(),plan.width,1usize<<fixed.len(),plan.lookups.saturating_mul(1u64<<fixed.len()));
    let mut trace=TaylorStats::default();
    let packed=contract(&m,&fixed,&plan,modbits,&phase,start,&mut trace)?;
    if trace.completed!=trace.planned{return Err("affine incomplete leaves".into());}
    let answer=decode_affine_packed(packed,&n,&m.delta,(m.degree,k,base),(bits,digits),start)?;
    let solve_ns=start.elapsed().as_nanos();let t=Instant::now();
    println!("s SATISFIABLE");
    println!("c r mpfr {} {}",answer.count.re.to_string_radix(10,Some(digits as usize+4)),answer.count.im.to_string_radix(10,Some(digits as usize+4)));
    let output_ns=t.elapsed().as_nanos();
    eprintln!("RESEARCH_MPFR_PHASE_NS read={read_ns} parse={parse_ns} solve={solve_ns} output={output_ns} nodes=0 cache_hits=0 decompositions=0 digits={digits} bits={bits}");
    eprintln!("RESEARCH_MPFR_AFFINE_TAYLOR planned={} completed={} terms={k} base_bits={base} model_bits={} lookups={} arithmetic={} peak_pool={} peak_live={} gc={} certified=true",trace.planned,trace.completed,n.significant_bits(),trace.actual_work,trace.arithmetic,trace.peak_pool,trace.peak_live,trace.gc);
    Ok(())
}

pub(crate) fn run(mut args:impl Iterator<Item=std::ffi::OsString>)->Result<(),String>{
    let file=args.next().ok_or("missing source")?;
    if args.next().is_some(){return Err("unexpected arguments".into());}
    let t=Instant::now();
    let raw=std::fs::read_to_string(file).map_err(|e|e.to_string())?;
    let p=super::super::parse_emitted(&raw,100)?;
    let m=compile(&p,&raw)?;
    println!("AFFINE_COMPILE variables={} private_gates={} initial_free={} xor={} free={} factors={} degree={} delta={}",p.n,m.gates,m.initial_free,m.xor,m.free.len(),m.factors.len(),m.degree,m.delta);
    let (fixed,plan)=choose(&m,t)?;
    let leaves=1u64<<fixed.len();
    println!("AFFINE_PLAN_FINAL width={} leaves={} cells={} lookups={} cutset={:?} elapsed_ms={} planner_only=true",plan.width,leaves,plan.cells.saturating_mul(leaves),plan.lookups.saturating_mul(leaves),fixed,t.elapsed().as_millis());
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_histogram_matches_original_with_signed_and_xor(){
        for parity in [false,true]{
            let mut raw=String::from("p cnf 7 10\nc t wmc\nc p weight 1 -1 1e-16 0\nc p weight -2 -1 1e-16 0\nc p weight -4 -1 1e-16 0\nc p weight 5 -1 1e-16 0\nc p weight -7 -1 1e-16 0\n");
            for mask in 0usize..8{
                if (mask.count_ones()%2==1)!=parity{
                    for v in 1..=3{raw.push_str(&format!("{} ",if mask&(1<<(v-1))==0{v}else{-v}));}
                    raw.push_str("0\n");
                }
            }
            raw.push_str("4 -1 2 0\n-4 1 0\n-4 -2 0\n5 -4 0\n-5 4 0\n6 0\n");
            // Variable 4 is not private here due to the equivalence.
            // Use a genuinely private signed gate in a separate test below.
            let p=super::super::super::parse_emitted(&raw,100).unwrap();
            let m=compile(&p,&raw).unwrap();
            assert_eq!(m.xor,1);
            let (carrier,_,d)=crate::factor_indexed_fused::exact_phase_carrier(&p,&raw).unwrap();
            let mut expected=vec![0usize;d as usize+1];
            for mask in 0..1usize<<p.n{
                let yes=|l:i32|((mask>>(var(l)-1))&1!=0)==(l>0);
                if p.clauses.iter().all(|c|c.iter().copied().any(yes)){
                    let e=(1..=p.n).map(|v|exponent(&carrier,if yes(v as i32){v as i32}else{-(v as i32)}).unwrap() as usize).sum::<usize>();
                    expected[e]+=1;
                }
            }
            let mut actual=vec![0usize;d as usize+1];
            let free:Vec<_>=m.free.iter().copied().collect();
            for mask in 0..1usize<<free.len(){
                let mut e=Some(0usize);
                for f in &m.factors{
                    let idx=f.scope.iter().enumerate().fold(0,|a,(i,&v)|a|(usize::from(at(&free,mask,v))<<i));
                    e=e.zip(f.values[idx]).map(|(a,b)|a+b as usize);
                }
                if let Some(e)=e{actual[e]+=1;}
            }
            assert_eq!(actual,expected);
        }
    }
    #[test]
    fn exact_ring_cutset_partition_preserves_polynomial(){
        let raw="p cnf 4 7\nc t wmc\nc p weight 4 -1 1e-16 0\nc p weight -1 -1 1e-16 0\n1 2 -3 0\n1 -2 3 0\n-1 2 3 0\n-1 -2 -3 0\n4 -1 -2 0\n-4 1 0\n-4 2 0\n";
        let p=super::super::super::parse_emitted(raw,100).unwrap();
        let mut m=compile(&p,raw).unwrap();
        m.factors=merged(&m.factors).unwrap();
        let start=Instant::now();
        let fixed=vec![*m.free.first().unwrap()];
        let plan=plan(&m,&fixed.iter().copied().collect(),start).unwrap();
        let mut trace=TaylorStats::default();
        let got=contract(&m,&fixed,&plan,64,&Integer::from(7),start,&mut trace).unwrap();
        assert_eq!(got,22); // P(z)=1+3z from independent exhaustive truth table
        assert_eq!(trace.completed,2);
    }
    #[test]
    fn private_gate_weight_moves_through_affine_pivot(){
        let raw="p cnf 4 7\nc t wmc\nc p weight 4 -1 1e-16 0\nc p weight -1 -1 1e-16 0\n1 2 -3 0\n1 -2 3 0\n-1 2 3 0\n-1 -2 -3 0\n4 -1 -2 0\n-4 1 0\n-4 2 0\n";
        let p=super::super::super::parse_emitted(raw,100).unwrap();
        let m=compile(&p,raw).unwrap();
        assert_eq!((m.gates,m.xor,m.free.len()),(1,1,2));
        let free:Vec<_>=m.free.iter().copied().collect();
        let mut hist=[0;3];
        for mask in 0..4{
            let e=m.factors.iter().map(|f|{
                let index=f.scope.iter().enumerate().fold(0,|a,(i,&v)|a|(usize::from(at(&free,mask,v))<<i));
                f.values[index].unwrap() as usize
            }).sum::<usize>();
            hist[e]+=1;
        }
        assert_eq!(hist,[1,3,0]);
    }
}
