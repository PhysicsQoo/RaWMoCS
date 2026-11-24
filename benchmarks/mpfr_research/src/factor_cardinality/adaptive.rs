//! Research-only automatic routing from original weighted CNF structure.
//! No circuit name or benchmark-family label enters the decision.
use super::{extract_with_arity, recognize_cliques, phase_keys, count_bounded, definitional,
    idx,var,edge,minfill,Graph,Trace,Problem,Outcome};
use crate::{parse_emitted,trail_engine::TrailEngine,weighted_equivalence};
use std::collections::BTreeSet;
use std::time::{Duration,Instant};

#[derive(Debug,Default)]
struct Profile {
    gates:usize,free:usize,residual:usize,width:usize,work:u64,peak:u64,
    cliques:Vec<usize>,phase_eligible:bool,complete:bool,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Route { Cardinality, Definitional, Trail }
impl Route {
    fn name(self)->&'static str{
        match self{Self::Cardinality=>"cardinality",Self::Definitional=>"definitional",Self::Trail=>"trail"}
    }
}
fn profile(p:&Problem,raw:&str)->Result<Profile,String>{
    let start=Instant::now();
    let (gates,rep,units,residual)=extract_with_arity(p,2048,4)?;
    let free:BTreeSet<_>=rep.iter().copied().filter(|&l|l!=0&&!units.contains_key(&var(l))).map(var).collect();
    let mut out=Profile{gates:gates.len(),free:free.len(),residual:residual.len(),
        phase_eligible:crate::factor_indexed_fused::exact_phase_carrier(p,raw).is_ok(),..Profile::default()};
    if residual.is_empty()&&p.n<=1024&&gates.iter().all(|(_,a)|a.len()==2){
        if let Ok((_,c))=recognize_cliques(&gates,&rep,&free,&phase_keys(raw,p.n)){
            if c.len()>=2&&c[0].0.is_disjoint(&c[1].0){
                out.cliques=c.iter().take(2).map(|(v,_)|v.len()).collect();
            }
        }
    }
    let mut scopes:Vec<BTreeSet<usize>>=free.iter().map(|&v|BTreeSet::from([v])).collect();
    for (_,args) in gates{
        let b:Vec<_>=args.iter().map(|&l|rep[idx(l)]).collect();
        if b.contains(&0){return Err("unowned profile argument".into());}
        scopes.push(b.into_iter().map(var).filter(|v|!units.contains_key(v)).collect());
    }
    for c in residual{
        if c.len()>8{return Ok(out);}
        scopes.push(c.into_iter().map(var).collect());
    }
    if scopes.iter().flatten().any(|v|!free.contains(v)){return Err("profile scope ownership".into());}
    scopes.retain(|s|!s.is_empty());
    let mut graph:Graph=free.iter().map(|&v|(v,BTreeSet::new())).collect();
    for s in &scopes{for (i,&a) in s.iter().enumerate(){for &b in s.iter().skip(i+1){edge(&mut graph,a,b);}}}
    let mut live=scopes.iter().map(|s|1u64<<s.len()).sum::<u64>();out.peak=live;
    while !graph.is_empty(){
        if start.elapsed()>=Duration::from_secs(2){return Ok(out);}
        let v=*graph.keys().min_by_key(|&&v|minfill(&graph,v)).ok_or("profile graph")?;
        let nb=graph.remove(&v).ok_or("profile pivot")?;
        out.width=out.width.max(nb.len());
        if nb.len()>18{return Ok(out);}
        let size=1u64<<nb.len();let mut removed=0;let mut incident=0;
        scopes.retain(|s|{if s.contains(&v){removed+=1u64<<s.len();incident+=1;false}else{true}});
        out.work=out.work.saturating_add(2*size*incident);
        out.peak=out.peak.max(live.saturating_add(size));
        live=live.saturating_sub(removed);
        if !nb.is_empty(){live+=size;scopes.push(nb.clone());}
        if out.work>60_000_000||out.peak>2_000_000{return Ok(out);}
        for &u in &nb{
            let s=graph.get_mut(&u).ok_or("profile neighbor")?;
            s.remove(&v);s.extend(nb.iter().filter(|&&x|x!=u).copied());
        }
    }
    out.complete=true;
    Ok(out)
}
fn choose(p:&Profile)->Route{
    // Exact uniform cliques have their own internal limits. A failed
    // admission or time cap never changes the generic source semantics.
    if p.cliques.len()==2{return Route::Cardinality;}
    if p.complete&&p.width<=18&&p.work<=60_000_000&&p.peak<=2_000_000{
        return Route::Definitional;
    }
    Route::Trail
}
fn generic(p:&Problem,bits:u32,hybrid:bool)->Result<(Outcome,u64,u64,u64),String>{
    let (reduced,scope,_)=weighted_equivalence::reduce(p,bits);
    let mut trail=TrailEngine::new(&reduced,bits,10_000,1_000_000_000);
    trail.use_inline_weight_relabel=true;
    // Compare generic hybrid components against the same existing
    // search settings, rather than selecting by MQT family.
    trail.use_component_local_ve=hybrid;
    trail.use_forest_dp=hybrid;
    let answer=trail.count(scope,0)?;
    Ok((answer,trail.stats.nodes,trail.stats.cache_hits,trail.stats.decompositions))
}
fn solve(p:&Problem,raw:&str,bits:u32,hybrid:bool)->Result<(Outcome,Route,u64,u64,u64),String>{
    let t=Instant::now();
    let profile=match profile(p,raw){
        Ok(p)=>p,
        Err(reason)=>{eprintln!("ADAPTIVE_PROFILE_REJECT reason={reason}");Profile::default()}
    };
    let route=choose(&profile);
    eprintln!("ADAPTIVE_PROFILE gates={} free={} residual={} width={} work={} peak={} cliques={:?} phase_eligible={} complete={} planned={} profile_ns={}",
        profile.gates,profile.free,profile.residual,profile.width,profile.work,profile.peak,profile.cliques,profile.phase_eligible,profile.complete,route.name(),t.elapsed().as_nanos());
    if route!=Route::Trail{
        let mut trace=Trace::default();
        let attempt=Instant::now();
        let result=match route{
            Route::Cardinality=>count_bounded(p,raw,bits,&mut trace,15),
            Route::Definitional=>definitional::count_bounded(p,bits,&mut trace,15),
            Route::Trail=>unreachable!(),
        };
        match result{
            Ok(answer)=>return Ok((answer,route,0,0,0)),
            Err(reason)=>eprintln!("ADAPTIVE_FALLBACK from={} reason={} attempt_ns={}",route.name(),reason,attempt.elapsed().as_nanos()),
        }
    }
    let (answer,nodes,hits,components)=generic(p,bits,hybrid)?;
    Ok((answer,Route::Trail,nodes,hits,components))
}
pub(crate) fn run(mut args:impl Iterator<Item=std::ffi::OsString>,hybrid:bool)->Result<(),String>{
    let digits:u32=args.next().and_then(|x|x.into_string().ok()).ok_or("missing precision")?.parse().map_err(|_|"invalid precision")?;
    let bits=match digits{30=>100,100=>333,_=>return Err("unsupported precision".into())};
    let file=args.next().ok_or("missing source")?;
    if args.next().is_some(){return Err("unexpected arguments".into());}
    let t=Instant::now();
    let raw=std::fs::read_to_string(file).map_err(|e|e.to_string())?;
    let read_ns=t.elapsed().as_nanos();
    let t=Instant::now();let p=parse_emitted(&raw,bits)?;let parse_ns=t.elapsed().as_nanos();
    let t=Instant::now();let (answer,route,nodes,cache_hits,decompositions)=solve(&p,&raw,bits,hybrid)?;
    let solve_ns=t.elapsed().as_nanos();let t=Instant::now();
    println!("s {}",if answer.sat{"SATISFIABLE"}else{"UNSATISFIABLE"});
    println!("c r mpfr {} {}",answer.count.re.to_string_radix(10,Some(digits as usize+4)),answer.count.im.to_string_radix(10,Some(digits as usize+4)));
    let output_ns=t.elapsed().as_nanos();
    eprintln!("RESEARCH_MPFR_PHASE_NS read={read_ns} parse={parse_ns} solve={solve_ns} output={output_ns} nodes={nodes} cache_hits={cache_hits} decompositions={decompositions} digits={digits} bits={bits}");
    eprintln!("RESEARCH_MPFR_ADAPTIVE route={} hybrid={} completed=true",route.name(),hybrid);
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::C;
    #[test]
    fn adaptive_original_cnf_oracle_and_generic_hybrid(){
        let cases=[
            "p cnf 4 4\nc t wmc\nc p weight 4 -2 3 0\nc p weight -1 0 0 0\n4 -1 2 0\n-4 1 0\n-4 -2 0\n1 2 3 0\n",
            "p cnf 2 2\nc t wmc\nc p weight 2 -1 0 0\n1 0\n-1 0\n",
            "p cnf 3 0\nc t wmc\nc p weight 1 -1 0 0\n",
        ];
        for bits in [100,333]{
            for raw in cases{
                let p=parse_emitted(raw,bits).unwrap();
                let mut expected=Outcome::unsat(bits);
                for mask in 0..1usize<<p.n{
                    let yes=|l:i32|((mask>>(var(l)-1))&1!=0)==(l>0);
                    if p.clauses.iter().all(|c|c.iter().copied().any(yes)){
                        expected.sat=true;
                        let mut product=C::one(bits);
                        for v in 1..=p.n{
                            let l=v as i32;
                            product=product.mul(super::super::weight(&p,if yes(l){l}else{-l}),bits);
                        }
                        expected.count=expected.count.add(&product,bits);
                    }
                }
                for hybrid in [false,true]{
                    let (got,_,_,_,_)=solve(&p,raw,bits,hybrid).unwrap();
                    let (trail,_,_,_)=generic(&p,bits,hybrid).unwrap();
                    for out in [got,trail]{
                        assert_eq!(out.sat,expected.sat);
                        assert_eq!(out.count.re,expected.count.re);
                        assert_eq!(out.count.im,expected.count.im);
                    }
                }
            }
        }
    }
    #[test]
    fn policy_uses_structure_and_caps_only(){
        assert_eq!(choose(&Profile::default()),Route::Trail);
        let p=Profile{complete:true,width:10,work:1000,peak:100,..Profile::default()};
        assert_eq!(choose(&p),Route::Definitional);
        let p=Profile{cliques:vec![8,8],..Profile::default()};
        assert_eq!(choose(&p),Route::Cardinality);
    }
}
