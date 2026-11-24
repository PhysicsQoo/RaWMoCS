"""Independent original-CNF fixed-point interval oracle; no gate extraction or SCC."""
from fractions import Fraction as Q
from hashlib import sha256
from itertools import combinations
from time import monotonic
import json
import sys

BITS=550
S=1<<BITS
ONE=(S,S,0,0)
ZERO=(0,0,0,0)
MAX_WIDTH=19
MAX_CLAUSE_WIDTH=9
MAX_WORK=40_000_000
MAX_LIVE=2_000_000

def enclosure(q):
    q=Q(q)*S
    return q.numerator//q.denominator,-((-q.numerator)//q.denominator)

def mul_interval(a,b,c,d):
    if a==b==0 or c==d==0:return (0,0)
    if a>=0:
        if c>=0:lo,hi=a*c,b*d
        elif d<=0:lo,hi=b*c,a*d
        else:lo,hi=b*c,b*d
    elif b<=0:
        if c>=0:lo,hi=a*d,b*c
        elif d<=0:lo,hi=b*d,a*c
        else:lo,hi=a*d,a*c
    else:
        if c>=0:lo,hi=a*d,b*d
        elif d<=0:lo,hi=b*c,a*c
        else:lo,hi=min(a*d,b*c),max(a*c,b*d)
    return lo>>BITS,-((-hi)>>BITS)

def mul(x,y):
    if x is None or y is None:return None
    if x==ONE:return y
    if y==ONE:return x
    if x==ZERO or y==ZERO:return ZERO
    a,b,c,d=x;e,f,g,h=y
    ac=mul_interval(a,b,e,f);bd=mul_interval(c,d,g,h)
    ad=mul_interval(a,b,g,h);bc=mul_interval(c,d,e,f)
    return (ac[0]-bd[1],ac[1]-bd[0],ad[0]+bc[0],ad[1]+bc[1])

def add(x,y):
    if x is None:return y
    if y is None:return x
    return tuple(a+b for a,b in zip(x,y))

