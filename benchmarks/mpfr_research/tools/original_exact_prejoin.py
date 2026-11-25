"""Independent bounded original-CNF oracle: exact local sums, cutsets, prejoins.

No native gate recognition, SCC, source-family dispatch, or phase snapping.
Plan all branches before interval execution; retain original width/work/live limits.
"""
import argparse
from fractions import Fraction as Q
from hashlib import sha256
from itertools import combinations, product
import heapq
import json
from pathlib import Path
from time import monotonic

from original_fixed_interval import (
    BITS, S, ONE as I_ONE, ZERO as I_ZERO, enclosure, mul as imul, add as iadd,
    MAX_WIDTH, MAX_WORK, MAX_LIVE, MAX_CLAUSE_WIDTH, MAX_SOURCE_VARS, MAX_SOURCE_CLAUSES,
)

ONE = (Q(1), Q(0))
ZERO = (Q(0), Q(0))
LOCAL_WIDTH = 4
LOCAL_WORK = 1_000_000
EXACT_BITS = 16384
RESIDUAL_CAP = 64
PLAN_SECONDS = 30
POLICIES = tuple(product((0, 4, None), (True, False)))


def checked(x):
    if x is not None and any(max(a.numerator.bit_length(), a.denominator.bit_length()) > EXACT_BITS for a in x):
        raise ValueError("LOCAL_EXACT_BITS")
    return x


def mul(a, b):
    if a is None or b is None:
        return None
    if a == ONE:
        return b
    if b == ONE:
        return a
    if a == ZERO or b == ZERO:
        return ZERO
    return checked((a[0]*b[0]-a[1]*b[1], a[0]*b[1]+a[1]*b[0]))


def add(a, b):
    if a is None:
        return b
    if b is None:
        return a
    return checked((a[0]+b[0], a[1]+b[1]))


def parse(raw, expected):
    if sha256(raw).hexdigest() != expected:
        raise ValueError("SOURCE_DRIFT")
    n = nc = None
    weights, clauses = {}, []
    for line in raw.decode().splitlines():
        p = line.split()
        if not p:
            continue
        if p[:2] == ["p", "cnf"]:
            if n is not None or len(p) != 4:
                raise ValueError("HEADER")
            n, nc = map(int, p[2:])
            if not 0 <= n <= MAX_SOURCE_VARS or not 0 <= nc <= MAX_SOURCE_CLAUSES:
                raise ValueError("SOURCE_CAP")
        elif p[:3] == ["c", "p", "weight"]:
            if n is None or len(p) != 7 or p[-1] != "0":
                raise ValueError("WEIGHT_FORMAT")
            v = int(p[3])
            if not 1 <= abs(v) <= n or v in weights:
                raise ValueError("WEIGHT_OWNERSHIP")
            for a in p[4:6]:
                if len(a) > 128 or ("e" in a.lower() and abs(int(a.lower().split("e")[1])) > 1024):
                    raise ValueError("WEIGHT_RANGE")
            weights[v] = checked((Q(p[4]), Q(p[5])))
        elif p[0] != "c":
            if n is None or p[-1] != "0":
                raise ValueError("CLAUSE_FORMAT")
            c = tuple(map(int, p[:-1]))
            if any(not 1 <= abs(v) <= n for v in c):
                raise ValueError("CLAUSE_LITERAL")
            clauses.append(c)
            if len(clauses) > nc:
                raise ValueError("CLAUSE_COUNT")
    if n is None or len(clauses) != nc:
        raise ValueError("CLAUSE_COUNT")
    scopes = [tuple(sorted(set(map(abs, c)))) for c in clauses]
    if any(len(s) > MAX_CLAUSE_WIDTH for s in scopes):
        raise ValueError("CLAUSE_WIDTH")
    if 2*n + sum(1 << len(s) for s in scopes) > MAX_LIVE:
        raise ValueError("INITIAL_RESOURCE_CAP")
    return n, clauses, scopes, weights


def reduce_axes(scope, vals):
    # Only EXACT rational equality (including None vs supported zero).
    dropped = 0
    for j in range(len(scope)-1, -1, -1):
        bit = 1 << j
        if all(vals[m] == vals[m|bit] for m in range(len(vals)) if not m&bit):
            vals = [x for m, x in enumerate(vals) if not m&bit]
            scope = scope[:j]+scope[j+1:]
            dropped += 1
    return scope, vals, dropped


