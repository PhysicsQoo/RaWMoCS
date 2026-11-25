//! Research-only automatic routing from original weighted CNF structure.
//! No circuit name or benchmark-family label enters the decision.
use super::{extract_with_limits, recognize_cliques, phase_keys, count_bounded, definitional,
    idx,var,edge,minfill,Graph,Trace,Problem,Outcome};
use crate::{parse_emitted,trail_engine::TrailEngine,weighted_equivalence};
use std::collections::{BTreeMap,BTreeSet};
use std::time::{Duration,Instant};

#[derive(Debug,Default)]
struct Profile {
    gates:usize,free:usize,residual:usize,width:usize,work:u64,peak:u64,
    cliques:Vec<usize>,phase_eligible:bool,complete:bool,
    order:Vec<usize>,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Route { Cardinality, Definitional, Conditioned, Trail }
impl Route {
    fn name(self)->&'static str{
        match self{Self::Cardinality=>"cardinality",Self::Definitional=>"definitional",Self::Conditioned=>"conditioned",Self::Trail=>"trail"}
    }
}
fn profile(p:&Problem,raw:&str)->Result<Profile,String>{
    let start=Instant::now();
    let (gates,rep,units,residual)=extract_with_limits(p,4096,16_000,4)?;
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
        if c.len()>12{return Ok(out);}
        scopes.push(c.into_iter().map(var).collect());
    }
    if scopes.iter().flatten().any(|v|!free.contains(v)){return Err("profile scope ownership".into());}
    let mut best:Option<Plan>=None;
    // Large sparse graphs can exhaust min-fill's score budget long before
    // reaching a useful elimination frontier. A cheap minimum-degree
    // plan is a resource-checked candidate, not a relaxation of caps.
    if free.len()>=512{
        let plan=plan_scopes(&free,&scopes,start,DEGREE_ONLY,true)?;
        let admitted=plan.complete;
        best=Some(plan);
        if admitted{
            let b=best.take().expect("degree plan");
            out.width=b.width;out.work=b.work;out.peak=b.peak;out.complete=b.complete;out.order=b.order;
            return Ok(out);
        }
    }
    // Keep the original order for cheap admitted instances. For costly
    // instances use a small deterministic resource portfolio; not a family ID.
    for (penalty,reverse) in [(0,false),(1,true),(2,true),(0,true),(4,true),(2,false)]{
        let plan=plan_scopes(&free,&scopes,start,penalty,reverse)?;
        if best.as_ref().is_none_or(|b|(!plan.complete,plan.work,plan.peak,plan.width)<(!b.complete,b.work,b.peak,b.width)){
            best=Some(plan);
        }
        if best.as_ref().is_some_and(|b|b.complete&&b.work<=1_000_000){break;}
        if start.elapsed()>=Duration::from_secs(2){break;}
    }
    if let Some(b)=best{
        out.width=b.width;out.work=b.work;out.peak=b.peak;out.complete=b.complete;out.order=b.order;
    }
    Ok(out)
}
#[derive(Default)]
struct Plan{width:usize,work:u64,peak:u64,complete:bool,order:Vec<usize>}
type Score=(usize,usize,usize,usize);
const DEGREE_ONLY:usize=usize::MAX;
fn score(g:&Graph,v:usize,penalty:usize,reverse:bool)->Score{
    let degree=g[&v].len();
    let cost=if penalty==DEGREE_ONLY{degree}else{
        let (fill,_,_)=minfill(g,v);
        fill.saturating_add(penalty.saturating_mul(degree))
    };
    (cost,degree,if reverse{usize::MAX-v}else{v},v)
}
fn plan_scopes(free:&BTreeSet<usize>,original:&[BTreeSet<usize>],start:Instant,penalty:usize,reverse:bool)->Result<Plan,String>{
    let mut scopes:Vec<Option<BTreeSet<usize>>>=original.iter().cloned().map(Some).collect();
    let mut graph:Graph=free.iter().map(|&v|(v,BTreeSet::new())).collect();
    let mut incident:BTreeMap<usize,BTreeSet<usize>>=free.iter().map(|&v|(v,BTreeSet::new())).collect();
    for (id,s) in original.iter().enumerate(){
        for (i,&a) in s.iter().enumerate(){
            incident.get_mut(&a).ok_or("profile factor owner")?.insert(id);
            for &b in s.iter().skip(i+1){edge(&mut graph,a,b);}
        }
    }
    let mut live=original.iter().map(|s|1u64<<s.len()).sum::<u64>();
    let mut out=Plan{peak:live,..Plan::default()};
    if live>2_000_000{return Ok(out);}
    let mut scores=BTreeMap::new();
    let mut queue=BTreeSet::new();
    for &v in graph.keys(){
        if start.elapsed()>=Duration::from_secs(2){return Ok(out);}
        let key=score(&graph,v,penalty,reverse);
        scores.insert(v,key);queue.insert(key);
    }
    while let Some(key)=queue.pop_first(){
        if start.elapsed()>=Duration::from_secs(2){return Ok(out);}
        let v=key.3;
        scores.remove(&v);
        let nb=graph.remove(&v).ok_or("profile pivot")?;
        out.width=out.width.max(nb.len());
        if nb.len()>20{return Ok(out);}
        let size=1u64<<nb.len();
        let ids=incident.remove(&v).ok_or("profile bucket owner")?;
        let mut removed=0;
        for &id in &ids{
            let scope=scopes[id].take().ok_or("profile stale factor")?;
            removed+=1u64<<scope.len();
            for u in scope{
                if u!=v&&!incident.get_mut(&u).ok_or("profile incident owner")?.remove(&id){
                    return Err("profile missing incidence".into());
                }
            }
        }
        out.work=out.work.saturating_add(2*size*ids.len() as u64);
        out.peak=out.peak.max(live.saturating_add(size));
        live=live.checked_sub(removed).ok_or("profile live underflow")?;
        live+=size;
        if out.work>60_000_000||out.peak>2_000_000{return Ok(out);}
        let id=scopes.len();
        for &u in &nb{incident.get_mut(&u).ok_or("profile output owner")?.insert(id);}
        scopes.push(Some(nb.clone()));
        // Only neighbors of the removed vertex or vertices whose
        // neighbor-induced graph sees a new fill edge can change score.
        // The latter are covered by the old neighbors of every nb member.
        let mut affected=nb.clone();
        if penalty!=DEGREE_ONLY{
            for &u in &nb{
                affected.extend(graph.get(&u).ok_or("profile neighbor")?.iter().copied());
            }
        }
        affected.remove(&v);
        for &u in &nb{
            let s=graph.get_mut(&u).ok_or("profile neighbor")?;
            s.remove(&v);s.extend(nb.iter().filter(|&&x|x!=u).copied());
        }
        for u in affected{
            let old=scores.get_mut(&u).ok_or("profile score owner")?;
            if !queue.remove(old){return Err("profile stale score".into());}
            *old=score(&graph,u,penalty,reverse);
            queue.insert(*old);
        }
        out.order.push(v);
    }
    if !graph.is_empty()||!incident.is_empty(){return Err("profile incomplete queue".into());}
    out.complete=true;
    Ok(out)
}

