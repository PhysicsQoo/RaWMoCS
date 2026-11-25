#!/usr/bin/env python3
"""Bounded official MCC2024 Track 2 real-WMC admission and discovery.

The original archive is checked against the publisher's MD5. Original
uncompressed source bytes and common solver representation are separately
hashed. No quantum-specific transformations or fabricated source weights.
"""
import argparse
import bz2
from fractions import Fraction
import gzip
import hashlib
import io
import json
import lzma
from pathlib import Path
import re
import tarfile

from quprs_long_tail_paired import (AS_BYTES, CNF_MAX_BYTES, GPMC_SHA,
    PAIR_S, git_sha, native, complex_relative_witness, sha)

DOI="10.5281/zenodo.14249068"
ARCHIVE_MD5="1c7e6279eaf29bf3731cebe07eedf107"
ARCHIVE_MAX=100_000_000
MAX_CASES=8
SHARDS=4
PRECISIONS=((30,60,200),(100,130,432))
DECIMAL=re.compile(r"[+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]{1,3})?")

def normalize(raw):
    """Strict subset of the published nonprojected literal-real WMC format.

    Only paired explicit decimal weights (or neither, implicit unit) are
    admitted. Reject fractions, negative weights, one-sided weights,
    projections, global factors and unrecognized problem-specific records.
    Preserve exact decimal tokens and clause order; add zero imaginary
    component only, never evaluate weights via a binary float.
    """
    header=None
    weights={}
    clauses=[]
    origins=[]
    types=[]
    for line in raw.decode("ascii").splitlines():
        t=line.split()
        if not t:continue
        if t[0]=="c":
            if t[:2]==["c","r"]:origins.append(" ".join(t[2:])[:1000])
            elif t[:2]==["c","t"]:types.append(t[2:])
            elif t[:2]==["c","p"]:
                if len(t)!=6 or t[2]!="weight" or t[-1]!="0":
                    raise ValueError("UNSUPPORTED_PROBLEM_RECORD")
                lit=int(t[3]);w=t[4]
                if not lit or lit in weights or len(w)>128 or not DECIMAL.fullmatch(w):
                    raise ValueError("UNSUPPORTED_WEIGHT")
                value=Fraction(w)
                if not 0<=value<=1:raise ValueError("WEIGHT_RANGE")
                weights[lit]=w
            continue
        if t[0]=="p":
            if header is not None or len(t)!=4 or t[1]!="cnf":
                raise ValueError("INVALID_HEADER")
            n,m=map(int,t[2:])
            if n<0 or m<0:raise ValueError("INVALID_HEADER")
            header=n,m
            continue
        if header is None:raise ValueError("CLAUSE_BEFORE_HEADER")
        clause=list(map(int,t))
        if not clause or clause[-1]!=0 or any(x==0 or abs(x)>header[0] for x in clause[:-1]):
            raise ValueError("INVALID_CLAUSE")
        clauses.append(clause)
    if header is None or len(clauses)!=header[1] or len(types)>1 or any(t!=["wmc"] for t in types):
        raise ValueError("INVALID_WMC_TYPE_OR_COUNT")
    if not weights or any(abs(l)>header[0] or -l not in weights for l in weights):
        raise ValueError("MISSING_OR_UNPAIRED_WEIGHTS")
    out=[f"p cnf {header[0]} {header[1]}","c t wmc"]
    out.extend(f"c p weight {l} {w} 0 0" for l,w in weights.items())
    out.extend(" ".join(map(str,c)) for c in clauses)
    data=("\n".join(out)+"\n").encode("ascii")
    if len(data)>CNF_MAX_BYTES:raise ValueError("NORMALIZED_SIZE_CAP")
    return data,dict(variables=header[0],clauses=header[1],weighted_literals=len(weights),
                     origins=origins,normalization="exact decimal real + imaginary zero; implicit absent pair unit")