def local_reduce(raw, expected, deadline, *, diagnostic=False):
    n, clauses, scopes, weights = parse(raw, expected)
    fs = {}
    inc = {v: set() for v in range(1, n+1)}
    next_id = live = peak = drops = work = eliminated = 0
    scalar = ONE

    def insert(scope, vals):
        nonlocal next_id, live, peak, drops, scalar
        peak = max(peak, live+2*len(vals))
        if peak > MAX_LIVE:
            raise ValueError("LOCAL_LIVE")
        scope, vals, d = reduce_axes(scope, vals)
        drops += d
        if not scope:
            scalar = mul(scalar, vals[0])
            return
        fs[next_id] = (scope, vals)
        live += len(vals)
        for v in scope:
            inc[v].add(next_id)
        next_id += 1

    for c, scope in zip(clauses, scopes):
        vals = [ONE if any(bool(m&(1<<scope.index(abs(l)))) == (l>0) for l in c) else None
                for m in range(1<<len(scope))]
        insert(scope, vals)
    for v in inc:
        insert((v,), [weights.get(-v, ONE), weights.get(v, ONE)])

    def neighbors(v):
        return set().union(*(set(fs[i][0]) for i in inc[v]))-{v}

    while inc:
        if monotonic() >= deadline:
            raise ValueError("LOCAL_DEADLINE")
        v = min(inc, key=lambda v: (len(neighbors(v)), len(inc[v]), -v))
        scope = tuple(sorted(neighbors(v)))
        if len(scope) > LOCAL_WIDTH:
            break
        ids = sorted(inc[v])
        bucket = [fs[i] for i in ids]
        size = 1<<len(scope)
        work += 2*size*len(bucket)
        peak = max(peak, live+2*size)
        if work > LOCAL_WORK or peak > MAX_LIVE:
            raise ValueError("LOCAL_RESOURCE")
        out = []
        for m in range(size):
            pair = []
            for positive in (False, True):
                x = ONE
                for s, t in bucket:
                    j = sum(1<<k for k, u in enumerate(s)
                            if (positive if u == v else bool(m&(1<<scope.index(u)))))
                    x = mul(x, t[j])
                    if x is None:
                        break
                pair.append(x)
            out.append(add(*pair))
        for i, (s, t) in zip(ids, bucket):
            del fs[i]
            live -= len(t)
            for u in s:
                inc[u].remove(i)
        del inc[v]
        insert(scope, out)
        eliminated += 1
    if not diagnostic and len(inc) > RESIDUAL_CAP:
        raise ValueError("RESIDUAL_CAP")
    return list(fs.values()), set(inc), scalar, dict(variables=n, clauses=len(clauses),
        eliminated=eliminated, residual=len(inc), factors=len(fs), dropped_axes=drops,
        work=work, peak=peak, retained_cells=live)


def restrict(factors, scalar, pivots, values):
    result = []
    for scope, vals in factors:
        for v, value in zip(pivots, values):
            if v in scope:
                j = scope.index(v)
                scope = scope[:j]+scope[j+1:]
                vals = [x for m, x in enumerate(vals) if bool(m&(1<<j)) == value]
        scope, vals, _ = reduce_axes(scope, vals)
        if scope:
            result.append((scope, vals))
        else:
            scalar = mul(scalar, vals[0])
    return result, scalar


