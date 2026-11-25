//! Research-only exact-local elimination and bounded prejoin contraction.
use crate::{C,Outcome,Problem,parse_emitted};
use rug::{Assign,Float,Integer,Rational,ops::Pow};
use std::collections::{BTreeMap,BTreeSet};
use std::time::{Duration,Instant};

const WIDTH:usize=19;
const WORK:usize=40_000_000;
const LIVE:usize=2_000_000;
const LOCAL_WORK:usize=1_000_000;
const EXACT_BITS:u32=16_384;
type Scope=BTreeSet<usize>;
type R=(Rational,Rational);
type Cell<T>=Option<T>;
#[derive(Clone)]
struct Factor<T>{scope:Vec<usize>,values:Vec<Cell<T>>}
#[derive(Default,Debug,Clone,PartialEq,Eq)]
struct Cost{width:usize,work:usize,peak:usize,joins:usize}
fn deadline(d:Instant)->Result<(),String>{
    if Instant::now()>=d{Err("prejoin deadline".into())}else{Ok(())}
}
fn decimal(s:&str)->Result<Rational,String>{
    if s.len()>128{return Err("exact decimal length".into());}
    let mut p=s.split(['e','E']);
    let a=p.next().ok_or("decimal")?;
    let e=p.next().map_or(Ok(0),str::parse::<i32>).map_err(|_|"decimal exponent")?;
    if p.next().is_some()||!(-256..=256).contains(&e){return Err("decimal exponent".into());}
    let neg=a.starts_with('-');
    let a=a.strip_prefix(['-','+']).unwrap_or(a);
    let (w,f)=a.split_once('.').unwrap_or((a,""));
    if (w.is_empty()&&f.is_empty())||!w.bytes().chain(f.bytes()).all(|b|b.is_ascii_digit()){return Err("decimal digits".into());}
    let mut n=Integer::from_str_radix(&format!("{w}{f}"),10).map_err(|_|"decimal integer")?;
    if neg{n = -n;}
    let scale=e-f.len() as i32;
    let d=if scale<0{Integer::from(10).pow((-scale) as u32)}else{n*=Integer::from(10).pow(scale as u32);Integer::from(1)};
    Ok(Rational::from((n,d)))
}
fn one()->R{(Rational::from(1),Rational::from(0))}
fn checked(x:R)->Result<R,String>{
    if [&x.0,&x.1].iter().any(|r|r.numer().significant_bits().max(r.denom().significant_bits())>EXACT_BITS){
        return Err("exact local bit cap".into());
    }
    Ok(x)
}
fn rmul_full(a:&R,b:&R)->Result<R,String>{
    checked((Rational::from(&a.0*&b.0)-Rational::from(&a.1*&b.1),
             Rational::from(&a.0*&b.1)+Rational::from(&a.1*&b.0)))
}
// Exact axes and Boolean support depend on exact rational equality.
// Shortcuts only for algebraic identities, never rounded MPFR comparisons.
fn rmul(a:&R,b:&R)->Result<R,String>{
    if (a.0==0&&a.1==0)||(b.0==0&&b.1==0){
        return Ok((Rational::from(0),Rational::from(0)));
    }
    if a.0==1&&a.1==0{return checked(b.clone());}
    if b.0==1&&b.1==0{return checked(a.clone());}
    if a.0== -1&&a.1==0{return checked((-b.0.clone(),-b.1.clone()));}
    if b.0== -1&&b.1==0{return checked((-a.0.clone(),-a.1.clone()));}
    if a.0==0&&a.1==1{return checked((-b.1.clone(),b.0.clone()));}
    if b.0==0&&b.1==1{return checked((-a.1.clone(),a.0.clone()));}
    if a.0==0&&a.1== -1{return checked((b.1.clone(),-b.0.clone()));}
    if b.0==0&&b.1== -1{return checked((a.1.clone(),-a.0.clone()));}
    // Non-unit real/imaginary axes require only two rational products.
    // Keep exact support and the same final bit-cap check as the full formula.
    if a.1==0{return checked((Rational::from(&a.0*&b.0),Rational::from(&a.0*&b.1)));}
    if b.1==0{return checked((Rational::from(&a.0*&b.0),Rational::from(&a.1*&b.0)));}
    if a.0==0{return checked((-Rational::from(&a.1*&b.1),Rational::from(&a.1*&b.0)));}
    if b.0==0{return checked((-Rational::from(&a.1*&b.1),Rational::from(&a.0*&b.1)));}
    rmul_full(a,b)
}
fn radd(a:Cell<R>,b:Cell<R>)->Result<Cell<R>,String>{
    match(a,b){(Some(a),Some(b))=>Ok(Some(checked((a.0+b.0,a.1+b.1))?)),(a,None)|(None,a)=>Ok(a)}
}
fn reduce_axes<T:PartialEq>(mut f:Factor<T>)->Factor<T>{
    for j in (0..f.scope.len()).rev(){
        let bit=1<<j;
        if (0..f.values.len()).filter(|m|m&bit==0).all(|m|f.values[m]==f.values[m|bit]){
            f.scope.remove(j);
            f.values=f.values.into_iter().enumerate().filter_map(|(m,x)|(m&bit==0).then_some(x)).collect();
        }
    }
    f
}
fn rscalar(s:Cell<R>,f:Cell<R>)->Result<Cell<R>,String>{
    match(s,f){(Some(a),Some(b))=>Ok(Some(rmul(&a,&b)?)),_=>Ok(None)}
}
fn mask(scope:&[usize],out:&[usize],m:usize,pivot:usize,positive:bool)->usize{
    scope.iter().enumerate().fold(0,|a,(j,u)|{
        let set=if *u==pivot{positive}else{m&(1<<out.binary_search(u).expect("scope subset"))!=0};
        a|(usize::from(set)<<j)
    })
}
type Local=(Vec<Factor<R>>,Scope,Cell<R>,Cost);
fn local(p:&Problem,raw:&str,d:Instant)->Result<Local,String>{
    if p.clauses.len()>16_000{return Err("source clause cap".into());}
    let mut weights=vec![[one(),one()];p.n];
    for line in raw.lines(){
        let a:Vec<_>=line.split_whitespace().collect();
        if a.len()==7&&a[..3]==["c","p","weight"]{
            let l:i32=a[3].parse().map_err(|_|"weight literal")?;
            weights[l.unsigned_abs() as usize-1][usize::from(l>0)]=checked((decimal(a[4])?,decimal(a[5])?))?;
        }
    }
    let scopes:Vec<Vec<usize>>=p.clauses.iter().map(|c|c.iter().map(|l|l.unsigned_abs() as usize).collect::<Scope>().into_iter().collect()).collect();
    if scopes.iter().any(|s|s.len()>9){return Err("clause width cap".into());}
    let initial=2*p.n+scopes.iter().map(|s|1usize<<s.len()).sum::<usize>();
    if initial>LIVE{return Err("initial cells cap".into());}
    let mut fs=Vec::new();
    let mut scalar=Some(one());
    let insert=|fs:&mut Vec<Factor<R>>,scalar:&mut Cell<R>,f:Factor<R>|->Result<(),String>{
        let f=reduce_axes(f);
        if f.scope.is_empty(){*scalar=rscalar(scalar.take(),f.values.into_iter().next().unwrap())?;}
        else{fs.push(f);}
        Ok(())
    };
    for (c,scope) in p.clauses.iter().zip(scopes){
        deadline(d)?;
        let values=(0..1usize<<scope.len()).map(|m|{
            c.iter().any(|l|(m&(1<<scope.binary_search(&(l.unsigned_abs() as usize)).unwrap())!=0)==(*l>0)).then(one)
        }).collect();
        insert(&mut fs,&mut scalar,Factor{scope,values})?;
    }
    for (i,[a,b]) in weights.into_iter().enumerate(){
        insert(&mut fs,&mut scalar,Factor{scope:vec![i+1],values:vec![Some(a),Some(b)]})?;
    }
    let mut nodes:Scope=(1..=p.n).collect();
    let mut cost=Cost{peak:initial,..Cost::default()};
    // Rebuild incidence for a bounded local frontier. Exact equal axes
    // can disappear without eliminating their variable (free factor of 2).
    while !nodes.is_empty(){
        deadline(d)?;
        let mut incidence:BTreeMap<usize,(Scope,usize)>=nodes.iter().map(|&v|(v,(Scope::new(),0))).collect();
        for f in &fs{for &v in &f.scope{
            let (nb,n)=incidence.get_mut(&v).ok_or("local owner")?;
            nb.extend(f.scope.iter().copied().filter(|&u|u!=v));*n+=1;
        }}
        let (&v,(nb,_))=incidence.iter().min_by_key(|(v,(nb,n))|(nb.len(),*n,usize::MAX-**v)).ok_or("local pivot")?;
        if nb.len()>4{break;}
        let out:Vec<_>=nb.iter().copied().collect();
        let live=fs.iter().map(|f|f.values.len()).sum::<usize>();
        let (bucket,rest):(Vec<_>,Vec<_>)=fs.into_iter().partition(|f|f.scope.contains(&v));
        fs=rest;
        let size=1usize<<out.len();
        cost.work+=2*size*bucket.len();
        cost.peak=cost.peak.max(live+2*size);
        cost.width=cost.width.max(out.len());
        if cost.work>LOCAL_WORK||cost.peak>LIVE{return Err("local budget".into());}
        let mut vals=Vec::with_capacity(size);
        for m in 0..size{
            let mut pair=[None,None];
            for (j,positive) in [false,true].into_iter().enumerate(){
                let mut x=Some(one());
                for f in &bucket{
                    let b=&f.values[mask(&f.scope,&out,m,v,positive)];
                    x=match(x,b){(Some(a),Some(b))=>Some(rmul(&a,b)?),_=>None};
                    if x.is_none(){break;}
                }
                pair[j]=x;
            }
            let [a,b]=pair;vals.push(radd(a,b)?);
        }
        insert(&mut fs,&mut scalar,Factor{scope:out,values:vals})?;
        nodes.remove(&v);
    }
    if nodes.len()>64{return Err("residual variable cap".into());}
    Ok((fs,nodes,scalar,cost))
}
fn restrict(fs:&[Factor<R>],scalar:&Cell<R>,cuts:&[usize],m:usize)->Result<(Vec<Factor<R>>,Cell<R>),String>{
    let mut out=Vec::new();let mut scalar=scalar.clone();
    for f in fs{
        let mut f=f.clone();
        for (i,v) in cuts.iter().enumerate(){
            if let Ok(j)=f.scope.binary_search(v){
                let bit=1<<j;let value=m&(1<<i)!=0;
                f.scope.remove(j);
                f.values=f.values.into_iter().enumerate().filter_map(|(a,x)|((a&bit!=0)==value).then_some(x)).collect();
            }
        }
        let f=reduce_axes(f);
        if f.scope.is_empty(){scalar=rscalar(scalar,f.values.into_iter().next().unwrap())?;}else{out.push(f);}
    }
    Ok((out,scalar))
}
#[derive(Clone)]
struct Op{ids:Vec<usize>,dst:usize,out:Vec<usize>,pivot:Option<usize>}
#[derive(Clone)]
struct Plan{ops:Vec<Op>,cost:Cost,extra:usize}
// Dense indices are local to a plan; original variable IDs and ordering remain
// unchanged. A mask represents the same undirected graph as the ordered sets.
fn mask_indices(mut m:u64)->impl Iterator<Item=usize>{
    std::iter::from_fn(move ||{
        if m==0{return None;}
        let i=m.trailing_zeros() as usize;
        m&=m-1;
        Some(i)
    })
}
fn fill_count(graph:&[u64],nb:u64)->usize{
    let mut remaining=nb;
    let mut fill=0;
    while remaining!=0{
        let i=remaining.trailing_zeros() as usize;
        remaining&=remaining-1;
        fill+=(remaining&!graph[i]).count_ones() as usize;
    }
    fill
}
fn plan(fs:&[Factor<R>],nodes:&Scope,policy:(Option<usize>,bool),retained:usize,d:Instant)->Result<Option<Plan>,String>{
    let mut scopes:BTreeMap<usize,Scope>=fs.iter().enumerate().map(|(i,f)|(i,f.scope.iter().copied().collect())).collect();
    let vars:Vec<_>=nodes.iter().copied().collect();
    if vars.len()>64{return Err("plan mask cap".into());}
    let mut graph=vec![0u64;vars.len()];
    let mut active=if vars.len()==64{u64::MAX}else{(1u64<<vars.len())-1};
    for s in scopes.values(){
        let mut m=0u64;
        for v in s{m|=1u64<<vars.binary_search(v).map_err(|_|"plan owner")?;}
        for i in mask_indices(m){graph[i]|=m&!(1u64<<i);}
    }
    let mut live=fs.iter().map(|f|f.values.len()).sum::<usize>();
    let extra=retained+live;
    let mut cost=Cost{peak:live+extra,..Cost::default()};
    let mut ops=Vec::new();let mut next=scopes.len();
    while active!=0{
        deadline(d)?;
        let i=mask_indices(active).min_by_key(|&i|{
            let nb=graph[i];
            let degree=nb.count_ones() as usize;
            let fill=if policy.0.is_some(){fill_count(&graph,nb)}else{0};
            let v=vars[i];
            (policy.0.map_or(degree,|p|fill+p*degree),degree,if policy.1{usize::MAX-v}else{v})
        }).ok_or("plan pivot")?;
        let v=vars[i];
        let neighbors=graph[i];
        let nb:Scope=mask_indices(neighbors).map(|j|vars[j]).collect();
        if nb.len()>WIDTH{return Ok(None);}
        cost.width=cost.width.max(nb.len());
        active&=!(1u64<<i);
        graph[i]=0;
        let mut bucket:Vec<_>=scopes.iter().filter(|(_,s)|s.contains(&v)).map(|(&i,s)|(i,s.clone())).collect();
        for (i,_) in &bucket{scopes.remove(i);}
        while bucket.len()>2{
            let mut best=None;
            for i in 0..bucket.len(){for j in i+1..bucket.len(){
                let n=bucket[i].1.union(&bucket[j].1).count();
                if n<nb.len()&&best.is_none_or(|k|(n,i,j)<k){best=Some((n,i,j));}
            }}
            let Some((n,i,j))=best else{break;};
            let size=1usize<<n;
            if live+size+extra>LIVE{break;}
            let (ib,b)=bucket.remove(j);let (ia,a)=bucket.remove(i);
            let u:Scope=a.union(&b).copied().collect();
            cost.work+=2*size;cost.peak=cost.peak.max(live+size+extra);cost.joins+=1;
            live=live+size-(1<<a.len())-(1<<b.len());
            ops.push(Op{ids:vec![ia,ib],dst:next,out:u.iter().copied().collect(),pivot:None});
            bucket.push((next,u));next+=1;
        }
        let size=1usize<<nb.len();
        cost.work+=2*size*bucket.len();cost.peak=cost.peak.max(live+size+extra);
        if cost.work>WORK||cost.peak>LIVE{return Ok(None);}
        live=live+size-bucket.iter().map(|(_,s)|1usize<<s.len()).sum::<usize>();
        ops.push(Op{ids:bucket.iter().map(|(i,_)|*i).collect(),dst:next,out:nb.iter().copied().collect(),pivot:Some(v)});
        scopes.insert(next,nb.clone());next+=1;
        for j in mask_indices(neighbors){
            graph[j]=(graph[j]&!(1u64<<i))|(neighbors&!(1u64<<j));
        }
    }
    Ok(Some(Plan{ops,cost,extra}))
}
type Selected=(Vec<usize>,Vec<Plan>,Cost);
fn choose(fs:&[Factor<R>],nodes:&Scope,scalar:&Cell<R>,local:&Cost,d:Instant)->Result<Selected,String>{
    let retained=fs.iter().map(|f|f.values.len()).sum::<usize>();
    let mut high:Vec<_>=nodes.iter().copied().collect();
    high.sort_by_key(|v|{
        let nb:Scope=fs.iter().filter(|f|f.scope.contains(v)).flat_map(|f|f.scope.iter().copied()).filter(|u|u!=v).collect();
        (usize::MAX-nb.len(),*v)
    });
    high.truncate(12);
    for n in 0..=2{
        let candidates:Vec<Vec<usize>>=match n{
            0=>vec![vec![]],
            1=>nodes.iter().map(|&v|vec![v]).collect(),
            _=>high.iter().enumerate().flat_map(|(i,&v)|high.iter().skip(i+1).map(move|&u|vec![v,u])).collect(),
        };
        let mut best:Option<Selected>=None;
        for cuts in candidates{
            deadline(d)?;
            let ns:Scope=nodes.iter().copied().filter(|v|!cuts.contains(v)).collect();
            let mut plans=Vec::new();let mut cost=local.clone();
            for m in 0..1usize<<cuts.len(){
                let (f,_)=restrict(fs,scalar,&cuts,m)?;
                let mut leaf:Option<Plan>=None;
                for p in [Some(0),Some(4),None]{for reverse in [true,false]{
                    if let Some(x)=plan(&f,&ns,(p,reverse),retained,d)?{
                        if leaf.as_ref().is_none_or(|b|(x.cost.work,x.cost.peak,x.cost.width)<(b.cost.work,b.cost.peak,b.cost.width)){leaf=Some(x);}
                    }
                }}
                let Some(x)=leaf else{break;};
                cost.work+=x.cost.work+retained;cost.peak=cost.peak.max(x.cost.peak);
                cost.width=cost.width.max(x.cost.width);cost.joins+=x.cost.joins;
                plans.push(x);
            }
            if plans.len()==1<<cuts.len()&&cost.work<=WORK&&cost.peak<=LIVE&&best.as_ref().is_none_or(|b|
                (cost.work,cost.peak,cost.width,&cuts)<(b.2.work,b.2.peak,b.2.width,&b.0)){best=Some((cuts,plans,cost));}
        }
        if let Some(b)=best{return Ok(b);}
    }
    Err("no bounded prejoin plan".into())
}
fn mpfr(x:Cell<R>,bits:u32)->Cell<C>{
    x.map(|(a,b)|C{re:Float::with_val(bits,a),im:Float::with_val(bits,b)})
}
struct Scratch([Float;4]);
impl Scratch{
    fn new(bits:u32)->Self{Self(std::array::from_fn(|_|Float::new(bits)))}
    fn multiply(&mut self,a:&mut C,b:&C){
        if (a.re==0&&a.im==0)||(b.re==0&&b.im==0){a.re.assign(0);a.im.assign(0);return;}
        if b.re==1&&b.im==0{return;}
        if a.re==1&&a.im==0{a.re.assign(&b.re);a.im.assign(&b.im);return;}
        let [ac,bd,ad,bc]=&mut self.0;
        ac.assign(&a.re*&b.re);bd.assign(&a.im*&b.im);ad.assign(&a.re*&b.im);bc.assign(&a.im*&b.re);
        a.re.assign(&*ac-&*bd);a.im.assign(&*ad+&*bc);
    }
}
fn cadd(a:Cell<C>,b:Cell<C>)->Cell<C>{
    match(a,b){(Some(mut a),Some(b))=>{a.re+=&b.re;a.im+=&b.im;Some(a)},(a,None)|(None,a)=>a}
}
fn execute(fs:Vec<Factor<R>>,scalar:Cell<R>,p:&Plan,bits:u32,d:Instant)->Result<Cell<C>,String>{
    let mut tables:BTreeMap<usize,Factor<C>>=fs.into_iter().enumerate().map(|(i,f)|(i,Factor{scope:f.scope,values:f.values.into_iter().map(|x|mpfr(x,bits)).collect()})).collect();
    let mut live=tables.values().map(|f|f.values.len()).sum::<usize>();
    let mut cost=Cost{peak:live+p.extra,..Cost::default()};
    let mut scratch=Scratch::new(bits);
    let (mut left,mut right)=(C::one(bits),C::one(bits));
    for op in &p.ops{
        deadline(d)?;
        let bucket:Vec<_>=op.ids.iter().map(|i|tables.remove(i).ok_or("execution owner")).collect::<Result<_,_>>()?;
        let size=1usize<<op.out.len();
        cost.peak=cost.peak.max(live+size+p.extra);
        if op.pivot.is_some(){cost.width=cost.width.max(op.out.len());cost.work+=2*size*bucket.len();}
        else{cost.joins+=1;cost.work+=2*size;}
        if cost.work>WORK||cost.peak>LIVE||cost.width>WIDTH{return Err("execution cap".into());}
        let mut indices=vec![0;bucket.len()];
        let mut pivots=vec![0;bucket.len()];
        let mut flips=Vec::new();
        for (i,f) in bucket.iter().enumerate(){
            let mut b=vec![0usize;op.out.len()];
            for (j,&u) in f.scope.iter().enumerate(){
                if Some(u)==op.pivot{pivots[i]=1<<j;}else{b[op.out.binary_search(&u).map_err(|_|"execution scope")?]=1<<j;}
            }
            for j in 1..b.len(){b[j]^=b[j-1];}flips.push(b);
        }
        let mut values=Vec::with_capacity(size);
        for m in 0..size{
            if m%4096==0{deadline(d)?;}
            let mut branch=|positive:bool,x:&mut C|{
                // Reject unsupported assignments before any MPFR arithmetic.
                // Some(0) remains Boolean support; never prune on numeric zero.
                let cell=|i:usize|indices[i]|if positive{pivots[i]}else{0};
                if bucket.iter().enumerate().any(|(i,f)|f.values[cell(i)].is_none()){
                    return false;
                }
                x.re.assign(1);x.im.assign(0);
                for (i,f) in bucket.iter().enumerate(){
                    let y=f.values[cell(i)].as_ref().expect("validated support");
                    scratch.multiply(x,y);
                }true
            };
            let a=branch(false,&mut left);
            let b=op.pivot.is_some()&&branch(true,&mut right);
            values.push(match(a,b){(true,true)=>{left.re+=&right.re;left.im+=&right.im;Some(left.clone())},(true,false)=>Some(left.clone()),(false,true)=>Some(right.clone()),_=>None});
            if m+1<size{let bit=(m+1).trailing_zeros() as usize;for (i,x) in indices.iter_mut().enumerate(){*x^=flips[i][bit];}}
        }
        live=live+size-bucket.iter().map(|f|f.values.len()).sum::<usize>();
        tables.insert(op.dst,Factor{scope:op.out.clone(),values});
    }
    if cost!=p.cost{return Err("plan replay drift".into());}
    let mut out=mpfr(scalar,bits);
    for f in tables.into_values(){
        if !f.scope.is_empty()||f.values.len()!=1{return Err("uneliminated".into());}
        out=match(out,f.values.into_iter().next().unwrap()){(Some(mut a),Some(b))=>{scratch.multiply(&mut a,&b);Some(a)},_=>None};
    }
    Ok(out)
}
fn solve(p:&Problem,raw:&str,bits:u32)->Result<Outcome,String>{
    let start=Instant::now();let d=start+Duration::from_secs(45);let pd=start+Duration::from_secs(30);
    let (fs,nodes,scalar,local)=local(p,raw,pd)?;
    let local_ns=start.elapsed().as_nanos();
    let phase=Instant::now();
    let (cuts,plans,cost)=choose(&fs,&nodes,&scalar,&local,pd)?;
    let choose_ns=phase.elapsed().as_nanos();
    let plan_ns=start.elapsed().as_nanos();
    let mut out=None;
    let mut restrict_ns=0;
    let mut execute_ns=0;
    for (m,plan) in plans.iter().enumerate(){
        let phase=Instant::now();
        let (f,s)=restrict(&fs,&scalar,&cuts,m)?;
        restrict_ns+=phase.elapsed().as_nanos();
        let phase=Instant::now();
        out=cadd(out,execute(f,s,plan,bits,d)?);
        execute_ns+=phase.elapsed().as_nanos();
    }
    eprintln!("PREJOIN_PHASE_NS local={local_ns} choose={choose_ns} restrict={restrict_ns} execute={execute_ns}");
    eprintln!("PREJOIN_COMPLETE residual={} cuts={} leaves={} width={} work={} peak={} joins={} plan_ns={}",nodes.len(),cuts.len(),plans.len(),cost.width,cost.work,cost.peak,cost.joins,plan_ns);
    match out{
        None=>Ok(Outcome::unsat(bits)),
        Some(count) if count.re.is_finite()&&count.im.is_finite()=>Ok(Outcome{sat:true,count}),
        _=>Err("nonfinite prejoin".into()),
    }
}
pub(crate) fn run(mut args:impl Iterator<Item=std::ffi::OsString>)->Result<(),String>{
    let digits:u32=args.next().and_then(|x|x.into_string().ok()).ok_or("missing precision")?.parse().map_err(|_|"invalid precision")?;
    let bits=match digits{30=>100,60=>200,100=>333,130=>432,_=>return Err("unsupported precision".into())};
    let file=args.next().ok_or("missing source")?;
    if args.next().is_some(){return Err("unexpected arguments".into());}
    let t=Instant::now();let raw=std::fs::read_to_string(file).map_err(|e|e.to_string())?;let read_ns=t.elapsed().as_nanos();
    let t=Instant::now();let p=parse_emitted(&raw,bits)?;let parse_ns=t.elapsed().as_nanos();
    let t=Instant::now();let answer=solve(&p,&raw,bits)?;let solve_ns=t.elapsed().as_nanos();let t=Instant::now();
    println!("s {}",if answer.sat{"SATISFIABLE"}else{"UNSATISFIABLE"});
    println!("c r mpfr {} {}",answer.count.re.to_string_radix(10,Some(digits as usize+4)),answer.count.im.to_string_radix(10,Some(digits as usize+4)));
    let output_ns=t.elapsed().as_nanos();
    eprintln!("RESEARCH_MPFR_PHASE_NS read={read_ns} parse={parse_ns} solve={solve_ns} output={output_ns} nodes=0 cache_hits=0 decompositions=0 digits={digits} bits={bits}");
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn mask_fill_matches_set_reference(){
        // Every undirected graph on five vertices, every neighborhood.
        for edges in 0..1024u64{
            let mut graph=[0u64;5];let mut bit=0;
            for i in 0..5{for j in i+1..5{
                if edges&(1<<bit)!=0{graph[i]|=1<<j;graph[j]|=1<<i;}
                bit+=1;
            }}
            for nb in 0..32u64{
                let mut expected=0;
                for (i,&neighbors) in graph.iter().enumerate(){for j in i+1..5{
                    if nb&(1<<i)!=0&&nb&(1<<j)!=0&&neighbors&(1<<j)==0{expected+=1;}
                }}
                assert_eq!(fill_count(&graph,nb),expected);
            }
        }
        assert_eq!(mask_indices(u64::MAX).collect::<Vec<_>>(),(0..64).collect::<Vec<_>>());
        assert_eq!(fill_count(&[0u64;64],u64::MAX),2016);
    }
    #[test]
    fn exact_rational_identity_shortcuts_match_full_formula(){
        let values=[(0,0),(1,0),(-1,0),(0,1),(0,-1),(2,3),(-3,2)];
        let mut x:Vec<R>=values.into_iter().map(|(r,i)|(Rational::from(r),Rational::from(i))).collect();
        x.push((decimal("-1.0000000000000000").unwrap(),decimal("1.2246467991473532e-16").unwrap()));
        x.push((decimal("0.12345678901234567890123456789").unwrap(),decimal("-1e-80").unwrap()));
        x.push((decimal("1e80").unwrap(),decimal("1e-80").unwrap()));
        for v in ["2","-3","0.125","-0.1234567890123456789","1e80","1e-80"]{
            let q=decimal(v).unwrap();
            x.push((q.clone(),Rational::from(0)));
            x.push((Rational::from(0),q));
        }
        for a in &x{for b in &x{
            assert_eq!(rmul(a,b).unwrap(),rmul_full(a,b).unwrap());
        }}
        // A supported exact zero is still Some, not Boolean impossibility.
        let z=Some(rmul(&x[0],&x[7]).unwrap());
        assert!(z.is_some());
        assert_eq!(z,Some((Rational::from(0),Rational::from(0))));
        assert_eq!(rscalar(None,Some(one())).unwrap(),None);
    }
    #[test]
    fn exact_local_support_and_free_axes(){
        for bits in [100,333]{
            for (raw,sat,re,im) in [
                ("p cnf 3 0\nc t wmc\n",true,8,0),
                ("p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",true,0,0),
                ("p cnf 1 2\nc t wmc\n1 0\n-1 0\n",false,0,0),
                ("p cnf 2 1\nc t wmc\nc p weight 1 -2 3 0\n1 2 0\n",true,-3,6),
            ]{let p=parse_emitted(raw,bits).unwrap();let a=solve(&p,raw,bits).unwrap();assert_eq!(a.sat,sat);assert_eq!(a.count.re,re);assert_eq!(a.count.im,im);}
        }
    }
    #[test]
    fn bounded_prejoin_replay_matches_exact_enumeration(){
        // Dense scope forces the interval executor rather than local elimination.
        let raw="p cnf 6 2\nc t wmc\nc p weight 1 -2 3 0\nc p weight -3 1 -1 0\n1 2 3 4 5 6 0\n-1 -2 3 -4 5 -6 0\n";
        for bits in [100,333]{
            let p=parse_emitted(raw,bits).unwrap();let a=solve(&p,raw,bits).unwrap();
            let mut expected=Outcome::unsat(bits);
            for m in 0..64{
                let yes=|l:i32|(m&(1<<(l.unsigned_abs()-1))!=0)==(l>0);
                if p.clauses.iter().all(|c|c.iter().copied().any(yes)){
                    expected.sat=true;let mut x=C::one(bits);
                    for (i,w) in p.weights.iter().enumerate(){x=x.mul(if m&(1<<i)!=0{&w.positive}else{&w.negative},bits);}
                    expected.count=expected.count.add(&x,bits);
                }
            }
            assert_eq!(a.sat,expected.sat);assert_eq!(a.count.re,expected.count.re);assert_eq!(a.count.im,expected.count.im);
        }
    }
}