def solve(raw,expected_sha,deadline_seconds=180):
    if sha256(raw).hexdigest()!=expected_sha:raise ValueError('SOURCE_DRIFT')
    start=monotonic();weights={};clauses=[];n=nc=None
    for line in raw.decode().splitlines():
        p=line.split()
        if not p:continue
        if p[:2]==['p','cnf']:
            if n is not None:raise ValueError('DUPLICATE_HEADER')
            n,nc=map(int,p[2:]);continue
        if p[:3]==['c','p','weight']:
            if len(p)!=7 or p[6]!='0':raise ValueError('WEIGHT_FORMAT')
            v=int(p[3])
            if v in weights or not 1<=abs(v)<=n:raise ValueError('WEIGHT_OWNERSHIP')
            weights[v]=enclosure(p[4])+enclosure(p[5]);continue
        if p[0]=='c':continue
        if p[-1]!='0':raise ValueError('CLAUSE_FORMAT')
        c=tuple(map(int,p[:-1]))
        if any(not 1<=abs(v)<=n for v in c):raise ValueError('CLAUSE_LITERAL')
        clauses.append(c)
    if n is None or len(clauses)!=nc:raise ValueError('CLAUSE_COUNT')
    if n>2048 or nc>10000:raise ValueError('SOURCE_CAP')
    # Bound the actual initial factor cells before allocating any tables.
    # One nine-literal original W-state clause is within the existing live budget.
    clause_scopes=[tuple(sorted(set(map(abs,c)))) for c in clauses]
    if any(len(scope)>MAX_CLAUSE_WIDTH for scope in clause_scopes):raise ValueError('CLAUSE_WIDTH')
    initial_cells=2*n+sum(1<<len(scope) for scope in clause_scopes)
    if initial_cells>MAX_LIVE:raise ValueError('INITIAL_RESOURCE_CAP')
    fs=[];adj={v:set() for v in range(1,n+1)}
    for c,scope in zip(clauses,clause_scopes):
        vals=[ONE if any(bool(m&(1<<scope.index(abs(l))))==(l>0) for l in c) else None for m in range(1<<len(scope))]
        fs.append((scope,vals))
        for a,b in combinations(scope,2):adj[a].add(b);adj[b].add(a)
    for v in adj:fs.append(((v,),[weights.get(-v,ONE),weights.get(v,ONE)]))
    g={v:set(s) for v,s in adj.items()};scopes=[s for s,_ in fs];order=[];width=work=0;live=sum(1<<len(s) for s in scopes);peak=live
    while g:
        if monotonic()-start>deadline_seconds:raise ValueError('PLAN_DEADLINE')
        v=min(g,key=lambda v:(sum(b not in g[a] for a,b in combinations(g[v],2)),len(g[v]),-v))
        nb=g.pop(v);width=max(width,len(nb))
        if len(nb)>MAX_WIDTH:raise ValueError('WIDTH_CAP')
        bucket=[s for s in scopes if v in s];scopes=[s for s in scopes if v not in s];size=1<<len(nb)
        work+=2*size*len(bucket);peak=max(peak,live+size)
        if work>MAX_WORK or peak>MAX_LIVE:raise ValueError('RESOURCE_CAP')
        live+=size-sum(1<<len(s) for s in bucket);scopes.append(tuple(sorted(nb)));order.append(v)
        for u in nb:g[u].discard(v);g[u].update(nb-{u})
    for v in order:
        if monotonic()-start>deadline_seconds:raise ValueError('SOLVE_DEADLINE')
        nb=adj.pop(v);out=tuple(sorted(nb));bucket=[f for f in fs if v in f[0]];fs=[f for f in fs if v not in f[0]]
        bucket.sort(key=lambda f:(not any(x is None for x in f[1]),len(f[0])))
        tables=[t for _,t in bucket];pivots=[];flips=[]
        for scope,_ in bucket:
            pivots.append(1<<scope.index(v));bits=[0]*len(out)
            for j,u in enumerate(scope):
                if u!=v:bits[out.index(u)]=1<<j
            for j in range(1,len(bits)):bits[j]^=bits[j-1]
            flips.append(bits)
        indices=[0]*len(bucket);vals=[];size=1<<len(out)
        for m in range(size):
            if m%4096==0 and monotonic()-start>deadline_seconds:raise ValueError('SOLVE_DEADLINE')
            pair=[]
            for positive in (False,True):
                prod=ONE
                for t,i,pivot in zip(tables,indices,pivots):
                    x=t[i|(pivot if positive else 0)]
                    if x is None:prod=None;break
                    prod=mul(prod,x)
                pair.append(prod)
            vals.append(add(*pair))
            if m+1<size:
                bit=((m+1)&-(m+1)).bit_length()-1
                for i,flip in enumerate(flips):indices[i]^=flip[bit]
        fs.append((out,vals))
        for u in nb:adj[u].discard(v);adj[u].update(nb-{u})
    result=ONE
    for scope,vals in fs:
        if scope or len(vals)!=1:raise ValueError('UNELIMINATED')
        result=mul(result,vals[0])
    sat=result is not None
    if result is None:result=ZERO
    return dict(source_sha256=expected_sha,bits=BITS,variables=n,clauses=nc,width=width,work=work,peak=peak,sat=sat,real=[str(Q(x,S)) for x in result[:2]],imag=[str(Q(x,S)) for x in result[2:]],wall_s=monotonic()-start)

def compare(row,ref,digits):
    if row['status']!='COMPLETED' or digits not in (30,100):raise ValueError('INVALID_WITNESS_COMPARISON')
    r=list(map(Q,ref['real']));i=list(map(Q,ref['imag']));a,b=Q(row['real']),Q(row['imag'])
    scale=max(Q(1),abs(a),abs(b),min(abs(r[0]),abs(r[1])) if r[0]*r[1]>0 else Q(0),min(abs(i[0]),abs(i[1])) if i[0]*i[1]>0 else Q(0))
    upper=max(abs(a-r[0]),abs(a-r[1]),abs(b-i[0]),abs(b-i[1]))/scale
    return {'pass':row['sat']==ref['sat'] and upper<=Q(1,10**(digits-6)), 'relative_upper_bound':str(upper),'guard':str(Q(1,10**(digits-6)))}