def plan(factors, nodes, policy, retained, deadline):
    penalty, rev = policy
    g = {v:set() for v in nodes}
    ss = {i:set(s) for i, (s, _) in enumerate(factors)}
    inc = {v:set() for v in nodes}
    for i, s in ss.items():
        for v in s:
            inc[v].add(i)
            g[v].update(s-{v})

    def score(v):
        a = g[v]
        d = len(a)
        f = 0 if penalty is None else sum(b not in g[u] for u,b in combinations(a,2))
        return (d if penalty is None else f+penalty*d, d, -v if rev else v, v)

    h = [score(v) for v in g]
    heapq.heapify(h)
    cur = {x[-1]:x for x in h}
    live = sum(1<<len(s) for s in ss.values())
    # The exact source factors and restricted exact tables remain resident.
    extra = retained+live
    peak = live+extra
    work = width = joins = 0
    idx = len(ss)
    ops = []
    while g:
        if monotonic() >= deadline:
            raise ValueError("PLAN_DEADLINE")
        while True:
            x = heapq.heappop(h)
            v = x[-1]
            if cur.get(v) == x:
                break
        cur.pop(v)
        nb = g.pop(v)
        width = max(width,len(nb))
        if width > MAX_WIDTH:
            return None
        size = 1<<len(nb)
        ids = sorted(inc.pop(v))
        bucket = [(i,ss.pop(i)) for i in ids]
        for i,s in bucket:
            for u in s-{v}:
                inc[u].remove(i)
        while len(bucket)>2:
            choices = [(len(a|b),i,j) for i,(_,a) in enumerate(bucket)
                       for j,(_,b) in enumerate(bucket) if j>i and len(a|b)<=len(nb)-1]
            if not choices:
                break
            _,i,j = min(choices)
            ia,a = bucket[i]
            ib,b = bucket[j]
            u = a|b
            sz = 1<<len(u)
            if live+sz+extra > MAX_LIVE:
                break
            work += 2*sz
            peak = max(peak,live+sz+extra)
            live += sz-(1<<len(a))-(1<<len(b))
            ops.append(("join",(ia,ib),idx,tuple(sorted(u)),None))
            bucket.pop(j)
            bucket.pop(i)
            bucket.append((idx,u))
            idx += 1
            joins += 1
        work += 2*size*len(bucket)
        peak = max(peak,live+size+extra)
        if work > MAX_WORK or peak > MAX_LIVE:
            return None
        live += size-sum(1<<len(s) for _,s in bucket)
        ops.append(("sum",tuple(i for i,_ in bucket),idx,tuple(sorted(nb)),v))
        ss[idx] = nb.copy()
        affected = set(nb)
        for u in nb:
            g[u].discard(v)
            g[u].update(nb-{u})
            inc[u].add(idx)
            affected.update(g[u])
        idx += 1
        for u in affected:
            x = score(u)
            cur[u] = x
            heapq.heappush(h,x)
    return dict(width=width, work=work, peak=peak, joins=joins, policy=policy, ops=ops,
                retained_extra=extra)


def choose(factors, nodes, scalar, local, deadline, cut_sizes=(0,1,2)):
    adjacency = {v:set() for v in nodes}
    for s,_ in factors:
        for v in s:
            adjacency[v].update(set(s)-{v})
    high = sorted(nodes,key=lambda v:(-len(adjacency[v]),v))[:12]
    counts = []
    for cuts in cut_sizes:
        candidates = ([()] if cuts==0 else [(v,) for v in sorted(nodes)] if cuts==1
                      else list(combinations(high,2)))
        best = None
        admitted = 0
        for pivots in candidates:
            branches = []
            for values in product((False,True),repeat=cuts):
                branch,_ = restrict(factors,scalar,pivots,values)
                plans = [p for pol in POLICIES if (p:=plan(branch,nodes-set(pivots),pol,
                         local["retained_cells"],deadline)) is not None]
                if not plans:
                    break
                p = min(plans,key=lambda x:(x["work"],x["peak"],x["width"]))
                branches.append((values,p))
            if len(branches)!=2**cuts:
                continue
            # Count conditioning table traversal conservatively too.
            work = local["work"]+sum(p["work"]+local["retained_cells"] for _,p in branches)
            peak = max([local["peak"]]+[p["peak"] for _,p in branches])
            width = max(p["width"] for _,p in branches)
            if work > MAX_WORK or peak > MAX_LIVE:
                continue
            admitted += 1
            key = (work,peak,width,pivots)
            if best is None or key<best[0]:
                best = key,branches
        counts.append(dict(cutset_size=cuts,candidates=len(candidates),admitted=admitted))
        if best is not None:
            key,branches = best
            return key,branches,counts
    raise ValueError("NO_BOUNDED_PREJOIN_PLAN")


def interval(x):
    return None if x is None else enclosure(x[0])+enclosure(x[1])


def flips(scope,out):
    bits = [0]*len(out)
    for j,u in enumerate(scope):
        if u in out:
            bits[out.index(u)] = 1<<j
    for j in range(1,len(bits)):
        bits[j] ^= bits[j-1]
    return bits


def table_join(a,b,out,deadline):
    sa,ta=a
    sb,tb=b
    fa,fb=flips(sa,out),flips(sb,out)
    ia=ib=0
    vals=[]
    size=1<<len(out)
    for m in range(size):
        if m%4096==0 and monotonic()>=deadline:
            raise ValueError("JOIN_DEADLINE")
        vals.append(imul(ta[ia],tb[ib]))
        if m+1<size:
            bit=((m+1)&-(m+1)).bit_length()-1
            ia^=fa[bit]
            ib^=fb[bit]
    return vals