fn choose(p:&Profile)->Route{
    // Exact uniform cliques have their own internal limits. A failed
    // admission or time cap never changes the generic source semantics.
    if p.cliques.len()==2{return Route::Cardinality;}
    if p.complete&&p.width<=20&&p.work<=60_000_000&&p.peak<=2_000_000{
        return Route::Definitional;
    }
    if p.gates>0&&p.residual==0&&(19..=21).contains(&p.width){return Route::Conditioned;}
    Route::Trail
}
// One bounded, static balanced BFS-layer separator on the reduced primal
// graph. Every path from an earlier layer to a later layer crosses this
// layer. Scores only reorder exact Shannon branches; no assignments dropped.
fn separator_layer(p:&Problem,scope:&[usize])->Option<(Vec<u16>,usize,usize,usize)>{
    if p.n>4096 || scope.len()<8 || scope.len()>4096 || p.clauses.len()>20000{return None;}
    let mut graph=vec![BTreeSet::new();p.n+1];
    let mut edges=0usize;
    let active: BTreeSet<_>=scope.iter().copied().collect();
    if active.len()!=scope.len(){return None;}
    for c in &p.clauses{
        if c.len()>32{return None;}
        let vs:BTreeSet<_>=c.iter().map(|l|l.unsigned_abs() as usize).collect();
        if vs.iter().any(|v|!active.contains(v)){return None;}
        let vs:Vec<_>=vs.into_iter().collect();
        for (i,&v) in vs.iter().enumerate(){
            for &u in &vs[i+1..]{
                if graph[v].insert(u){graph[u].insert(v);edges+=1;if edges>100000{return None;}}
            }
        }
    }
    let bfs=|root:usize|{
        let mut d=vec![usize::MAX;p.n+1];
        let mut q=vec![root];d[root]=0;
        let mut i=0;
        while i<q.len(){let v=q[i];i+=1;for &u in &graph[v]{
            if d[u]==usize::MAX{d[u]=d[v]+1;q.push(u);}
        }}
        (d,q)
    };
    // Root from largest connected component, rather than an isolated
    // free variable; at most two extra sweeps over that component.
    let mut seen=vec![false;p.n+1];let mut largest=Vec::new();
    for &v in scope{
        if seen[v]{continue;}
        let (_,q)=bfs(v);for &u in &q{seen[u]=true;}
        if q.len()>largest.len(){largest=q;}
    }
    let first=*largest.first()?;
    let (d,q)=bfs(first);
    let far=*q.iter().max_by_key(|&&v|(d[v],std::cmp::Reverse(v)))?;
    let (d,q)=bfs(far);
    let max=*q.iter().map(|v|&d[*v]).max()?;
    let mut sizes=vec![0usize;max+1];
    for &v in &q{sizes[d[v]]+=1;}
    let mut left=0;let mut best=None;
    for (i,&size) in sizes.iter().enumerate(){
        let right=q.len()-left-size;
        if left>0&&right>0&&left.max(right)*4<=q.len()*3&&size*4<=q.len(){
            let key=(size,left.max(right),i,left,right);
            if best.is_none_or(|b|key<b){best=Some(key);}
        }
        left+=size;
    }
    let (size,_,layer,left,right)=best?;
    let mut scores=vec![0u16;p.n+1];
    for v in q{if d[v]==layer{scores[v]=1;}}
    Some((scores,size,left,right))
}
fn generic(p:&Problem,bits:u32,hybrid:bool,variant:u8)->Result<(Outcome,u64,u64,u64),String>{
    let (reduced,scope,_)=weighted_equivalence::reduce(p,bits);
    let mut trail=TrailEngine::new(&reduced,bits,50_000,1_000_000_000);
    trail.use_inline_weight_relabel=true;
    if variant>0{
        trail.use_parent_ids=true;
        trail.use_compact_residual=true;
        trail.use_prehashed_cache=true;
        trail.use_fused_compact=true;
    }
    if variant==2{
        if let Some((scores,size,left,right))=separator_layer(&reduced,&scope){
            eprintln!("MCC_SEPARATOR_LAYER size={size} left={left} right={right}");
            trail.primal_scores=Some(scores);
        }else{eprintln!("MCC_SEPARATOR_LAYER rejected=true");}
    }
    // Compare generic hybrid components against the same existing
    // search settings, rather than selecting by MQT family.
    trail.use_component_local_ve=hybrid;
    trail.use_forest_dp=hybrid;
    let result=trail.count(scope,0);
    if variant>0{
        eprintln!("MCC_TRAIL_FULL variant={variant} nodes={} hits={} decompositions={} assignments={} separator_overrides={}",
            trail.stats.nodes,trail.stats.cache_hits,trail.stats.decompositions,trail.assignments,trail.primal_overrides);
    }
    let answer=result?;
    Ok((answer,trail.stats.nodes,trail.stats.cache_hits,trail.stats.decompositions))
}
fn solve(p:&Problem,raw:&str,bits:u32,hybrid:bool,variant:u8)->Result<(Outcome,Route,u64,u64,u64),String>{
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
            Route::Definitional=>definitional::count_planned(p,bits,&mut trace,if profile.width>=19{30}else{15},&profile.order),
            Route::Conditioned=>definitional::count_conditioned(p,bits,&mut trace,45),
            Route::Trail=>unreachable!(),
        };
        match result{
            Ok(answer)=>{
                eprintln!("ADAPTIVE_CONTRACTION width={} work={} peak={} planned_width={} planned_work={} planned_peak={}",trace.width,trace.work,trace.peak,profile.width,profile.work,profile.peak);
                return Ok((answer,route,0,0,0));
            },
            Err(reason)=>eprintln!("ADAPTIVE_FALLBACK from={} reason={} attempt_ns={}",route.name(),reason,attempt.elapsed().as_nanos()),
        }
    }
    let (answer,nodes,hits,components)=generic(p,bits,hybrid,variant)?;
    Ok((answer,Route::Trail,nodes,hits,components))
}
// Fixed node prefix of the actual adaptive Trail fallback. A node-budget
// stop is diagnostic, never a complete model count or SAT verdict.
fn trail_prefix(p:&Problem,bits:u32,compact:bool,profile:bool)->Result<String,String>{
    const NODES:u64=512;
    let start=Instant::now();
    let (reduced,scope,_)=weighted_equivalence::reduce(p,bits);
    let mut trail=TrailEngine::new(&reduced,bits,50_000,NODES);
    trail.use_inline_weight_relabel=true;
    trail.use_parent_ids=compact;
    trail.use_compact_residual=compact;
    trail.use_prehashed_cache=compact;
    trail.use_fused_compact=compact;
    // Keep source-variable keys and branch policy unchanged, not weight
    // class relabeling, component VE or an alternate search order.
    trail.stage_profile=profile;
    let result=trail.count(scope,0);
    let status=match result{
        Ok(answer)=>format!("COMPLETE sat={} real={} imag={}",answer.sat,
            answer.count.re.to_string_radix(10,None),answer.count.im.to_string_radix(10,None)),
        Err(reason) if reason=="node limit"&&trail.stats.nodes==NODES=>"NODE_PREFIX_ONLY".into(),
        Err(reason)=>return Err(reason),
    };
    if profile{
        eprintln!("TRAIL_PREFIX_PROFILE_NS compact={} total={} key={} lookup={} unit={} frequency={} components={} materialize={} order={} pack={}",
            compact,start.elapsed().as_nanos(),trail.stage_key_ns,trail.stage_lookup_ns,
            trail.stage_unit_ns,trail.stage_frequency_ns,trail.stage_components_ns,
            trail.stage_key_materialize_ns,trail.stage_key_order_ns,trail.stage_key_pack_ns);
    }
    Ok(format!("TRAIL_PREFIX_V1 bits={bits} nodes={} cache_hits={} decompositions={} assignments={} {status}",
        trail.stats.nodes,trail.stats.cache_hits,trail.stats.decompositions,trail.assignments))
}
pub(crate) fn run(mut args:impl Iterator<Item=std::ffi::OsString>,hybrid:bool)->Result<(),String>{
    let first=args.next().and_then(|x|x.into_string().ok()).ok_or("missing precision")?;
    let mode=match first.as_str(){
        "trail-prefix-legacy"=>Some((false,false)),
        "trail-prefix-compact"=>Some((true,false)),
        "trail-prefix-legacy-profile"=>Some((false,true)),
        "trail-prefix-compact-profile"=>Some((true,true)),
        _=>None,
    };
    let variant=match first.as_str(){"trail-full-compact"=>1,"trail-full-separator"=>2,_=>0};
    if (mode.is_some()||variant>0)&&hybrid{return Err("diagnostic requires non-hybrid adaptive".into());}
    let digits:u32=if mode.is_some()||variant>0{args.next().and_then(|x|x.into_string().ok()).ok_or("missing precision")?}else{first}.parse().map_err(|_|"invalid precision")?;
    let bits=match digits{30=>100,60=>200,100=>333,130=>432,_=>return Err("unsupported precision".into())};
    let file=args.next().ok_or("missing source")?;
    if args.next().is_some(){return Err("unexpected arguments".into());}
    let t=Instant::now();
    let raw=std::fs::read_to_string(file).map_err(|e|e.to_string())?;
    let read_ns=t.elapsed().as_nanos();
    let t=Instant::now();let p=parse_emitted(&raw,bits)?;let parse_ns=t.elapsed().as_nanos();
    if let Some((compact,profile))=mode{
        println!("{}",trail_prefix(&p,bits,compact,profile)?);
        return Ok(());
    }
    let t=Instant::now();let (answer,route,nodes,cache_hits,decompositions)=solve(&p,&raw,bits,hybrid,variant)?;
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
    fn balanced_layer_separates_path_and_preserves_count(){
        let raw="p cnf 9 8\nc t wmc\n1 2 0\n2 3 0\n3 4 0\n4 5 0\n5 6 0\n6 7 0\n7 8 0\n8 9 0\n";
        for bits in [100,333]{
            let p=parse_emitted(raw,bits).unwrap();
            let (scores,size,left,right)=separator_layer(&p,&(1..=9).collect::<Vec<_>>()).unwrap();
            assert_eq!((size,left,right),(1,4,4));
            assert_eq!(scores[5],1);
            let (a,_,_,_)=generic(&p,bits,false,0).unwrap();
            for variant in [1,2]{
                let (b,_,_,_)=generic(&p,bits,false,variant).unwrap();
                assert_eq!(a.sat,b.sat);assert_eq!(a.count.re,b.count.re);assert_eq!(a.count.im,b.count.im);
            }
        }
    }
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
                    let (got,_,_,_,_)=solve(&p,raw,bits,hybrid,0).unwrap();
                    let (trail,_,_,_)=generic(&p,bits,hybrid,0).unwrap();
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
    fn width19_admission_requires_complete_resource_plan(){
        let vars:BTreeSet<_>=(1..=20).collect();
        let mut scopes=Vec::new();
        for &a in &vars{for &b in vars.range((a+1)..){for _ in 0..4{scopes.push(BTreeSet::from([a,b]));}}}
        let p=plan_scopes(&vars,&scopes,Instant::now(),1,true).unwrap();
        // Dense complete graph exceeds total lookup cap. Width alone
        // cannot authorize allocation or computation.
        assert!(!p.complete);
        assert_eq!(p.width,19);
        assert_eq!(choose(&Profile{width:19,complete:false,..Profile::default()}),Route::Trail);
        assert_eq!(choose(&Profile{width:19,complete:true,work:30_000_000,peak:800_000,..Profile::default()}),Route::Definitional);
    }
    #[test]
    fn incremental_profile_matches_full_scan(){
        let vars:BTreeSet<_>=(1..=12).collect();
        for seed in 1..=24u64{
            let mut x=seed;
            let mut scopes:Vec<BTreeSet<usize>>=vars.iter().map(|&v|BTreeSet::from([v])).collect();
            for a in 1..=12{
                for b in a+1..=12{
                    x=x.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if x>>61<3{scopes.push(BTreeSet::from([a,b]));}
                }
            }
            for (penalty,reverse) in [(0,false),(1,true),(4,true),(DEGREE_ONLY,false),(DEGREE_ONLY,true)]{
                let actual=plan_scopes(&vars,&scopes,Instant::now(),penalty,reverse).unwrap();
                assert!(actual.complete);
                let mut g:Graph=vars.iter().map(|&v|(v,BTreeSet::new())).collect();
                for s in &scopes{for (i,&a) in s.iter().enumerate(){for &b in s.iter().skip(i+1){edge(&mut g,a,b);}}}
                let mut fs=scopes.clone();let mut live=fs.iter().map(|s|1u64<<s.len()).sum::<u64>();
                let (mut work,mut peak,mut width)=(0,live,0);
                let mut order=Vec::new();
                while !g.is_empty(){
                    let v=*g.keys().min_by_key(|&&v|score(&g,v,penalty,reverse)).unwrap();
                    let nb=g.remove(&v).unwrap();let size=1u64<<nb.len();
                    let mut removed=0;let mut n=0;
                    fs.retain(|s|{if s.contains(&v){removed+=1u64<<s.len();n+=1;false}else{true}});
                    width=width.max(nb.len());work+=2*size*n;peak=peak.max(live+size);
                    live=live+size-removed;fs.push(nb.clone());order.push(v);
                    for &u in &nb{let s=g.get_mut(&u).unwrap();s.remove(&v);s.extend(nb.iter().filter(|&&x|x!=u).copied());}
                }
                assert_eq!((actual.order,actual.work,actual.peak,actual.width),(order,work,peak,width));
            }
        }
    }
    #[test]
    fn width20_admission_preserves_resource_caps(){
        let admitted=Profile{width:20,complete:true,work:50_100_000,peak:1_610_000,..Profile::default()};
        assert_eq!(choose(&admitted),Route::Definitional);
        assert_eq!(choose(&Profile{work:60_000_001,..admitted}),Route::Trail);
        let admitted=Profile{width:20,complete:true,work:50_100_000,peak:2_000_001,..Profile::default()};
        assert_eq!(choose(&admitted),Route::Trail);
        let p=Profile{width:21,gates:300,residual:0,..Profile::default()};
        assert_eq!(choose(&p),Route::Conditioned);
    }
    #[test]
    fn policy_uses_structure_and_caps_only(){
        assert_eq!(choose(&Profile::default()),Route::Trail);
        let p=Profile{complete:true,width:10,work:1000,peak:100,..Profile::default()};
        assert_eq!(choose(&p),Route::Definitional);
        let p=Profile{cliques:vec![8,8],..Profile::default()};
        assert_eq!(choose(&p),Route::Cardinality);
        let p=Profile{gates:100,width:19,..Profile::default()};
        assert_eq!(choose(&p),Route::Conditioned);
    }
}
