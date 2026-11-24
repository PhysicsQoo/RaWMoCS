#!/usr/bin/env python3
"""Research-only original-CNF cardinality contraction; no production defaults."""
import argparse
import collections
from fractions import Fraction
from pathlib import Path
import hashlib
import itertools
import json
import os
import subprocess
import sys
import tempfile
import time

import mpmath as mp
import networkx as nx
import numpy as np

from quprs_long_tail_paired import command, native, sha, git_sha, QU_PRS_SHA, GPMC_SHA

def extract(raw):
    n = None
    weights, clauses = {}, set()
    for line in raw.decode().splitlines():
        p = line.split()
        if not p:
            continue
        if p[0] == "p":
            assert n is None and len(p) == 4 and p[1] == "cnf"
            n, declared = map(int, p[2:])
            seen = 0
        elif p[:3] == ["c", "p", "weight"]:
            assert len(p) == 7 and p[-1] == "0"
            l = int(p[3])
            assert l not in weights
            weights[l] = (Fraction(p[4]), Fraction(p[5]))
        elif p[0] not in ("c", "%"):
            assert n is not None and p[-1] == "0"
            c = frozenset(map(int, p[:-1]))
            assert all(0 < abs(l) <= n for l in c)
            seen += 1
            if not any(-l in c for l in c):
                clauses.add(c)
    assert n is not None and seen == declared
    assert all(0 < abs(l) <= n for l in weights)
    occ = collections.defaultdict(set)
    for c in clauses:
        for l in c:
            occ[abs(l)].add(c)
    gates = {}
    for c in clauses:
        if len(c) != 3:
            continue
        for o in c:
            a = tuple(sorted(-l for l in c if l != o))
            defs = {c} | {frozenset((-o, l)) for l in a}
            if defs <= clauses and occ[abs(o)] == defs:
                gates[o, a] = defs
    outs = {abs(o) for o, a in gates}
    assert len(outs) == len(gates)
    assert all(abs(l) not in outs for o, a in gates for l in a)
    rem = clauses - set().union(*gates.values())
    vs = set(range(1, n + 1)) - outs
    graph = nx.DiGraph()
    graph.add_nodes_from(l for v in vs for l in (v, -v))
    for c in rem:
        if len(c) == 2:
            a, b = c
            graph.add_edge(-a, b)
            graph.add_edge(-b, a)
    rep = {}
    for s in nx.strongly_connected_components(graph):
        assert not any(-v in s for v in s)
        v = min(s, key=abs)
        for l in s:
            rep[l] = v
    assert all(rep[-l] == -v for l, v in rep.items())
    res = {frozenset(rep[l] for l in c) for c in rem}
    res = {c for c in res if not any(-l in c for l in c)}
    unit = {}
    while any(len(c) == 1 for c in res):
        l = next(iter(next(c for c in res if len(c) == 1)))
        assert abs(l) not in unit
        unit[abs(l)] = l > 0
        res = {c - {-l} for c in res if l not in c}
        assert frozenset() not in res
    assert not res, "unsupported residual hard clauses"
    free = {abs(rep[v]) for v in vs} - unit.keys()
    return free, rep, weights, unit, gates

