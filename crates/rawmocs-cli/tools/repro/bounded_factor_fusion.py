#!/usr/bin/env python3
"""Exact factor fusion diagnostic. No production cap or numeric policy changes."""
import argparse
from collections import Counter
from fractions import Fraction as Q
from hashlib import sha256
from itertools import combinations, product
import json
from pathlib import Path
from time import monotonic
import original_exact_prejoin as o
from original_fixed_interval import BITS, S, ZERO, add as iadd

FUSION_WORK = 1_000_000

def fuse(fs, scalar, mode, deadline):
    """Preserve exact pointwise factor product and Boolean support.
    Fuse identical scopes; in subset mode absorb a smaller scope into a
    containing scope. Never grow a scope. Reject failed exact operations.
    """
    items=list(fs)
    work=merges=splits=axis_drops=rejects=0
    peak=sum(len(t) for _,t in items)
    blocked=set()
    while monotonic()<deadline:
        candidates=[]
        for i,(a,x) in enumerate(items):
            for j in range(i+1,len(items)):
                b,y=items[j]
                if (i,j) in blocked: continue
                if a==b or (mode=="subset" and (set(a)<=set(b) or set(b)<=set(a))):
                    candidates.append((max(len(a),len(b)),i,j))
        if not candidates:break
        _,i,j=min(candidates)
        a,x=items[i];b,y=items[j]
        scope=tuple(sorted(set(a)|set(b)))
        size=1<<len(scope)
        charge=2*size
        if work+charge>FUSION_WORK or sum(len(t) for _,t in items)+2*size>o.MAX_LIVE:break
        work+=charge
        peak=max(peak,sum(len(t) for _,t in items)+2*size)
        def project(m,s):
            return sum(((m>>scope.index(v))&1)<<k for k,v in enumerate(s))
        try:
            vals=[o.mul(x[project(m,a)],y[project(m,b)]) for m in range(size)]
            scope,vals,dropped=o.reduce_axes(scope,vals)
            proposed=scalar if scope else o.mul(scalar,vals[0])
        except ValueError as exc:
            if str(exc)!="LOCAL_EXACT_BITS":raise
            rejects+=1;blocked.add((i,j));continue
        items.pop(j);items.pop(i)
        if scope:items.append((scope,vals))
        scalar=proposed;axis_drops+=dropped;merges+=1
        blocked.clear()
    # Exact separability checks after fusion. The split helper verifies every
    # original cell, including None vs supported numeric zero.
    if mode=="subset":
        pending=list(reversed(items));items=[]
        while pending:
            scope,vals=pending.pop()
            parts=None
            if 2<=len(scope)<=9:
                for n in range(1,len(scope)):
                    for rest in combinations(scope[1:],n-1):
                        charge=16*len(vals)
                        if monotonic()>=deadline or work+charge>FUSION_WORK:break
                        work+=charge
                        parts=o.split_rank_one(scope,vals,(scope[0],)+rest)
                        if parts is not None:break
                    if parts is not None or monotonic()>=deadline or work+16*len(vals)>FUSION_WORK:break
            if parts is None:items.append((scope,vals))
            else:
                splits+=1
                pending.extend(reversed(parts))
                peak=max(peak,sum(len(t) for _,t in items+pending)+2*len(vals))
    return items,scalar,dict(work=work,peak=peak,merges=merges,splits=splits,
        axis_drops=axis_drops,bit_cap_rejections=rejects,
        factors_before=len(fs),factors_after=len(items),
        cells_before=sum(len(t) for _,t in fs),cells_after=sum(len(t) for _,t in items),
        deadline_reached=monotonic()>=deadline)

def profile(fs,ns):
    adj,parts=o.residual_components(fs,ns)
    return dict(edges=sum(map(len,adj.values()))//2,components=sorted(map(len,parts),reverse=True),
                scopes=dict(sorted(Counter(map(lambda f:len(f[0]),fs)).items())))

def replay(fs,ns,scalar,local,prior,mode,deadline):
    # All original candidates are source-locked. Each branch can select its
    # own contraction order; no shared assumptions about factor graph.
    rows=[]
    for old in prior["replay"]:
        if monotonic()>=deadline:break
        cuts=old["cutset"];seq=old["order"]
        row=dict(cutset=cuts,branches=0,work=local["work"],peak=local["peak"],width=0)
        rows.append(row);plans=[]
        for values in product((False,True),repeat=len(cuts)):
            if monotonic()>=deadline:break
            try:
                branch,sc=o.restrict(fs,scalar,cuts,values)
                options=[]
                for pol in (None,*o.POLICIES):
                    p=o.plan(branch,ns-set(cuts),pol or (None,False),local["retained_cells"],deadline,
                             fixed_order=seq if pol is None else None)
                    if p is not None: options.append(p)
                if not options:break
            except ValueError as exc:
                if str(exc) not in ("PLAN_DEADLINE","LOCAL_EXACT_BITS"):raise
                row["reason"]=str(exc);break
            p=min(options,key=lambda x:(x["work"],x["peak"],x["width"]))
            plans.append((values,p))
            row["branches"]+=1
            row["work"]+=p["work"]+local["retained_cells"]
            row["peak"]=max(row["peak"],p["peak"])
            row["width"]=max(row["width"],p["width"])
        row["status"]="FEASIBLE" if len(plans)==1<<len(cuts) and row["work"]<=o.MAX_WORK and row["peak"]<=o.MAX_LIVE else "NOT_ADMITTED"
        if row["status"]=="FEASIBLE":return rows,(cuts,plans,row)
    return rows,None

