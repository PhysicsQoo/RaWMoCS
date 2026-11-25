"""Bounded original QPE8 or GF2^6 Simulation proxies, not long-case timing claims."""
import argparse
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys

from original_fixed_interval import compare
from quprs_long_tail_paired import AS_BYTES, CNF_MAX_BYTES, EXPORT_S, GPMC_SHA, QU_PRS_SHA, git_sha, native, parse_out, sha
from quprs_prejoin_native_pilot import TRACE

ROOT=Path(".research/codspeed-prejoin")
CONFIG=Path("codspeed-prejoin.generated.yml")
REL="benchmarks/MQTBench/Random_Rotation/qpeexact_nativegates_ibm_qiskit_opt0_8.qasm"
BLOB="01078415cf504c48fa770c5eea3fa9ccdd22fecd"
# Historical QPE16 preflight source, independently rehashed from artifact.
QPE16_CNF_SHA256="6385f6e460c2cf56e40094d2df99815f68deac55d6846e93d5ef627c66822696"
# First qualified execution freezes the CNF hash; later commits must match it.
EXPECTED="00f15ebed71b50746e11ececde9915ec19a46af1fac203d23e27877d814b14f9"
CASE="extended_qpe8_prejoin_focus"
ARMS=("gpmc","adaptive","prejoin")
DIGITS,BITS,TARGET=60,200,30
LIMIT,REFERENCE_LIMIT=1,45
GF2=os.environ.get("STUDY")=="gf2_6_prejoin"
BENCHMARK="qpe8_prejoin"
if GF2:
    REL="benchmarks/Feynman/gf2^6_mult.qasm"
    # Pinned QuPRS revision and prior independent native CNF hash.
    BLOB=None
    EXPECTED="2b8001227f544bc21a9590f21086747d094d647de04d93c48c27982bc9ba4a2d"
    CASE="extended_gf2_6_prejoin_holdout"
    ARMS=("prejoin",)
    # Shorter qualified precision to avoid simulation slowdown crossing native deadlines.
    DIGITS,BITS,TARGET=30,100,30
    LIMIT,REFERENCE_LIMIT=1,45
    BENCHMARK="gf2_6_prejoin"
PREC=re.compile(r"^RESEARCH_MPFR_PHASE_NS .* digits=(\d+) bits=(\d+)$",re.M)
ADAPTIVE=re.compile(r"^ADAPTIVE_CONTRACTION width=(\d+) work=(\d+) peak=(\d+) planned_width=(\d+) planned_work=(\d+) planned_peak=(\d+)$",re.M)

def trace(arm,stderr):
    if PREC.findall(stderr)!=[(str(DIGITS),str(BITS))]:
        raise ValueError("PRECISION_DRIFT")
    if arm=="adaptive":
        m=ADAPTIVE.findall(stderr)
        if (len(m)!=1 or m[0][:3]!=m[0][3:] or
            "RESEARCH_MPFR_ADAPTIVE route=definitional hybrid=false completed=true" not in stderr or
            "ADAPTIVE_FALLBACK" in stderr):
            raise ValueError("ADAPTIVE_ROUTE_DRIFT")
        sig=list(map(int,m[0]))
        if sig[0]>20 or sig[1]>60_000_000 or sig[2]>2_000_000:
            raise ValueError("ADAPTIVE_CAP")
        return sig
    m=TRACE.findall(stderr)
    if len(m)!=1:
        raise ValueError("PREJOIN_ROUTE_DRIFT")
    r,c,l,w,k,p,j,_=map(int,m[0])
    if r>64 or c>2 or l!=1<<c or w>19 or k>40_000_000 or p>2_000_000:
        raise ValueError("PREJOIN_CAP")
    return [r,c,l,w,k,p,j]