def member_bytes(tar,member):
    if not member.isfile() or member.size>CNF_MAX_BYTES:raise ValueError("ARCHIVE_MEMBER_CAP")
    raw=tar.extractfile(member).read(CNF_MAX_BYTES+1)
    if len(raw)>CNF_MAX_BYTES:raise ValueError("ARCHIVE_MEMBER_CAP")
    streams={".gz":gzip.GzipFile,".xz":lzma.LZMAFile,".bz2":bz2.BZ2File}
    suffix=Path(member.name).suffix
    if suffix in streams:
        with streams[suffix](io.BytesIO(raw)) as f:raw=f.read(CNF_MAX_BYTES+1)
    if len(raw)>CNF_MAX_BYTES:raise ValueError("UNCOMPRESSED_CAP")
    return raw

def gpmc_cmd(a,cnf,digits):
    return [str(a.gpmc.resolve()),"-mode=1",f"-prec={digits}",str(cnf.resolve())]

def rust_cmd(a,cnf,digits,route):
    return [str(a.mpfr.resolve()),route,str(digits),str(cnf.resolve())]

def self_test(a):
    # Synthetic checks test the adapter and both native CLIs only. They
    # are explicitly excluded from the public-source cohort.
    fixtures=[
        (b"p cnf 2 1\nc t wmc\nc p weight 1 .25 0\nc p weight -1 .75 0\n1 0\n",True,Fraction(1,2)),
        (b"p cnf 1 1\nc t wmc\nc p weight 1 0 0\nc p weight -1 1 0\n1 0\n",True,Fraction(0)),
        (b"p cnf 1 2\nc t wmc\nc p weight 1 0.5 0\nc p weight -1 0.5 0\n1 0\n-1 0\n",False,Fraction(0))]
    for i,(raw,sat,answer) in enumerate(fixtures):
        data,_=normalize(raw)
        file=a.out/f"adapter_control_{i}.cnf";file.write_bytes(data)
        for target,digits,bits in PRECISIONS:
            for arm,cmd in [("gpmc",gpmc_cmd(a,file,digits)),
                            ("candidate",rust_cmd(a,file,digits,"--factor-prejoin"))]:
                row=native(cmd,arm,10,digits,expected_bits=bits)
                assert row["status"]=="COMPLETED" and row["sat"]==sat,row
                assert abs(Fraction(row["real"])-answer)<=Fraction(1,10**target),row
                assert Fraction(row["imag"])==0,row
    raw=fixtures[0][0]
    for bad in (raw.replace(b"c p weight -1 .75 0\n",b""),raw.replace(b".25",b"1/4"),
                raw.replace(b"c t wmc",b"c t pwmc"),raw.replace(b".25",b"-.25")):
        try:normalize(bad)
        except ValueError:pass
        else:raise AssertionError("ADAPTER_ACCEPTED_UNSUPPORTED_FORMAT")
    print("MCC_ADAPTER_NATIVE_CONTROL_PASS",flush=True)

