#!/usr/bin/env python3
"""Research-only source-locked implication SCC + signed Complex WMC audit.

Source CNF is NEVER modified; this calculates a hypothetical exact reduction.
Min-degree induced width is NOT optimal treewidth, and not a speed benchmark.
"""
from __future__ import annotations
import argparse
from collections import defaultdict
from hashlib import sha256
import heapq
import itertools
import json
from pathlib import Path

CASES = ("feynman_gf2_6_mult", "mqt_qnn_8", "mqt_wstate_16",
         "mqt_qwalk_5", "feynman_adder_8")

def parse(path):
    n = declared = None
    clauses = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        s = raw.strip()
        if not s or s.startswith("c"):
            continue
        if s.startswith("p "):
            head = s.split()
            if n is not None or len(head) != 4 or head[1] != "cnf":
                raise ValueError("INVALID_HEADER")
            n, declared = map(int, head[2:])
            continue
        if n is None:
            raise ValueError("MISSING_HEADER")
        values = tuple(map(int, s.split()))
        if not values or values[-1] != 0 or 0 in values[:-1]:
            raise ValueError("INVALID_CLAUSE")
        if any(abs(l) > n for l in values[:-1]):
            raise ValueError("INVALID_LITERAL")
        clauses.append(values[:-1])
    if n is None or n <= 0 or declared != len(clauses):
        raise ValueError("INVALID_CNF_COUNT")
    return n, tuple(clauses)

def signed_scc(n, clauses):
    graph = {v:set() for v in range(-n,n+1) if v}
    reversed_graph = {v:set() for v in graph}
    def edge(a,b):
        graph[a].add(b)
        reversed_graph[b].add(a)
    for clause in clauses:
        if len(clause) == 1:
            (a,) = clause
            edge(-a,a)
        elif len(clause) == 2:
            a,b = clause
            edge(-a,b)
            edge(-b,a)
    visited, finished = set(), []
    for start in sorted(graph, key=lambda x:(abs(x),x)):
        if start in visited:
            continue
        visited.add(start)
        stack = [(start,iter(graph[start]))]
        while stack:
            x,following = stack[-1]
            try:
                y = next(following)
            except StopIteration:
                finished.append(x)
                stack.pop()
                continue
            if y not in visited:
                visited.add(y)
                stack.append((y,iter(graph[y])))
    component, groups = {}, []
    for start in reversed(finished):
        if start in component:
            continue
        ident, todo, group = len(groups), [start], []
        component[start] = ident
        while todo:
            v = todo.pop()
            group.append(v)
            for w in reversed_graph[v]:
                if w not in component:
                    component[w] = ident
                    todo.append(w)
        groups.append(group)
    if any(component[x] == component[-x] for x in range(1,n+1)):
        return None, 0, 0
    mapping, seen, classes, eliminated = {}, set(), 0, 0
    for x in range(1,n+1):
        a,b = component[x],component[-x]
        if a in seen:
            continue
        seen.update((a,b))
        vertices = groups[a]+groups[b]
        rep = min(abs(v) for v in vertices)
        for v in groups[component[rep]]:
            mapping[v]=rep
        for v in groups[component[-rep]]:
            mapping[v]=-rep
        count=len(vertices)//2
        if count>1:
            classes+=1
            eliminated+=count-1
    assert len(mapping)==2*n
    assert all(mapping[-v]==-mapping[v] for v in range(1,n+1))
    return mapping,classes,eliminated

def reduced_clauses(clauses,mapping):
    output=[]
    removed_tautology=0
    for clause in clauses:
        new=set(mapping[v] for v in clause)
        if any(-v in new for v in new):
            removed_tautology+=1
            continue
        output.append(tuple(sorted(new,key=lambda v:(abs(v),v))))
    return tuple(dict.fromkeys(output)),removed_tautology

def exact_transfer(n,mapping,weights):
    """Keep BOTH literal factors of each eliminated variable, incl zeros."""
    out={}
    for v in range(1,n+1):
        rep=mapping[v]
        t=v if rep>0 else -v
        r=abs(rep)
        out[r]=out.get(r,1+0j)*weights.get(t,1+0j)
        out[-r]=out.get(-r,1+0j)*weights.get(-t,1+0j)
    return out

def exhaustive(variables,clauses,weights):
    """Return (Boolean SAT, signed Complex WMC); a zero WMC may be SAT."""
    vs=sorted(variables)
    assert len(vs)<=12
    sat,total=False,0j
    for values in itertools.product((False,True),repeat=len(vs)):
        assignment=dict(zip(vs,values))
        if all(any(assignment[abs(l)]==(l>0) for l in c) for c in clauses):
            sat=True
            w=1+0j
            for v in vs:
                w*=weights.get(v if assignment[v] else -v,1+0j)
            total+=w
    return sat,total