def prepare():
    q=Path(".upstream/quprs");g=Path(".upstream/gpmc")
    if git_sha(q)!=QU_PRS_SHA or git_sha(g)!=GPMC_SHA:
        raise ValueError("SOURCE_REVISION_DRIFT")
    blob=subprocess.check_output(["git","-C",str(q),"rev-parse",f"{QU_PRS_SHA}:{REL}"],text=True).strip()
    if BLOB is not None and blob!=BLOB:raise ValueError("QASM_BLOB_DRIFT")
    ROOT.mkdir(parents=True,exist_ok=True)
    cnf=ROOT/f"{CASE}-gpmc.cnf";qasm=ROOT/f"{CASE}.qasm"
    exporter=q/"rust/target/release/examples/quprs_long_tail_source_bridge"
    row=native([str(exporter.resolve()),CASE,str(q.resolve()),str(ROOT.resolve()),REL],"source",EXPORT_S,None)
    if (row["status"]!="COMPLETED" or not cnf.is_file() or not qasm.is_file() or
        sha(qasm)!=sha(q/REL) or not 0<cnf.stat().st_size<=CNF_MAX_BYTES or
        (EXPECTED is not None and sha(cnf)!=EXPECTED)):
        raise ValueError("SOURCE_DRIFT")
    expected=sha(cnf)
    report={"status":"REFERENCE_PENDING","source_cnf_sha256":expected,"source_qasm_git_blob":blob,
            "source_qasm_sha256":sha(qasm),"checkout":git_sha(Path(".")),"quprs":QU_PRS_SHA,"gpmc":GPMC_SHA,
            "hash_status":"FIRST_FREEZE" if EXPECTED is None else "PRELOCKED",
            "digits":DIGITS,"bits":BITS,"target_digits":TARGET,"native_limit_s":LIMIT,
            "scope":("whole short original GF2^6 Rust-only contraction proxy; no GPMC Simulation or GF2^7 timing claim" if GF2 else "whole short original QPE8 proxy only; no long QPE21/22 simulation claim"),"native":{}}
    path=ROOT/"qualification.json"
    def persist():path.write_text(json.dumps(report,indent=2)+"\n")
    persist()
    oracle=Path(__file__).with_name("original_exact_prejoin.py" if GF2 else "original_fixed_interval.py")
    refproc=subprocess.run(["/usr/bin/timeout","-k","2s",f"{REFERENCE_LIMIT}s","prlimit",f"--as={AS_BYTES}","--",
                          sys.executable,str(oracle),str(cnf),expected],capture_output=True,text=True,timeout=REFERENCE_LIMIT+7)
    if refproc.returncode:
        report.update(status="REFERENCE_INCONCLUSIVE",reference_error=refproc.stderr[-1000:]);persist()
        raise ValueError("REFERENCE_INCONCLUSIVE")
    ref=json.loads(refproc.stdout)
    if ref["source_sha256"]!=expected or ref["bits"]!=550:raise ValueError("REFERENCE_DRIFT")
    if GF2 and (ref["status"]!="INDEPENDENT_INTERVAL_COMPLETE" or ref["width"]>19 or ref["work"]>40_000_000 or ref["peak"]>2_000_000):raise ValueError("REFERENCE_CAP")
    report["reference"]=ref
    binary="benchmarks/mpfr_research/target/release/rawmocs-mpfr-research"
    cmds={"gpmc":[str(g/"build/gpmc"),"-mode=1",f"-prec={DIGITS}",str(cnf)],
          "adaptive":[binary,"--adaptive",str(DIGITS),str(cnf)],
          "prejoin":[binary,"--factor-prejoin",str(DIGITS),str(cnf)]}
    report["binary_sha256"]={"rust":sha(Path(binary))}
    if not GF2:report["binary_sha256"]["gpmc"]=sha(g/"build/gpmc")
    config=["options:",'  max-time: "5s"',"benchmarks:"]
    for arm in ARMS:
        row=native(cmds[arm],"gpmc" if arm=="gpmc" else "candidate",LIMIT,DIGITS,expected_bits=BITS)
        report["native"][arm]=row
        report["status"]="NATIVE_PREFLIGHT_INCONCLUSIVE";persist()
        if row["status"]!="COMPLETED" or not 0<row.get("wall_ns",0)<=LIMIT*10**9:
            raise ValueError("NATIVE_BUDGET")
        row["witness"]=compare(row,ref,TARGET)
        if not row["witness"]["pass"]:persist();raise ValueError("NATIVE_ACCURACY")
        if arm!="gpmc":row["trace"]=trace(arm,row["stderr_tail"])
        wrapper=ROOT/f"{arm}.sh";out=ROOT/f"{arm}.stdout";err=ROOT/f"{arm}.stderr"
        for f in (out,err):f.unlink(missing_ok=True)
        wrapper.write_text("#!/bin/sh\nexec "+shlex.join(cmds[arm])+" >"+shlex.quote(str(out))+" 2>"+shlex.quote(str(err))+"\n")
        wrapper.chmod(0o755)
        config.extend([f'  - name: "original_focused/{BENCHMARK}/{arm}_mpfr{BITS}"',f'    exec: "./{wrapper}"'])
        persist()
    report["status"]="NATIVE_PREFLIGHT_PASS";persist()
    CONFIG.write_text("\n".join(config)+"\n")
    print("PREJOIN_CODSPEED_PREFLIGHT_PASS",flush=True)

def verify():
    path=ROOT/"qualification.json";r=json.loads(path.read_text())
    if r["status"]!="NATIVE_PREFLIGHT_PASS" or sha(ROOT/f"{CASE}-gpmc.cnf")!=r["source_cnf_sha256"] or git_sha(Path("."))!=r["checkout"]:
        raise ValueError("POST_SOURCE_DRIFT")
    r["status"]="SIMULATION_OUTPUT_INCONCLUSIVE";r["simulation"]={}
    try:
        for arm in ARMS:
            sat,(real,imag)=parse_out((ROOT/f"{arm}.stdout").read_text(),"gpmc" if arm=="gpmc" else "rawmocs_mpfr")
            row={"status":"COMPLETED","sat":sat,"real":str(real),"imag":str(imag)}
            r["simulation"][arm]=row
            row["witness"]=compare(row,r["reference"],TARGET)
            before=r["native"][arm]
            if not row["witness"]["pass"] or any(row[k]!=before[k] for k in ("sat","real","imag")):
                raise ValueError("SIMULATION_OUTPUT_DRIFT")
            if arm!="gpmc":
                row["trace"]=trace(arm,(ROOT/f"{arm}.stderr").read_text())
                if row["trace"]!=before["trace"]:raise ValueError("SIMULATION_PLAN_DRIFT")
        r["status"]="SIMULATION_OUTPUT_ROUTE_PASS"
        print("PREJOIN_CODSPEED_OUTPUT_ROUTE_PASS",flush=True)
    finally:
        path.write_text(json.dumps(r,indent=2)+"\n")

if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("stage",choices=("prepare","verify"))
    a=p.parse_args()
    (prepare if a.stage=="prepare" else verify)()
