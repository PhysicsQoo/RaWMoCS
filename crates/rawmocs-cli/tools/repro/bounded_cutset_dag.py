#!/usr/bin/env python3
"""Research-only cross-branch exact expression DAG with last-use schedule.

All branch plans individually pass original caps. Aggregate original dense
work can exceed the cap in a structural diagnostic, but numerical execution
is forbidden unless the scheduled unique work and conservative live peak pass.
"""
import argparse
from collections import Counter
from fractions import Fraction as Q
from hashlib import sha256
from itertools import product
import json
from pathlib import Path
from time import monotonic
import original_exact_prejoin as o
from original_fixed_interval import BITS,S,ONE,ZERO,add,mul


def build(branches,local):
    ids={}
    nodes=[]
    roots=[]
    repeated=0
    def intern(key,record):
        if key not in ids:
            ids[key]=len(nodes)
            nodes.append(record)
        return ids[key]
    def leaf(scope,vals):
        return intern(("input",scope,tuple(vals)),("input",scope,tuple(vals),None))
    for fs,scalar,p in branches:
        tables={i:leaf(s,t) for i,(s,t) in enumerate(fs)}
        for op,owners,dst,out,v in p["ops"]:
            parents=tuple(tables[i] for i in owners)
            key=(op,out,parents,v)
            node=intern(key,key)
            repeated+=2*(1<<len(out))*(1 if op=="join" else len(parents))
            for i in owners: del tables[i]
            tables[dst]=node
        assert all(not nodes[i][1] for i in tables.values())
        roots.append((leaf((),[scalar]),*tables.values()))
    # Topological creation order; each node appears before its consumers.
    uses=Counter(i for op,s,parents,v in nodes if op!="input" for i in parents)
    uses.update(i for root in roots for i in root)
    # Roots are reduced when all their dependencies are available, not at end.
    ready={}
    for root in roots: ready.setdefault(max(root),[]).append(root)
    live=peak=work=0
    sizes=[1<<len(n[1]) for n in nodes]
    # Conservative exact resident source plus all distinct restricted inputs,
    # even if their Python objects share cells. Numerical table live tracked
    # separately. A cell budget is not a byte/RSS bound.
    resident=local["retained_cells"]+sum(sizes[i] for i,n in enumerate(nodes) if n[0]=="input")
    conditioning=local["work"]+len(branches)*local["retained_cells"]
    work=conditioning
    remaining=uses.copy()
    active=set()
    def release(i):
        nonlocal live
        remaining[i]-=1
        assert remaining[i]>=0
        if remaining[i]==0:
            assert i in active
            active.remove(i)
            live-=sizes[i]
    peak=resident
    for i,(op,s,parents,v) in enumerate(nodes):
        size=sizes[i]
        assert i not in active and remaining[i]>0
        active.add(i);live+=size
        peak=max(peak,resident+live+2) # scalar accumulation scratch
        if op!="input":
            work+=2*size*(1 if op=="join" else len(parents))
            for p in parents: release(p)
        for root in ready.get(i,()):
            work+=2*len(root)+1 # conservative root multiplications and addition
            for p in root: release(p)
    assert not active and live==0 and not any(remaining.values())
    return nodes,roots,ready,uses,dict(branches=len(branches),
        nodes=len(nodes),unique_operations=sum(n[0]!="input" for n in nodes),
        repeated_work=repeated+conditioning,scheduled_work=work,
        resident_exact_cells=resident,scheduled_peak=peak,
        work_cap_pass=work<=o.MAX_WORK,live_cap_pass=peak<=o.MAX_LIVE)


def execute(nodes,roots,ready,uses,deadline):
    tables={}
    remain=uses.copy()
    result=None
    def release(i):
        remain[i]-=1
        if remain[i]==0: del tables[i]
    for i,(op,scope,parents,v) in enumerate(nodes):
        if monotonic()>=deadline: raise ValueError("EXECUTE_DEADLINE")
        if op=="input":
            vals=[o.interval(x) for x in parents]
        else:
            bucket=[tables[p] for p in parents]
            vals=o.table_join(*bucket,scope,deadline) if op=="join" else o.table_sum(bucket,v,scope,deadline)
            del bucket
        tables[i]=(scope,vals)
        if op!="input":
            for p in parents: release(p)
        for root in ready.get(i,()):
            x=ONE
            for p in root:
                s,t=tables[p]
                assert not s and len(t)==1
                x=mul(x,t[0])
            result=add(result,x)
            for p in root: release(p)
        del vals
    assert not tables and not any(remain.values())
    return result