def self_test():
    import random
    rng=random.Random(21)
    for _ in range(1000):
        a,b=sorted(rng.randint(-100,100) for _ in range(2));c,d=sorted(rng.randint(-100,100) for _ in range(2))
        lo,hi=mul_interval(a,b,c,d);p=[a*c,a*d,b*c,b*d]
        assert lo*S<=min(p) and hi*S>=max(p)
    # Exercise actual rounding boundaries, not only sub-grid products.
    for _ in range(2000):
        a,b=sorted(rng.randrange(-3*S,3*S) for _ in range(2))
        c,d=sorted(rng.randrange(-3*S,3*S) for _ in range(2))
        lo,hi=mul_interval(a,b,c,d)
        p=[a*c,a*d,b*c,b*d]
        assert lo==min(p)//S and hi==-((-max(p))//S)
    for _ in range(40):
        n=5;clauses=[tuple(rng.choice((-1,1))*v for v in rng.sample(range(1,n+1),3)) for _ in range(5)]
        weights={v:(Q(rng.randint(-3,3),10),Q(rng.randint(-3,3),10)) for v in list(range(1,n+1))+list(range(-n,0))}
        raw=(f'p cnf {n} {len(clauses)}\nc t wmc\n'+''.join(f'c p weight {v} {float(a):.1f} {float(b):.1f} 0\n' for v,(a,b) in weights.items())+''.join(' '.join(map(str,c))+' 0\n' for c in clauses)).encode()
        ref=solve(raw,sha256(raw).hexdigest());sat=False;re=im=Q(0)
        for m in range(1<<n):
            truth=lambda l:bool(m&(1<<(abs(l)-1)))==(l>0)
            if not all(any(truth(l) for l in c) for c in clauses):continue
            sat=True;a,b=Q(1),Q(0)
            for v in range(1,n+1):
                c,d=weights[v if truth(v) else -v];a,b=a*c-b*d,a*d+b*c
            re+=a;im+=b
        assert ref['sat']==sat and Q(ref['real'][0])<=re<=Q(ref['real'][1]) and Q(ref['imag'][0])<=im<=Q(ref['imag'][1])
    # Original wide-clause factor: independent exact enumeration, signed complex
    # weights, an explicit zero weight and both satisfiable/contradictory controls.
    for extra in ([],[(-1,), (1,)]):
        n=9;clauses=[tuple(range(1,10)),(-1,2),(-3,4)]+extra
        weights={v:(Q((v%5)-2,10),Q((v%3)-1,10)) for v in range(-9,10) if v}
        weights[1]=(Q(0),Q(0))
        raw=(f'p cnf {n} {len(clauses)}\nc t wmc\n'+''.join(f'c p weight {v} {float(a):.1f} {float(b):.1f} 0\n' for v,(a,b) in weights.items())+''.join(' '.join(map(str,c))+' 0\n' for c in clauses)).encode()
        ref=solve(raw,sha256(raw).hexdigest());sat=False;re=im=Q(0)
        for m in range(1<<n):
            truth=lambda l:bool(m&(1<<(abs(l)-1)))==(l>0)
            if not all(any(truth(l) for l in c) for c in clauses):continue
            sat=True;a,b=Q(1),Q(0)
            for v in range(1,n+1):
                c,d=weights[v if truth(v) else -v];a,b=a*c-b*d,a*d+b*c
            re+=a;im+=b
        assert ref['sat']==sat and Q(ref['real'][0])<=re<=Q(ref['real'][1]) and Q(ref['imag'][0])<=im<=Q(ref['imag'][1])
    for raw,reason in ((b'p cnf 10 1\n1 2 3 4 5 6 7 8 9 10 0\n','CLAUSE_WIDTH'),
                       (b'p cnf 9 4000\n'+b'1 2 3 4 5 6 7 8 9 0\n'*4000,'INITIAL_RESOURCE_CAP')):
        try:
            solve(raw,sha256(raw).hexdigest())
        except ValueError as exc:
            assert str(exc)==reason
        else:
            raise AssertionError('invalid initial factor budget admitted')
    print('FIXED_INTERVAL_SELF_TEST PASS',file=sys.stderr,flush=True)

if __name__=='__main__':
    self_test()
    if len(sys.argv)==3:
        raw=open(sys.argv[1],'rb').read()
        print(json.dumps(solve(raw,sys.argv[2])))
    elif len(sys.argv)!=1:
        raise SystemExit('usage: original_fixed_interval.py [CNF SHA256]')
