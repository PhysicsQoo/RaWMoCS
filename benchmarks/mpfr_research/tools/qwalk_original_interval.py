"""Independent original-CNF interval witness for cancellation-sensitive Qwalk.
No private definitions, SCC substitutions, source rewrites or Rust factor code.
The finite full-source interval is an accuracy oracle, not solver input.
"""
from fractions import Fraction as Q
from hashlib import sha256
import itertools
import time

SOURCE_SHA="adde521cb9182d603d08eb7a07991f4fdae75ba3ee7635da9e6a606a7d9b945e"

def solve(raw, bits=550):
    import mpmath as mp
    import numpy as np
    start=time.monotonic()
    if sha256(raw).hexdigest()!=SOURCE_SHA or bits!=550:
        raise ValueError("QWALK_SOURCE_OR_PRECISION_DRIFT")
    mp.iv.prec=bits
    lines=raw.decode().splitlines()
    n,nc=map(int,lines[0].split()[2:]); weights={}; clauses=[]
    assert lines[0].split()[:2]==["p","cnf"] and lines[1]=="c t wmc"
    one=mp.iv.mpc(1)
    for line in lines[2:]:
        p=line.split()
        if not p: continue
        if p[:3]==["c","p","weight"]:
            a,b=Q(p[4]),Q(p[5])
            assert len(p)==7 and p[6]=="0" and int(p[3]) not in weights
            weights[int(p[3])]=mp.iv.mpc(mp.iv.mpf(a.numerator)/a.denominator,mp.iv.mpf(b.numerator)/b.denominator)
        else:
            assert p[-1]=="0"
            clauses.append(tuple(map(int,p[:-1])))
    assert len(clauses)==nc
    fs=[]; adj={v:set() for v in range(1,n+1)}
    for c in clauses:
        scope=tuple(sorted(set(map(abs,c))))
        assert len(scope)<=10
        vals=[one if any(bool(x[scope.index(abs(l))])==(l>0) for l in c) else None
              for x in itertools.product((0,1),repeat=len(scope))]
        fs.append((scope,np.array(vals,dtype=object).reshape((2,)*len(scope))))
        for a,b in itertools.combinations(scope,2):
            adj[a].add(b);adj[b].add(a)
    for v in adj:
        fs.append(((v,),np.array([weights.get(-v,one),weights.get(v,one)],dtype=object)))
    def mul(a,b):
        sa,x=a;sb,y=b;s=tuple(sorted(set(sa)|set(sb)))
        assert len(s)<=16
        x=np.broadcast_to(x.reshape(tuple(2 if v in sa else 1 for v in s)),(2,)*len(s))
        y=np.broadcast_to(y.reshape(tuple(2 if v in sb else 1 for v in s)),(2,)*len(s))
        z=np.empty(x.shape,dtype=object)
        for k in np.ndindex(z.shape):
            u,v=x[k],y[k]
            z[k]=None if u is None or v is None else v if u is one else u if v is one else u*v
        return s,z
    width=work=0
    while adj:
        if time.monotonic()-start>90:
            raise ValueError("QWALK_WITNESS_DEADLINE")
        def key(v):
            s=adj[v]
            return (sum(b not in adj[a] for a,b in itertools.combinations(s,2)),len(s),-v)
        v=min(adj,key=key);nb=adj.pop(v);width=max(width,len(nb));work+=2**(len(nb)+1)
        assert work<=1_000_000
        bucket=[f for f in fs if v in f[0]]
        fs=[f for f in fs if v not in f[0]]
        bucket.sort(key=lambda f:len(f[0]));a=bucket.pop(0)
        for b in bucket:a=mul(a,b)
        s,x=a;axis=s.index(v);out=tuple(u for u in s if u!=v)
        a0=np.take(x,0,axis=axis);a1=np.take(x,1,axis=axis)
        vals=np.empty((2,)*len(out),dtype=object)
        for k in np.ndindex(vals.shape):
            u=a0[k] if isinstance(a0,np.ndarray) else a0
            v1=a1[k] if isinstance(a1,np.ndarray) else a1
            vals[k]=v1 if u is None else u if v1 is None else u+v1
        fs.append((out,vals))
        for u in nb:adj[u].discard(v);adj[u].update(nb-{u})
    a=fs.pop(0)
    for b in fs:a=mul(a,b)
    assert not a[0]
    res=a[1].item()
    if res is None:
        raise ValueError("UNEXPECTED_QWALK_UNSAT")
    def exact(t):
        sign,man,exp,_=t
        return Q((-1 if sign else 1)*man)*Q(2)**exp
    return dict(source_sha256=SOURCE_SHA,bits=bits,variables=n,clauses=nc,
                width=width,work=work,sat=True,real=[str(exact(t)) for t in res.real._mpi_],
                imag=[str(exact(t)) for t in res.imag._mpi_],wall_s=time.monotonic()-start)

def compare(row, witness, target_digits):
    if witness["source_sha256"]!=SOURCE_SHA or target_digits not in (30,100):
        raise ValueError("WITNESS_CONTRACT_DRIFT")
    if row["status"]!="COMPLETED":
        raise ValueError("INCOMPLETE_ARM")
    r=list(map(Q,witness["real"]));i=list(map(Q,witness["imag"]))
    a,b=Q(row["real"]),Q(row["imag"])
    lo=max(Q(1),abs(a),abs(b),min(abs(r[0]),abs(r[1])) if r[0]*r[1]>0 else Q(0),
           min(abs(i[0]),abs(i[1])) if i[0]*i[1]>0 else Q(0))
    hi=max(Q(1),abs(a),abs(b),*map(abs,r),*map(abs,i))
    upper=max(abs(a-r[0]),abs(a-r[1]),abs(b-i[0]),abs(b-i[1]))/lo
    lower=max(Q(0),r[0]-a,a-r[1],i[0]-b,b-i[1])/hi
    guard=Q(1,10**(target_digits-6))
    passed=row["sat"]==witness["sat"] and upper<=guard
    return {"pass":passed,"sat_match":row["sat"]==witness["sat"],
            "relative_upper_bound":str(upper),"relative_lower_bound":str(lower),
            "relative_guard":str(guard),"status":"PASS" if passed else "FAIL_OR_INCONCLUSIVE"}
