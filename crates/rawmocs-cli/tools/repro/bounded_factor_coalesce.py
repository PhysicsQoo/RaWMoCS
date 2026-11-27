#!/usr/bin/env python3
"""Exact algebraic factor preprocessing; original cap replay, independent interval.
Research only. Count preprocessing and every cut branch; no production policy.
"""
import argparse
from collections import defaultdict
from fractions import Fraction as Q
from hashlib import sha256
from itertools import combinations, product
import json
from pathlib import Path
from time import monotonic
import original_exact_prejoin as o
from original_fixed_interval import BITS, S, ZERO, add as iadd, solve as fixed

PREP_CAP=1_000_000

def coalesce(fs, scalar, rank_one, deadline):
    before=sum(len(t) for _,t in fs)
    groups=defaultdict(list)
    for s,t in fs: groups[s].append(t)
    work=merges=splits=drops=rejected=0
    out=[]
    for s,tables in groups.items():
        if monotonic()>=deadline: raise ValueError("PREP_DEADLINE")
        if len(tables)==1:
            out.append((s,tables[0]));continue
        charge=len(tables)*len(tables[0])
        if work+charge>PREP_CAP:
            out.extend((s,t) for t in tables);continue
        work+=charge
        try:
            vals=[]
            for cells in zip(*tables):
                x=o.ONE
                for v in cells:
                    x=o.mul(x,v)
                    if x is None:break
                vals.append(x)
            ns,vals,d=o.reduce_axes(s,vals)
            if not ns:
                folded=o.mul(scalar,vals[0])
            else: folded=scalar
        except ValueError as exc:
            if str(exc)!="LOCAL_EXACT_BITS":raise
            rejected+=1
            out.extend((s,t) for t in tables);continue
        scalar=folded
        merges+=len(tables)-1
        drops+=d
        if ns:out.append((ns,vals))
    if rank_one:
        pending=list(reversed(out));out=[]
        while pending:
            s,t=pending.pop()
            found=None
            if 2<=len(s)<=9:
                for k in range(1,len(s)):
                    for tail in combinations(s[1:],k-1):
                        # Charge division, full reconstruction and comparisons.
                        charge=8*len(t)
                        if work+charge>PREP_CAP or monotonic()>=deadline:break
                        work+=charge
                        found=o.split_rank_one(s,t,(s[0],)+tail)
                        if found is not None:break
                    if found is not None or work+8*len(t)>PREP_CAP or monotonic()>=deadline:break
            if found is None:out.append((s,t))
            else:
                splits+=1
                pending.extend(reversed(found))
    after=sum(len(t) for _,t in out)
    # Conservative table-cell proxy includes input, intermediate output,
    # temporary group output, scalar and rank-one reconstruction scratch.
    peak=4*(before+after)+8
    if peak>o.MAX_LIVE:raise ValueError("PREP_LIVE")
    assert all(s and len(t)==1<<len(s) for s,t in out)
    return out,scalar,dict(work=work,merged=merges,rank_one_splits=splits,
        axis_drops=drops,bit_cap_rejected_groups=rejected,peak=peak,
        factors_before=len(fs),factors_after=len(out),cells_before=before,cells_after=after)