def inspect(raw,expected,prior,seconds=210):
    start=monotonic()
    deadline=start+seconds
    width=prior["local_width"]
    fs,ns,scalar,local=o.local_reduce(raw,expected,min(deadline,start+30),diagnostic=True,local_width=width,incremental=True)
    assert local==prior["local"]
    report=dict(status="DIAGNOSTIC_ONLY",source_sha256=expected,local_width=width,rows=[])
    for old in prior["replay"]:
        if monotonic()>=deadline: break
        cuts,seq=old["cutset"],old["order"]
        row=dict(cutset=cuts,order=seq,graph_width=old["graph_width"])
        report["rows"].append(row)
        branches=[]
        for values in product((False,True),repeat=len(cuts)):
            branch,sc=o.restrict(fs,scalar,cuts,values)
            p=o.plan(branch,ns-set(cuts),(None,False),local["retained_cells"],deadline,fixed_order=seq)
            if p is None: break
            branches.append((branch,sc,p))
        if len(branches)!=1<<len(cuts):
            row.update(status="INDIVIDUAL_BRANCH_REJECTED",branches=len(branches));continue
        nodes,roots,ready,uses,profile=build(branches,local)
        row.update(profile)
        row["status"]="SCHEDULE_CAP_PASS" if profile["work_cap_pass"] and profile["live_cap_pass"] else "SCHEDULE_CAP_REJECTED"
        # Support visits are an independent proxy, not admissible work.
        try:
            row["support"]=o.support_work_profile([(f,p) for f,_,p in branches],local,min(deadline,monotonic()+15))
        except ValueError as exc:
            if str(exc)!="SUPPORT_DEADLINE": raise
            row["support"]={"status":"SUPPORT_DEADLINE"}
        if row["status"]=="SCHEDULE_CAP_PASS":
            try:
                value=execute(nodes,roots,ready,uses,min(deadline,monotonic()+90))
            except ValueError as exc:
                if str(exc) not in ("JOIN_DEADLINE","SUM_DEADLINE","EXECUTE_DEADLINE"):raise
                row["execution_stop"]=str(exc)
                break
            sat=value is not None
            if value is None: value=ZERO
            report.update(status="INDEPENDENT_INTERVAL_COMPLETE",bits=BITS,sat=sat,
                real=[str(Q(x,S)) for x in value[:2]],imag=[str(Q(x,S)) for x in value[2:]],
                width=max(p["width"] for _,_,p in branches),work=profile["scheduled_work"],peak=profile["scheduled_peak"])
            break
    report["wall_s"]=monotonic()-start
    return report


def self_test():
    # Signed complex values, supported zero and unsatisfiable assignments.
    from original_fixed_interval import solve as fixed
    for raw in (b"p cnf 3 2\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight -2 0 0 0\n1 2 0\n-1 3 0\n",
                b"p cnf 2 2\n1 0\n-1 0\n"):
        expected=sha256(raw).hexdigest()
        n,clauses,scopes,weights=o.parse(raw,expected)
        fs=[(s,[o.ONE if any(bool(m&(1<<s.index(abs(l))))==(l>0) for l in c) else None for m in range(1<<len(s))]) for c,s in zip(clauses,scopes)]
        fs.extend(((v,),[weights.get(-v,o.ONE),weights.get(v,o.ONE)]) for v in range(1,n+1))
        # Duplicate identical cut branch operations are expected to share.
        local=dict(retained_cells=sum(len(t) for _,t in fs),work=0)
        branches=[]
        for values in ((False,),(True,)):
            f,sc=o.restrict(fs,o.ONE,(1,),values)
            p=o.plan(f,set(range(2,n+1)),(None,False),local["retained_cells"],monotonic()+10)
            assert p is not None
            branches.append((f,sc,p))
        nodes,roots,ready,uses,profile=build(branches,local)
        assert profile["work_cap_pass"] and profile["live_cap_pass"]
        out=execute(nodes,roots,ready,uses,monotonic()+10)
        ref=fixed(raw,expected)
        assert (out is not None)==ref["sat"]
        if out is None:out=ZERO
        for key,vals in (("real",out[:2]),("imag",out[2:])):
            a,b=map(Q,ref[key]);x,y=(Q(v,S) for v in vals)
            assert x<=b and a<=y


if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("cnf",type=Path)
    p.add_argument("prior",type=Path)
    p.add_argument("sha256")
    a=p.parse_args()
    self_test()
    raw=a.cnf.read_bytes()
    assert sha256(raw).hexdigest()==a.sha256
    prior=json.loads(a.prior.read_text())
    assert prior["source_sha256"]==a.sha256
    print(json.dumps(inspect(raw,a.sha256,prior)),flush=True)