def main():
    p=argparse.ArgumentParser()
    for arg in ("archive","gpmc-source","gpmc","mpfr","out"):
        p.add_argument("--"+arg,type=Path,required=True)
    p.add_argument("--shard",type=int,choices=range(SHARDS),required=True)
    a=p.parse_args()
    if git_sha(a.gpmc_source)!=GPMC_SHA:raise ValueError("GPMC_SOURCE_DRIFT")
    if a.archive.stat().st_size>ARCHIVE_MAX:raise ValueError("ARCHIVE_CAP")
    archive=a.archive.read_bytes()
    if hashlib.md5(archive).hexdigest()!=ARCHIVE_MD5:raise ValueError("ARCHIVE_MD5_DRIFT")
    a.out.mkdir(parents=True,exist_ok=True)
    self_test(a)
    report=dict(status="IN_PROGRESS",doi=DOI,archive_md5=ARCHIVE_MD5,
        archive_sha256=hashlib.sha256(archive).hexdigest(),gpmc_source=GPMC_SHA,
        gpmc_binary_sha256=sha(a.gpmc),rust_binary_sha256=sha(a.mpfr),
        shard=a.shard,limits=dict(seconds=PAIR_S,address_bytes=AS_BYTES),
        evidence="exploratory same-input real-WMC only; no independent large-instance reference",cases=[])
    def save():
        (a.out/"report.json").write_text(json.dumps(report,indent=2)+"\n")
    with tarfile.open(a.archive,"r:*") as tar:
        members=sorted((m for m in tar.getmembers() if m.isfile() and
            re.search(r"\.cnf(?:\.(?:gz|xz|bz2))?$",m.name)),key=lambda m:m.name)
        if len(members)<MAX_CASES or len({m.name for m in members})!=len(members):
            raise ValueError("ARCHIVE_MANIFEST")
        selected=[members[i*(len(members)-1)//(MAX_CASES-1)] for i in range(MAX_CASES)]
        report["archive_member_count"]=len(members)
        report["manifest"]=[m.name for m in selected];save()
        for i in range(a.shard,MAX_CASES,SHARDS):
            member=selected[i];case=f"mcc_track2_{i:02d}"
            row=dict(case=case,member=member.name,status="IN_PROGRESS",screen=[],pilots=[])
            report["cases"].append(row);save()
            try:
                raw=member_bytes(tar,member)
                row["original_sha256"]=hashlib.sha256(raw).hexdigest()
                (a.out/f"{case}.original.cnf").write_bytes(raw)
                data,meta=normalize(raw)
            except (ValueError,UnicodeError,OSError,EOFError,lzma.LZMAError) as exc:
                row.update(status="SOURCE_NOT_ADMITTED",reason=str(exc)[:300]);save()
                print("MCC_SOURCE_REJECT",case,member.name,row["reason"],flush=True);continue
            cnf=a.out/f"{case}.common.cnf";cnf.write_bytes(data)
            row.update(meta,common_sha256=sha(cnf))
            print("MCC_SOURCE",case,member.name,row["original_sha256"],row["common_sha256"],meta,flush=True)
            for target,digits,bits in PRECISIONS:
                assert sha(cnf)==row["common_sha256"]
                result=native(gpmc_cmd(a,cnf,digits),"gpmc",PAIR_S,digits,expected_bits=bits)
                result.update(target=target,digits=digits,bits=bits);row["screen"].append(result);save()
                print("MCC_GPMC_SCREEN",case,bits,result["status"],result.get("wall_ns"),flush=True)
                if result["status"]!="COMPLETED":break
            eligible=len(row["screen"])==2 and all(x["status"]=="COMPLETED" and x["wall_ns"]>=10**10 for x in row["screen"])
            row["status"]="GPMC_LONG_QUALIFIED" if eligible else "GPMC_NOT_LONG_OR_CENSORED";save()
            if not eligible:continue
            # Use unchanged GPMC screen result as exploratory correctness
            # comparison only, not a randomized timing or speed acceptance.
            for g in row["screen"]:
                for route in ("--adaptive","--factor-prejoin"):
                    assert sha(cnf)==row["common_sha256"]
                    r=native(rust_cmd(a,cnf,g["digits"],route),"candidate",PAIR_S,g["digits"],expected_bits=g["bits"])
                    pilot=dict(route=route,bits=g["bits"],rust=r,status="CENSORED")
                    if r["status"]=="COMPLETED":
                        witness=complex_relative_witness(g,r,g["target"])
                        pilot.update(witness=witness,status="EXPLORATORY_AGREEMENT" if witness["pass"] and g["sat"]==r["sat"] else "NUMERIC_INCONCLUSIVE")
                    row["pilots"].append(pilot);save()
                    print("MCC_RUST_PILOT",case,g["bits"],route,pilot["status"],r.get("wall_ns"),r.get("stderr_tail","")[-500:],flush=True)
    report["status"]="SCOUT_COMPLETE";save()
    print("MCC_SCOUT_FINAL",a.shard,[(r["case"],r["status"]) for r in report["cases"]],flush=True)

if __name__=="__main__":main()