def solve(raw,expected,prior,rank_one):
    start=monotonic()
    deadline=start+195
    width=prior["local_width"]
    fs,ns,scalar,local=o.local_reduce(raw,expected,start+30,diagnostic=True,local_width=width,incremental=True)
    assert local==prior["local"]
    report=dict(status="DIAGNOSTIC_ONLY",source_sha256=expected,local_width=width,
                rank_one=rank_one,rows=[])
    original_edges=sum(map(len,o.residual_components(fs,ns)[0].values()))//2
    # Preserve prior fixed orders; no more graph-only searches.
    fs2,scalar2,prep=coalesce(fs,scalar,rank_one,min(deadline,monotonic()+15))
    reduced_edges=sum(map(len,o.residual_components(fs2,ns)[0].values()))//2
    report["root_preprocess"]=dict(prep,edges_before=original_edges,edges_after=reduced_edges)
    retained=local["retained_cells"]+sum(len(t) for _,t in fs2)
    for old in prior["replay"]:
        if monotonic()>=deadline:break
        cuts,seq=old["cutset"],old["order"]
        row=dict(cutset=cuts,order=seq,branches=0,work=local["work"]+prep["work"],
                 peak=max(local["peak"],prep["peak"]+local["retained_cells"]),
                 width=0,merged=prep["merged"],rank_one_splits=prep["rank_one_splits"],
                 axis_drops=prep["axis_drops"],branches_detail=[])
        report["rows"].append(row)
        plans=[]
        for values in product((False,True),repeat=len(cuts)):
            if monotonic()>=deadline:break
            branch,sc=o.restrict(fs2,scalar2,cuts,values)
            f,c,pr=coalesce(branch,sc,rank_one,min(deadline,monotonic()+10))
            row["merged"]+=pr["merged"]
            row["rank_one_splits"]+=pr["rank_one_splits"]
            row["axis_drops"]+=pr["axis_drops"]
            row["work"]+=retained+pr["work"]
            # Keep all exact factors for admitted branch replay resident.
            p=o.plan(f,ns-set(cuts),(None,False),retained,deadline,fixed_order=seq)
            if p is None:
                row["branches_detail"].append(dict(values=values,status="PLAN_REJECTED",preprocess=pr))
                break
            row["branches"]+=1
            row["work"]+=p["work"]
            row["peak"]=max(row["peak"],pr["peak"]+retained,p["peak"])
            row["width"]=max(row["width"],p["width"])
            plans.append((f,c,p))
            row["branches_detail"].append(dict(values=values,status="PLAN_COMPLETE",preprocess=pr,work=p["work"],peak=p["peak"],width=p["width"]))
        # Count all saved branch exact tables and scalar entries as extra live.
        saved=sum(sum(len(t) for _,t in f)+1 for f,_,_ in plans)
        row["peak"]+=saved
        row["status"]="CAP_PASS" if (len(plans)==1<<len(cuts) and row["work"]<=o.MAX_WORK
                                      and row["peak"]<=o.MAX_LIVE and row["width"]<=o.MAX_WIDTH) else "CAP_REJECTED"
        if row["status"]!="CAP_PASS":continue
        result=None
        try:
            for f,c,p in plans:
                result=iadd(result,o.execute(f,c,p,min(deadline,start+195)))
        except ValueError as exc:
            if str(exc) not in ("JOIN_DEADLINE","SUM_DEADLINE","EXECUTE_DEADLINE"):raise
            row["execution_stop"]=str(exc)
            break
        sat=result is not None
        if result is None:result=ZERO
        report.update(status="INDEPENDENT_INTERVAL_COMPLETE",bits=BITS,sat=sat,
                      real=[str(Q(x,S)) for x in result[:2]],imag=[str(Q(x,S)) for x in result[2:]],
                      width=row["width"],work=row["work"],peak=row["peak"])
        break
    report["wall_s"]=monotonic()-start
    return report

def self_test():
    # Check exact transformation directly on every assignment, including
    # supported zero, contradictory support, signed complex and rank-one.
    fs=[((1,2),[(Q(1),Q(0)),(Q(2),Q(0)),(Q(0),Q(1)),(Q(0),Q(2))]),
        ((1,2),[o.ONE,o.ONE,o.ONE,o.ONE]),
        ((2,3),[o.ZERO,None,(Q(-1),Q(1)),o.ONE]),
        ((2,3),[o.ONE,o.ONE,(Q(1),Q(1)),o.ONE])]
    def evaluate(fs,scalar,m):
        for s,t in fs:
            ix=sum(1<<j for j,v in enumerate(s) if m&(1<<(v-1)))
            scalar=o.mul(scalar,t[ix])
        return scalar
    for rank in (False,True):
        f,c,r=coalesce(fs,o.ONE,rank,monotonic()+10)
        assert r["merged"]==2
        if rank: assert r["rank_one_splits"]>=1
        for m in range(8):assert evaluate(fs,o.ONE,m)==evaluate(f,c,m)
    # Exact cap rejection must leave the whole group intact, no partial update.
    huge=(Q(1<<10000),Q(0))
    fs=[((1,),[huge,o.ONE]),((1,),[huge,o.ONE])]
    f,c,r=coalesce(fs,o.ONE,False,monotonic()+10)
    assert f==fs and c==o.ONE and r["bit_cap_rejected_groups"]==1

if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("cnf",type=Path)
    p.add_argument("prior",type=Path)
    p.add_argument("sha256")
    p.add_argument("--rank-one",action="store_true")
    a=p.parse_args()
    self_test()
    raw=a.cnf.read_bytes()
    assert sha256(raw).hexdigest()==a.sha256
    prior=json.loads(a.prior.read_text())
    assert prior["source_sha256"]==a.sha256
    print(json.dumps(solve(raw,a.sha256,prior,a.rank_one)),flush=True)