def search(raw,expected,prior):
    start=monotonic()
    width=prior["local_width"]
    fs,ns,scalar,local=o.local_reduce(raw,expected,start+30,diagnostic=True,local_width=width,incremental=True)
    assert local==prior["local"]
    report=dict(status="DIAGNOSTIC_ONLY",source_sha256=expected,local_width=width,before=profile(fs,ns),modes=[])
    deadline=start+100
    for mode in ("same","subset"):
        if monotonic()>=deadline:break
        fused,sc,diag=fuse(fs,scalar,mode,min(deadline,monotonic()+15))
        row=dict(mode=mode,fusion=diag,after=profile(fused,ns))
        report["modes"].append(row)
        # Source and transformed exact factors both resident. Charge all
        # attempted transformation work, even rejected exact operations.
        retained=local["retained_cells"]+diag["cells_after"]
        charge=dict(local,work=local["work"]+diag["work"],retained_cells=retained,
                    peak=max(local["peak"],local["retained_cells"]+diag["peak"],retained))
        rows,selected=replay(fused,ns,sc,charge,prior,mode,min(deadline,monotonic()+35))
        row["replay"]=rows
        if selected is None:continue
        cuts,plans,cert=selected
        result=None
        try:
            execution_deadline=monotonic()+90
            for values,p in plans:
                branch,bc=o.restrict(fused,sc,cuts,values)
                result=iadd(result,o.execute(branch,bc,p,execution_deadline))
        except ValueError as exc:
            if str(exc) not in ("JOIN_DEADLINE","SUM_DEADLINE","EXECUTE_DEADLINE"):raise
            row["execution_stop"]=str(exc);break
        sat=result is not None
        if result is None:result=ZERO
        report.update(status="INDEPENDENT_INTERVAL_COMPLETE",bits=BITS,sat=sat,
            real=[str(Q(x,S)) for x in result[:2]],imag=[str(Q(x,S)) for x in result[2:]],
            width=cert["width"],work=cert["work"],peak=cert["peak"],selected_mode=mode)
        break
    report["wall_s"]=monotonic()-start
    return report

def self_test():
    # Compare all assignments, not just aggregate numeric cancellation.
    fs=[((1,2),[(Q(1),Q(0)),None,(Q(0),Q(0)),(Q(-2),Q(1))]),
        ((1,2),[(Q(2),Q(1)),(Q(1),Q(0)),(Q(1),Q(0)),(Q(-1),Q(0))]),
        ((2,),[(Q(1),Q(0)),(Q(0),Q(0))]),
        ((3,4),[(Q(2),Q(0)),(Q(4),Q(0)),(Q(3),Q(0)),(Q(6),Q(0))])]
    def value(factors,scalar,m):
        for scope,vals in factors:
            scalar=o.mul(scalar,vals[sum(((m>>(v-1))&1)<<k for k,v in enumerate(scope))])
        return scalar
    for mode in ("same","subset"):
        fused,scalar,diag=fuse(fs,o.ONE,mode,monotonic()+5)
        assert diag["merges"]>=1
        for m in range(16):assert value(fs,o.ONE,m)==value(fused,scalar,m)
        if mode=="subset":assert diag["splits"]>=1
    # Scalar zero must retain support; unsupported scalar remains None.
    for vals in ([o.ZERO,o.ZERO],[None,None]):
        f=[((1,),vals),((1,),[o.ONE,o.ONE])]
        out,sc,_=fuse(f,o.ONE,"same",monotonic()+5)
        assert not out and sc==vals[0]

if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("cnf",type=Path);p.add_argument("prior",type=Path);p.add_argument("sha256")
    a=p.parse_args()
    self_test()
    raw=a.cnf.read_bytes();assert sha256(raw).hexdigest()==a.sha256
    prior=json.loads(a.prior.read_text());assert prior["source_sha256"]==a.sha256
    print(json.dumps(search(raw,a.sha256,prior)),flush=True)