def greedy_width(clauses):
    adj=defaultdict(set)
    for clause in clauses:
        vs=sorted({abs(x) for x in clause})
        for v in vs: adj[v]
        for i,u in enumerate(vs):
            for v in vs[i+1:]:
                adj[u].add(v)
                adj[v].add(u)
    queue=[(len(near),x) for x,near in adj.items()]
    heapq.heapify(queue)
    width=0
    while queue:
        degree,x=heapq.heappop(queue)
        if x not in adj or len(adj[x])!=degree:
            continue
        near=list(adj.pop(x))
        width=max(width,len(near))
        for v in near:
            adj[v].discard(x)
        for i,u in enumerate(near):
            for v in near[i+1:]:
                adj[u].add(v)
                adj[v].add(u)
        for v in near:
            heapq.heappush(queue,(len(adj[v]),v))
    return width

def self_test():
    # x1=x2=not x3, plus x1 OR x4. Factors may be arbitrary complex.
    clauses=((-1,2),(1,-2),(-2,-3),(2,3),(1,4))
    m,classes,removed=signed_scc(4,clauses)
    assert m is not None and classes==1 and removed==2
    reduced,_=reduced_clauses(clauses,m)
    weight_sets=(
        {1:2+3j,-1:1-1j,2:-1+2j,-2:1+0j,3:1-2j,-3:2-2j,4:3+0j,-4:-2j},
        {1:0j,-1:1+0j,2:0j,-2:-1j,3:3+0j,-3:4j,4:2+0j,-4:1+0j},
        {1:1+0j,-1:-1+0j,2:1+0j,-2:1+0j,3:1+0j,-3:1+0j,4:1+0j,-4:1+0j},
    )
    for weights in weight_sets:
        old=exhaustive(range(1,5),clauses,weights)
        new=exhaustive({abs(x) for x in m.values()},reduced,exact_transfer(4,m,weights))
        assert old==new,(old,new)
    assert signed_scc(1,((1,),(-1,)))[0] is None
    sat,wmc=exhaustive((1,),((1,-1),),{1:1+0j,-1:-1+0j})
    assert sat and wmc==0j
    print("SCC_SIGNED_COMPLEX_WMC_EXHAUSTIVE_PASS",flush=True)

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--self-test",action="store_true")
    ap.add_argument("--screen-report",type=Path)
    ap.add_argument("--corpus",type=Path)
    ap.add_argument("--report",type=Path)
    args=ap.parse_args()
    self_test()
    if args.self_test:
        return
    if not args.screen_report or not args.corpus or not args.report:
        ap.error("missing --screen-report / --corpus / --report")
    source=json.loads(args.screen_report.read_text(encoding="utf-8"))
    if source.get("status")!="LONGTAIL_SCREEN_COMPLETE":
        raise SystemExit("SOURCE_SCREEN_INCOMPLETE")
    cases={row["case"]:row for row in source["cases"]}
    result={
        "status":"SOURCE_LOCKED_STRUCTURE_ONLY_NOT_SOLVER_SPEEDUP",
        "source_revisions":source["source_revisions"],
        "metric":"greedy_min_degree_induced_width_not_optimal_treewidth",
        "oracle":"exhaustive SAT + signed complex WMC incl zero and cancellation",
        "cases":[],
    }
    for name in CASES:
        item=cases.get(name)
        cnf=args.corpus / (name+"-gpmc.cnf")
        if not item or item.get("source_status")!="SOURCE_EXACT" or not cnf.is_file():
            raise SystemExit("MISSING_ORIGINAL_SOURCE_"+name)
        digest=sha256(cnf.read_bytes()).hexdigest()
        if digest!=item["cnf_sha256"]:
            raise SystemExit("SOURCE_CNF_SHA_DRIFT_"+name)
        n,clauses=parse(cnf)
        if n!=item["cnf_variables"] or len(clauses)!=item["cnf_clauses"]:
            raise SystemExit("SOURCE_CNF_HEADER_DRIFT_"+name)
        mapping,classes,eliminated=signed_scc(n,clauses)
        if mapping is None:
            raise SystemExit("SOURCE_CONTRADICTION_"+name)
        reduced,tauts=reduced_clauses(clauses,mapping)
        row={
            "case":name,"source_cnf_sha256":digest,
            "original_variables":n,"original_clauses":len(clauses),
            "binary_clauses":sum(len(c)==2 for c in clauses),
            "equivalence_classes":classes,"candidate_eliminated_variables":eliminated,
            "reduced_scope_variables":len({abs(v) for v in mapping.values()}),
            "reduced_clauses":len(reduced),"tautological_clauses_removed":tauts,
            "greedy_width_before":greedy_width(clauses),
            "greedy_width_after":greedy_width(reduced),
        }
        result["cases"].append(row)
        print("SCC_SOURCE_PROFILE case="+name+" eliminate="+str(eliminated)+
              " greedy_width="+str(row["greedy_width_before"])+
              "->"+str(row["greedy_width_after"]),flush=True)
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    print("SCC_SOURCE_LOCKED_STRUCTURAL_AUDIT_PASS",flush=True)

if __name__=="__main__":
    main()