def table_sum(bucket,v,out,deadline):
    pivots=[1<<s.index(v) for s,_ in bucket]
    fs=[flips(s,out) for s,_ in bucket]
    indices=[0]*len(bucket)
    vals=[]
    size=1<<len(out)
    for m in range(size):
        if m%4096==0 and monotonic()>=deadline:
            raise ValueError("SUM_DEADLINE")
        pair=[]
        for positive in (False,True):
            x=I_ONE
            for j,(_,t) in enumerate(bucket):
                x=imul(x,t[indices[j]|(pivots[j] if positive else 0)])
                if x is None:
                    break
            pair.append(x)
        vals.append(iadd(*pair))
        if m+1<size:
            bit=((m+1)&-(m+1)).bit_length()-1
            for j in range(len(indices)):
                indices[j]^=fs[j][bit]
    return vals


def execute(factors,scalar,p,deadline):
    tables={i:(s,[interval(x) for x in t]) for i,(s,t) in enumerate(factors)}
    live=sum(len(t) for _,t in tables.values())
    peak=live+p["retained_extra"]
    work=joins=width=0
    for op,ids,dst,out,v in p["ops"]:
        if monotonic()>=deadline:
            raise ValueError("EXECUTE_DEADLINE")
        bucket=[tables[i] for i in ids]
        size=1<<len(out)
        peak=max(peak,live+size+p["retained_extra"])
        if op=="join":
            work+=2*size
            joins+=1
        else:
            work+=2*size*len(bucket)
            width=max(width,len(out))
        if peak>MAX_LIVE or work>MAX_WORK or width>MAX_WIDTH:
            raise ValueError("EXECUTE_RESOURCE")
        vals=(table_join(*bucket,out,deadline) if op=="join" else table_sum(bucket,v,out,deadline))
        for i,(_,t) in zip(ids,bucket):
            del tables[i]
            live-=len(t)
        tables[dst]=(out,vals)
        live+=size
        del bucket
    if (width,work,peak,joins)!=(p["width"],p["work"],p["peak"],p["joins"]):
        raise ValueError("PLAN_REPLAY_DRIFT")
    result=interval(scalar)
    for scope,t in tables.values():
        if scope or len(t)!=1:
            raise ValueError("UNELIMINATED")
        result=imul(result,t[0])
    return result


def solve(raw, expected, seconds=180, profile=False):
    start=monotonic()
    deadline=start+seconds
    plan_deadline=min(deadline,start+PLAN_SECONDS)
    factors,nodes,scalar,local=local_reduce(raw,expected,plan_deadline)
    (work,peak,width,pivots),branches,counts=choose(factors,nodes,scalar,local,plan_deadline)
    report=dict(source_sha256=expected,bits=BITS,local=local,cutset=pivots,search=counts,
                width=width,work=work,peak=peak,
                branches=[dict(values=v,**{k:x for k,x in p.items() if k!="ops"}) for v,p in branches],
                status="STRUCTURAL_PLAN_ONLY" if profile else "PLANNED")
    if not profile:
        result=None
        for values,p in branches:
            f,c=restrict(factors,scalar,pivots,values)
            result=iadd(result,execute(f,c,p,deadline))
        report["sat"]=result is not None
        if result is None:
            result=I_ZERO
        report.update(status="INDEPENDENT_INTERVAL_COMPLETE",
                      real=[str(Q(x,S)) for x in result[:2]],
                      imag=[str(Q(x,S)) for x in result[2:]])
    report["wall_s"]=monotonic()-start
    return report


def residual_components(factors, nodes):
    """Partition primal graph; include isolated variables (free sums)."""
    adjacency = {v:set() for v in nodes}
    for scope,_ in factors:
        for v in scope:
            adjacency[v].update(set(scope)-{v})
    unseen=set(nodes)
    parts=[]
    while unseen:
        start=min(unseen)
        unseen.remove(start)
        comp={start}
        stack=[start]
        while stack:
            for u in sorted(adjacency[stack.pop()] & unseen):
                unseen.remove(u)
                comp.add(u)
                stack.append(u)
        parts.append(comp)
    return adjacency,parts