def solve(path, bits, interval=False, dense=False):
    start = time.monotonic()
    ctx = mp.iv if interval else mp.mp
    ctx.prec = bits
    vs, rep, w, unit, gs = extract(path.read_bytes())
    def cplx(t):
        return ctx.mpc(ctx.mpf(t[0].numerator)/t[0].denominator,
                       ctx.mpf(t[1].numerator)/t[1].denominator)
    one, zero = ctx.mpc(1), ctx.mpc(0)
    def weight(l):
        return cplx(w.get(l, (1, 0)))
    graph = nx.Graph()
    graph.add_nodes_from(vs)
    for o, a in gs:
        b = tuple(rep[l] for l in a)
        assert len(b) == 2 and all(abs(l) in vs for l in b)
        assert abs(b[0]) != abs(b[1])
        graph.add_edge(*map(abs,b))
    # Research admission only: at most two disjoint complete uniform cliques.
    cliques = [] if dense else [set(c) for c in sorted(nx.find_cliques(graph),
                  key=lambda c: (-len(c), sorted(c)))[:2]]
    assert dense or (len(cliques) == 2 and not cliques[0] & cliques[1]
                     and all(2 <= len(c) <= 12 for c in cliques))
    unary = {v: [one, one] for v in vs}
    scalar = one
    for v in sorted(k for k in rep if k > 0):
        r = rep[v]
        a = abs(r)
        if a in unit:
            scalar *= weight(v if unit[a] == (r > 0) else -v)
        else:
            for x in (0, 1):
                unary[a][x] *= weight(v if bool(x) == (r > 0) else -v)
    factors = []
    edge_weights = [[] for _ in cliques]
    def factor(scope, vals):
        return tuple(scope), {(0,0): np.array(vals, dtype=object).reshape((2,)*len(scope))}
    for o, a in sorted(gs):
        b = tuple(rep[l] for l in a)
        scope = tuple(sorted(map(abs, b)))
        group = next((i for i,c in enumerate(cliques) if set(scope) <= c), None)
        if group is not None:
            assert all(l > 0 for l in b) and w.get(-o,(1,0)) == (1,0)
            edge_weights[group].append((scope,w.get(o,(1,0))))
            continue
        vals = []
        for mask in itertools.product((0,1), repeat=2):
            d = dict(zip(scope, mask))
            vals.append(weight(o if all(bool(d[abs(l)]) == (l > 0) for l in b) else -o))
        factors.append(factor(scope, vals))
    phases = []
    for c, edges in zip(cliques, edge_weights):
        assert {tuple(sorted(p)) for p in itertools.combinations(c,2)} == {a for a,b in edges}
        assert len(edges) == len(c)*(len(c)-1)//2 and len({b for a,b in edges}) == 1
        phases.append(cplx(edges[0][1]))
        graph.remove_edges_from([a for a,b in edges])
    for v in sorted(vs):
        group = next((i for i,c in enumerate(cliques) if v in c), None)
        if group is None:
            factors.append(factor((v,),unary[v]))
        else:
            factors.append(((v,), {(0,0):np.array([unary[v][0],zero],dtype=object),
                (1,0) if group == 0 else (0,1):np.array([zero,unary[v][1]],dtype=object)}))
    def mul(A, B):
        sa,pa=A; sb,pb=B
        s=tuple(sorted(set(sa)|set(sb)))
        assert len(s) <= 16, "scope budget"
        assert len(pa)*len(pb)*(2**len(s)) <= 8_000_000, "coefficient work budget"
        ashape=tuple(2 if v in sa else 1 for v in s)
        bshape=tuple(2 if v in sb else 1 for v in s)
        p={}
        for (i,j),a in pa.items():
            for (k,l),b in pb.items():
                key=(i+k,j+l)
                x=a.reshape(ashape)*b.reshape(bshape)
                p[key]=p[key]+x if key in p else x
        return s,p
    adj={v:set(graph[v]) for v in graph}
    width=peak=0
    while adj:
        def key(v):
            s=adj[v]
            return (sum(y not in adj[x] for x,y in itertools.combinations(s,2)),
                    len(s),-v if interval else v)
        v=min(adj,key=key)
        nb=adj.pop(v)
        width=max(width,len(nb))
        bucket=[f for f in factors if v in f[0]]
        factors=[f for f in factors if v not in f[0]]
        assert bucket
        bucket.sort(key=lambda f:(len(f[1]),len(f[0])))
        a=bucket.pop(0)
        for b in bucket:
            a=mul(a,b)
        scope,p=a
        peak=max(peak,len(p)*2**len(scope))
        out=tuple(x for x in scope if x!=v)
        factors.append((out,{k:np.sum(x,axis=scope.index(v)) for k,x in p.items()}))
        for u in nb:
            adj[u].discard(v)
            adj[u].update(nb-{u})
    a=factors.pop(0)
    for b in factors:
        a=mul(a,b)
    assert not a[0]
    val=zero
    for (i,j),v in a[1].items():
        term=v.item() if isinstance(v,np.ndarray) else v
        for phase,k in zip(phases,(i,j)):
            if interval:
                for _ in range(k*(k-1)//2):
                    term*=phase
            else:
                term*=phase**(k*(k-1)//2)
        val+=term
    val*=scalar
    row=dict(bits=bits, sat=True, free_vars=len(vs), gates=len(gs), cliques=list(map(len,cliques)),
             width=width,peak_coeff_cells=peak,solve_s=time.monotonic()-start)
    if interval:
        row.update(real_interval=val.real._mpi_,imag_interval=val.imag._mpi_)
    else:
        row.update(real=mp.nstr(val.real,int(bits*.30103)+4),
                   imag=mp.nstr(val.imag,int(bits*.30103)+4))
    return row

def fraction_mpf(x):
    sign, mantissa, exponent, _ = x
    return (-1 if sign else 1)*Fraction(mantissa)*Fraction(2)**exponent

def check(row,witness,digits):
    assert row["sat"] is True
    ranges=[[fraction_mpf(x) for x in witness[k+"_interval"]] for k in ("real","imag")]
    error=max(abs(Fraction(row[k])-x) for k,ends in zip(("real","imag"),ranges) for x in ends)
    scale=max(Fraction(1), *(max(Fraction(0), lo, -hi) for lo,hi in ranges))
    assert error <= scale*Fraction(1,10**(digits-6)), "independent interval guard"
    return str(float(error/scale))

def run_candidate(cnf,bits,mode):
    with tempfile.TemporaryDirectory() as td:
        rss=Path(td)/"rss"
        cmd=["/usr/bin/timeout","-k","2s","120s","prlimit","--as=3221225472",
             "--fsize=16777216","--","/usr/bin/time","-f","%M","-o",str(rss),
             sys.executable,str(Path(__file__).resolve()),"--worker",str(cnf),str(bits),mode]
        t=time.perf_counter_ns()
        proc=subprocess.run(cmd,capture_output=True,text=True,timeout=127)
        row=dict(arm=mode,wall_ns=time.perf_counter_ns()-t,exit_code=proc.returncode,
                 status="COMPLETED" if proc.returncode==0 else
                 "CENSORED_TIME" if proc.returncode==124 else "CENSORED_EXIT_OR_RESOURCE",
                 stderr=proc.stderr[-3000:])
        if rss.exists() and rss.read_text().strip().isdigit():
            row["peak_rss_kib"]=int(rss.read_text().strip())
        if proc.returncode==0:
            row.update(json.loads(proc.stdout))
        return row

def main():
    if len(sys.argv)>1 and sys.argv[1]=="--worker":
        _,_,path,bits,mode=sys.argv
        print(json.dumps(solve(Path(path),int(bits),mode=="interval")))
        return
    ap=argparse.ArgumentParser()
    for field in ("corpus","quprs","gpmc-source","gpmc","report"):
        ap.add_argument("--"+field,type=Path,required=True)
    args=ap.parse_args()
    assert git_sha(args.quprs)==QU_PRS_SHA and git_sha(args.gpmc_source)==GPMC_SHA
    e=json.loads((args.corpus/"long-tail-source-locked-evidence.json").read_text())
    assert e["status"]=="LONGTAIL_SCREEN_COMPLETE" and e["source_revisions"]=={"quprs":QU_PRS_SHA,"gpmc":GPMC_SHA}
    out={"kind":"PYTHON_RESEARCH_PILOT_NOT_RUST_NATIVE","source_revisions":e["source_revisions"],"cases":[]}
    def persist():
        args.report.parent.mkdir(parents=True,exist_ok=True)
        args.report.write_text(json.dumps(out,indent=2)+"\n")
    for name in ("mqt_qnn_8","mqt_qnn_12"):
        rec=next(c for c in e["cases"] if c["case"]==name)
        cnf=args.corpus/(name+"-gpmc.cnf")
        assert rec["source_status"]=="SOURCE_EXACT" and sha(cnf)==rec["cnf_sha256"]
        assert sha(args.quprs/rec["original_qasm"])==rec["source_qasm_sha256"]
        case={"case":name,"cnf_sha256":sha(cnf),"trials":[]}
        out["cases"].append(case)
        witness=run_candidate(cnf,450,"interval")
        case["interval"]=witness
        persist()
        assert witness["status"]=="COMPLETED"
        for digits,bits in ((30,100),(100,333)):
            # One exploratory paired fresh process per precision, reversed order.
            arms=("gpmc","cardinality") if bits==100 else ("cardinality","gpmc")
            for arm in arms:
                row=native(command(args,name,cnf,digits,"gpmc"),"gpmc",120,digits) if arm=="gpmc" else run_candidate(cnf,bits,"cardinality")
                row["bits"]=bits
                case["trials"].append(row)
                persist()
                if row["status"]=="COMPLETED":
                    row["relative_error_upper"]=check(row,witness,digits)
                print("QNN_CARDINALITY",name,bits,arm,json.dumps(row),flush=True)
                persist()
            assert all(r["status"]=="COMPLETED" for r in case["trials"] if r["arm"]=="cardinality"),"candidate incomplete"
    out["status"]="EXPLORATORY_INTERVAL_CHECKED"
    persist()

if __name__=="__main__":
    main()
