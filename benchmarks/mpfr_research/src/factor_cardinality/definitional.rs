//! Research-only fused bucket contraction with exact Boolean support.
use super::{extract_with_arity, weight, var, idx, edge, minfill, Graph, Trace, C, Outcome, Problem};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

struct Factor {
    scope: Vec<usize>,
    values: Vec<Option<C>>,
}
fn truth(l: i32, scope: &[usize], mask: usize) -> bool {
    ((mask >> scope.binary_search(&var(l)).expect("literal in scope")) & 1 != 0) == (l > 0)
}
fn project(scope: &[usize], union: &[usize]) -> Vec<usize> {
    scope.iter().map(|v|union.binary_search(v).expect("scope subset")).collect()
}
fn index(mask: usize, positions: &[usize]) -> usize {
    positions.iter().enumerate().fold(0, |a,(i,&j)|a|(((mask>>j)&1)<<i))
}
fn multiply(a: C, b: &C, bits: u32) -> C {
    if (a.re==0&&a.im==0)||(b.re==0&&b.im==0) {return C::zero(bits);}
    if b.re==1&&b.im==0 {return a;}
    if a.re==1&&a.im==0 {return b.clone();}
    a.mul(b,bits)
}
fn add(a: Option<C>, b: Option<C>, bits: u32) -> Option<C> {
    match (a,b) {
        (Some(a),Some(b))=>Some(a.add(&b,bits)),
        (a,None)|(None,a)=>a,
    }
}
fn insert(factors: &mut Vec<Factor>, graph: &mut Graph, factor: Factor) {
    for (i,&a) in factor.scope.iter().enumerate() {
        for &b in &factor.scope[i+1..] {edge(graph,a,b);}
    }
    factors.push(factor);
}
pub(super) fn count(p: &Problem, bits: u32, trace: &mut Trace) -> Result<Outcome,String> {
    let start=Instant::now();
    let (gates,rep,units,residual)=extract_with_arity(p,2048,4)?;
    let free:BTreeSet<_>=rep.iter().copied().filter(|&l|l!=0&&!units.contains_key(&var(l))).map(var).collect();
    trace.gates=gates.len();
    trace.free=free.len();
    let mut scalar=C::one(bits);
    let mut unary:BTreeMap<_,_>=free.iter().map(|&v|(v,[C::one(bits),C::one(bits)])).collect();
    for v in 1..=p.n {
        let r=rep[2*(v-1)];
        if r==0 {continue;}
        if let Some(&x)=units.get(&var(r)) {
            scalar=multiply(scalar,weight(p,if x==(r>0){v as i32}else{-(v as i32)}),bits);
        } else {
            let u=unary.get_mut(&var(r)).ok_or("missing unary owner")?;
            for (x,value) in u.iter_mut().enumerate() {
                *value=multiply(value.clone(),weight(p,if (x==1)==(r>0){v as i32}else{-(v as i32)}),bits);
            }
        }
    }
    let mut graph:Graph=free.iter().map(|&v|(v,BTreeSet::new())).collect();
    let mut factors=Vec::new();
    for (o,a) in gates {
        let b:Vec<_>=a.iter().map(|&l|rep[idx(l)]).collect();
        if b.contains(&0) {return Err("unowned gate argument".into());}
        let scope:Vec<_>=b.iter().copied().map(var).filter(|v|!units.contains_key(v)).collect::<BTreeSet<_>>().into_iter().collect();
        let vals=(0..(1usize<<scope.len())).map(|mask| {
            let yes=b.iter().all(|&l|units.get(&var(l)).map_or_else(||truth(l,&scope,mask),|&v|v==(l>0)));
            Some(weight(p,if yes{o}else{-o}).clone())
        }).collect();
        insert(&mut factors,&mut graph,Factor{scope,values:vals});
    }
    for c in residual {
        if c.len()>8 {return Err("residual clause arity cap".into());}
        let scope:Vec<_>=c.iter().copied().map(var).collect::<BTreeSet<_>>().into_iter().collect();
        if scope.iter().any(|v|!free.contains(v)) {return Err("residual ownership".into());}
        let values=(0..(1usize<<scope.len())).map(|mask| {
            if c.iter().any(|&l|truth(l,&scope,mask)) {Some(C::one(bits))} else {None}
        }).collect();
        insert(&mut factors,&mut graph,Factor{scope,values});
    }
    for (v,[a,b]) in unary {
        factors.push(Factor{scope:vec![v],values:vec![Some(a),Some(b)]});
    }
    while !graph.is_empty() {
        if start.elapsed().as_secs()>=105 {return Err("definitional deadline".into());}
        let v=*graph.keys().min_by_key(|&&v|minfill(&graph,v)).ok_or("empty graph")?;
        let nb=graph.remove(&v).ok_or("missing pivot")?;
        trace.width=trace.width.max(nb.len());
        if nb.len()>18 {return Err("definitional width cap".into());}
        let (mut bucket,rest):(Vec<_>,Vec<_>)=factors.into_iter().partition(|f|f.scope.contains(&v));
        factors=rest;
        bucket.sort_by_key(|f|f.scope.len());
        let scope:Vec<_>=nb.iter().copied().collect();
        let mut union=scope.clone();
        union.push(v);union.sort_unstable();
        let size=1usize<<scope.len();
        let pivot=union.binary_search(&v).map_err(|_|"missing pivot axis")?;
        let low=(1usize<<pivot)-1;
        let positions:Vec<_>=bucket.iter().map(|f|project(&f.scope,&union)).collect();
        let work=size.saturating_mul(2).saturating_mul(bucket.len());
        trace.work=trace.work.saturating_add(work);
        if trace.work>60_000_000 {return Err("definitional work cap".into());}
        let live=factors.iter().map(|f|f.values.len()).sum::<usize>()+bucket.iter().map(|f|f.values.len()).sum::<usize>()+size;
        if live>2_000_000 {return Err("definitional live cell cap".into());}
        trace.peak=trace.peak.max(live);
        let mut values=Vec::with_capacity(size);
        for m in 0..size {
            if m%4096==0&&start.elapsed().as_secs()>=105 {return Err("definitional deadline".into());}
            let base=(m&low)|((m&!low)<<1);
            let branch=|mask:usize| -> Option<C> {
                let mut product=C::one(bits);
                // None means impossible. Some(0) still carries SAT support.
                for (f,pos) in bucket.iter().zip(&positions) {
                    let x=f.values[index(mask,pos)].as_ref()?;
                    product=multiply(product,x,bits);
                }
                Some(product)
            };
            values.push(add(branch(base),branch(base|(1<<pivot)),bits));
        }
        factors.push(Factor{scope,values});
        for &u in &nb {
            let s=graph.get_mut(&u).ok_or("missing neighbor")?;
            s.remove(&v);s.extend(nb.iter().filter(|&&x|x!=u).copied());
        }
    }
    for f in factors {
        if !f.scope.is_empty()||f.values.len()!=1 {return Err("uneliminated factor".into());}
        match f.values.into_iter().next().flatten() {
            Some(x)=>scalar=multiply(scalar,&x,bits),
            None=>return Ok(Outcome::unsat(bits)),
        }
    }
    if !scalar.re.is_finite()||!scalar.im.is_finite() {return Err("nonfinite result".into());}
    Ok(Outcome{sat:true,count:scalar})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_gate_residual_and_zero_oracle() {
        for bits in [100,333] {
            for zero in [false,true] {
                let raw=format!("p cnf 6 6\nc t wmc\nc p weight 1 {} 0 0\nc p weight -1 {} 0 0\nc p weight 4 -2 3 0\nc p weight -4 2 -1 0\nc p weight 5 1 2 0\n4 1 -2 0\n-4 -1 0\n-4 2 0\n-5 6 0\n5 -6 0\n1 2 3 0\n",if zero{0}else{-1},if zero{0}else{2});
                let p=super::super::super::parse_emitted(&raw,bits).unwrap();
                let got=count(&p,bits,&mut Trace::default()).unwrap();
                let mut expected=Outcome::unsat(bits);
                for mask in 0..(1usize<<p.n) {
                    let yes=|l:i32|((mask>>(var(l)-1))&1!=0)==(l>0);
                    if p.clauses.iter().all(|c|c.iter().copied().any(yes)) {
                        expected.sat=true;
                        let mut x=C::one(bits);
                        for v in 1..=p.n {
                            let l=v as i32;
                            x=x.mul(weight(&p,if yes(l){l}else{-l}),bits);
                        }
                        expected.count=expected.count.add(&x,bits);
                    }
                }
                assert_eq!(got.sat,expected.sat);
                assert_eq!(got.count.re,expected.count.re);
                assert_eq!(got.count.im,expected.count.im);
            }
        }
    }
    #[test]
    fn multivariate_signed_definitions_oracle() {
        // Two independent signed AND outputs of arity 3 and 4,
        // plus a complemented equivalence and a unit that fixes a gate input.
        // A free variable retains both weights even if absent from all clauses.
        for bits in [100,333] {
            let raw="p cnf 9 12\nc t wmc\nc p weight 5 -2 3 0\nc p weight -5 1 -1 0\nc p weight 6 3 2 0\nc p weight -6 -1 2 0\nc p weight 7 0 0 0\nc p weight -7 2 -3 0\nc p weight 9 1 2 0\nc p weight -9 -3 1 0\n5 -1 2 -3 0\n-5 1 0\n-5 -2 0\n-5 3 0\n-6 1 -2 3 -4 0\n6 -1 0\n6 2 0\n6 -3 0\n6 4 0\n2 7 0\n-2 -7 0\n4 0\n";
            let p=super::super::super::parse_emitted(raw,bits).unwrap();
            let mut trace=Trace::default();
            let got=count(&p,bits,&mut trace).unwrap();
            assert_eq!(trace.gates,2);
            let mut expected=Outcome::unsat(bits);
            for mask in 0..(1usize<<p.n) {
                let yes=|l:i32|((mask>>(var(l)-1))&1!=0)==(l>0);
                if p.clauses.iter().all(|c|c.iter().copied().any(yes)) {
                    expected.sat=true;
                    let mut x=C::one(bits);
                    for v in 1..=p.n {
                        let l=v as i32;
                        x=x.mul(weight(&p,if yes(l){l}else{-l}),bits);
                    }
                    expected.count=expected.count.add(&x,bits);
                }
            }
            assert!(got.sat);
            assert_eq!(got.sat,expected.sat);
            assert_eq!(got.count.re,expected.count.re);
            assert_eq!(got.count.im,expected.count.im);
        }
    }
    #[test]
    fn incomplete_definition_is_not_extracted() {
        let raw="p cnf 4 3\nc t wmc\n4 -1 -2 -3 0\n-4 1 0\n-4 2 0\n";
        let p=super::super::super::parse_emitted(raw,100).unwrap();
        let mut trace=Trace::default();
        let got=count(&p,100,&mut trace).unwrap();
        assert_eq!(trace.gates,0);
        let expected=(0..16).filter(|&mask| {
            let yes=|l:i32|((mask>>(var(l)-1))&1!=0)==(l>0);
            p.clauses.iter().all(|c|c.iter().copied().any(yes))
        }).count();
        assert!(got.sat);
        assert_eq!(got.count.re,expected as u32);
        assert_eq!(got.count.im,0);
    }
    #[test]
    fn impossible_not_numeric_zero() {
        let mut raw=String::from("p cnf 3 8\nc t wmc\n");
        for mask in 0..8 {
            for v in 1..=3 {
                raw.push_str(&format!("{} ",if mask&(1<<(v-1))==0{v}else{-v}));
            }
            raw.push_str("0\n");
        }
        let p=super::super::super::parse_emitted(&raw,100).unwrap();
        let got=count(&p,100,&mut Trace::default()).unwrap();
        assert!(!got.sat);
    }
}