def residual_profile(raw, expected):
    """Structural-only inspection beyond the global 64-node admission limit.
    All source/local arithmetic and resource caps remain enforced.
    Never execute a contraction or certify a WMC result in this mode.
    """
    start=monotonic()
    factors,nodes,scalar,local=local_reduce(raw,expected,start+PLAN_SECONDS,diagnostic=True)
    adjacency,parts=residual_components(factors,nodes)
    rows=[]
    for comp in sorted(parts,key=lambda x:(-len(x),min(x))):
        owned=[(s,t) for s,t in factors if s and s[0] in comp]
        if any(not set(s)<=comp for s,_ in owned):
            raise ValueError("COMPONENT_OWNERSHIP")
        rows.append(dict(nodes=len(comp),factors=len(owned),
            cells=sum(len(t) for _,t in owned),
            edges=sum(len(adjacency[v]) for v in comp)//2,
            min_degree=min(len(adjacency[v]) for v in comp),
            max_degree=max(len(adjacency[v]) for v in comp),
            within_64=len(comp)<=RESIDUAL_CAP))
    if sum(r["factors"] for r in rows)!=len(factors) or sum(r["nodes"] for r in rows)!=len(nodes):
        raise ValueError("COMPONENT_COVERAGE")
    # All component estimates are sequential; retain source/restricted-table
    # storage in each independent plan and never claim full solve admission.
    plans=[]
    deadline=start+60
    for idx,comp in enumerate(sorted(parts,key=lambda x:(-len(x),min(x)))):
        if idx>=16 or monotonic()>=deadline:
            plans.append(dict(component=idx,status="DIAGNOSTIC_BUDGET"))
            continue
        owned=[(s,t) for s,t in factors if s and s[0] in comp]
        try:
            (work,peak,width,pivots),_,_=choose(owned,comp,ONE,
                dict(work=0,peak=0,retained_cells=local["retained_cells"]),
                min(deadline,monotonic()+10),cut_sizes=(0,))
            plans.append(dict(component=idx,status="FEASIBLE",work=work,peak=peak,width=width))
        except ValueError as exc:
            if str(exc) not in ("PLAN_DEADLINE","NO_BOUNDED_PREJOIN_PLAN"):
                raise
            plans.append(dict(component=idx,status=str(exc)))
    return dict(status="RESIDUAL_PROFILE_ONLY",source_sha256=expected,local=local,
        component_count=len(parts),largest_component=max((len(p) for p in parts),default=0),
        all_components_within_64=all(len(p)<=RESIDUAL_CAP for p in parts),
        scalar_support=scalar is not None,components=rows,zero_cut_plans=plans,
        wall_s=monotonic()-start)

def cutset_spectrum(raw, expected):
    """Bounded independent structural diagnostic only; never a solve verdict."""
    start = monotonic()
    factors,nodes,scalar,local = local_reduce(raw,expected,start+PLAN_SECONDS)
    rows = []
    for n in (0,1,2):
        t = monotonic()
        try:
            (work,peak,width,pivots),branches,counts = choose(
                factors,nodes,scalar,local,t+PLAN_SECONDS,cut_sizes=(n,))
            rows.append(dict(cutset_size=n,status="FEASIBLE",work=work,peak=peak,
                width=width,pivots=pivots,search=counts,
                joins=sum(p["joins"] for _,p in branches),wall_s=monotonic()-t))
        except ValueError as exc:
            reason = str(exc)
            if reason not in ("PLAN_DEADLINE","NO_BOUNDED_PREJOIN_PLAN"):
                raise
            rows.append(dict(cutset_size=n,status="DEADLINE" if reason=="PLAN_DEADLINE" else "NO_FEASIBLE_PLAN",
                reason=reason,wall_s=monotonic()-t))
    first = next((x for x in rows if x["status"]=="FEASIBLE"),None)
    best = min((x for x in rows if x["status"]=="FEASIBLE"),
        key=lambda x:(x["work"],x["peak"],x["width"],x["cutset_size"]),default=None)
    return dict(status="STRUCTURAL_SPECTRUM_ONLY",source_sha256=expected,
        local=local,rows=rows,first_feasible=first,best_observed=best,
        all_sizes_complete=all(x["status"]!="DEADLINE" for x in rows),
        work_ratio=None if first is None or best is None or not best["work"] else first["work"]/best["work"],
        wall_s=monotonic()-start)

def self_test():
    import random
    from original_fixed_interval import solve as baseline
    rng=random.Random(21)
    for _ in range(32):
        n=5
        cs=[tuple(rng.choice((-1,1))*v for v in rng.sample(range(1,n+1),3)) for _ in range(5)]
        w={v:(Q(rng.randint(-3,3),10),Q(rng.randint(-3,3),10)) for v in range(-n,n+1) if v}
        raw=(f"p cnf {n} {len(cs)}\n"+''.join(f"c p weight {v} {float(a):.1f} {float(b):.1f} 0\n" for v,(a,b) in w.items())+
             ''.join(' '.join(map(str,c))+' 0\n' for c in cs)).encode()
        h=sha256(raw).hexdigest()
        got=solve(raw,h)
        exact=None
        for m in range(1<<n):
            yes=lambda l:bool(m&(1<<(abs(l)-1)))==(l>0)
            if all(any(yes(l) for l in c) for c in cs):
                x=ONE
                for v in range(1,n+1):
                    x=mul(x,w[v if yes(v) else -v])
                exact=add(exact,x)
        assert got["sat"]==(exact is not None)
        x=exact if exact is not None else ZERO
        for k,a in zip(("real","imag"),x):
            lo,hi=map(Q,got[k])
            assert lo<=a<=hi
        assert baseline(raw,h)["sat"]==got["sat"]
    # Exercise nontrivial interval join and sum directly, with unsupported
    # and supported zero entries. The two branch projections are exact.
    a=((1,2),[ONE,None,ZERO,(Q(-2),Q(3))])
    b=((1,3),[ONE,(Q(2),Q(-1)),None,ONE])
    d=monotonic()+5
    j=table_join((a[0],list(map(interval,a[1]))),(b[0],list(map(interval,b[1]))),(1,2,3),d)
    x=table_sum([((1,2,3),j)],1,(2,3),d)
    expected=[]
    for m in range(4):
        expected.append(interval(add(mul(a[1][(m&1)<<1],b[1][m&2]),
                                     mul(a[1][((m&1)<<1)|1],b[1][(m&2)|1]))))
    assert x==expected
    for raw,sat,answer in [(b"p cnf 1 2\n1 0\n-1 0\n",False,0),
                           (b"p cnf 1 0\nc p weight 1 -1 0 0\n",True,0),
                           (b"p cnf 3 0\n",True,8)]:
        r=solve(raw,sha256(raw).hexdigest())
        assert r["sat"]==sat and all(Q(a)==answer for a in r["real"])
    raw=b"p cnf 6 2\n1 2 3 4 5 6 0\n-1 -2 -3 -4 -5 -6 0\n"
    h=sha256(raw).hexdigest()
    ordinary=solve(raw,h,profile=True)
    spectrum=cutset_spectrum(raw,h)
    assert spectrum["status"]=="STRUCTURAL_SPECTRUM_ONLY"
    assert spectrum["all_sizes_complete"]
    first=spectrum["first_feasible"]
    assert (first["work"],first["peak"],first["width"])==(ordinary["work"],ordinary["peak"],ordinary["width"])
    assert spectrum["best_observed"]["work"]<=first["work"]
    assert [r["cutset_size"] for r in spectrum["rows"]]==[0,1,2]
    graph,parts=residual_components([((1,2),[ONE]*4),((3,4),[ONE]*4)],{1,2,3,4,5})
    assert parts==[{1,2},{3,4},{5}]
    assert graph[5]==set()
    profile=residual_profile(raw,h)
    assert profile["status"]=="RESIDUAL_PROFILE_ONLY"
    assert sum(c["nodes"] for c in profile["components"])==profile["local"]["residual"]
    print("EXACT_PREJOIN_SELF_TEST PASS")


if __name__=="__main__":
    ap=argparse.ArgumentParser()
    ap.add_argument("cnf",type=Path,nargs="?")
    ap.add_argument("sha256",nargs="?")
    ap.add_argument("--self-test",action="store_true")
    mode=ap.add_mutually_exclusive_group()
    mode.add_argument("--profile",action="store_true")
    mode.add_argument("--cutset-spectrum",action="store_true")
    mode.add_argument("--residual-profile",action="store_true")
    a=ap.parse_args()
    if a.self_test:
        self_test()
    else:
        if a.cnf is None or a.sha256 is None:
            ap.error("CNF and SHA256 required")
        raw=a.cnf.read_bytes()
        result=(residual_profile(raw,a.sha256) if a.residual_profile else
                cutset_spectrum(raw,a.sha256) if a.cutset_spectrum else solve(raw,a.sha256,profile=a.profile))
        print(json.dumps(result,indent=2))
