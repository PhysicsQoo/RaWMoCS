#!/usr/bin/env python3
"""Source-locked MQT cross-family adaptive solver admission study.

Benchmark labels are used ONLY to sample a reproducible cohort, never for
solver routing. No GPMC timeout is used as an eligible comparison.
"""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import subprocess
from quprs_long_tail_paired import (
    QU_PRS_SHA,GPMC_SHA,BITS,AS_BYTES,CNF_MAX_BYTES,EXPORT_S,PAIR_S,
    native,command,sha,git_sha,parse_cnf,complex_relative_witness,
)
MAX_CASES=12
SHARDS=4
KNOWN={
    "benchmarks/MQTBench/Random_Pauli/qpeexact_nativegates_ibm_qiskit_opt0_20.qasm":
        "41a34f6f574adf235233c1616890123dbbe56ac7d14f05399e822595f4571368",
    "benchmarks/MQTBench/Random_Rotation/qpeexact_nativegates_ibm_qiskit_opt0_20.qasm":
        "5cbea91349fe41f0955873bf6539354fff7714d7bef3167d53bbbf5368b347b6",
}
# Target sizes chosen in advance; take nearest available tracked source.
# Exclude previously used canonical examples from held-out selection.
FAMILIES=(("qnn",12),("wstate",20),("qaoa",16),("qpeinexact",20),
          ("qft",20),("grover-noancilla",6),("qwalk-noancilla",6),
          ("vqe",16),("graphstate",20),("bv",20),("ghz",20),("qpeexact",24))
EXCLUDED={
    ("qnn",8),("qnn",12),("wstate",16),("qwalk-noancilla",5),
    ("grover-noancilla",5),("grover-noancilla",6),("qaoa",11),
    ("qpeexact",16),("qft",16),
}
def item(rel,case_class):
    match=re.fullmatch(r"(.+)_nativegates_ibm_qiskit_opt0_(\d+)\.qasm",Path(rel).name)
    if not match:raise ValueError("INVALID_MQT_NAME")
    family,size=match[1],int(match[2])
    suffix=sha256(rel.encode()).hexdigest()[:12]
    return {"case":f"extended_mqt_{family.replace('-','_')}_{size}_{suffix}",
            "family":family,"size":size,"variant":str(Path(rel).parent),
            "path":rel,"case_class":case_class}
def candidates(root):
    paths=subprocess.check_output(["git","-C",str(root),"ls-tree","-r","--name-only",QU_PRS_SHA,
          "--","benchmarks/MQTBench"],text=True).splitlines()
    known=[item(p,"known_long_control") for p in KNOWN if p in paths]
    if len(known)!=len(KNOWN):raise ValueError("MISSING_LOCKED_QPE_CONTROL")
    groups=[]
    for fam,target in FAMILIES:
        pattern=re.compile(re.escape(fam)+r"_nativegates_ibm_qiskit_opt0_(\d+)\.qasm$")
        rows=[]
        for rel in paths:
            m=pattern.fullmatch(Path(rel).name)
            if m is None or rel in KNOWN:continue
            size=int(m[1])
            if not 4<=size<=24:continue
            if (fam,size) in EXCLUDED and Path(rel).parent==Path("benchmarks/MQTBench"):continue
            # Exclude all paths already sampled in the previous scout.
            if fam in ("qpeexact","qft") and size==20 and "Random_" in rel:continue
            if fam in ("vqe","qpeinexact") and size==16 and "Random_" in rel:continue
            rows.append(item(rel,"new_cross_family"))
        rows.sort(key=lambda r:(abs(r["size"]-target),r["size"],r["path"]))
        groups.append(rows)
    out=known
    for i in range(2):
        for group in groups:
            if len(group)>i:out.append(group[i])
    out=out[:MAX_CASES]
    if len({r["case"] for r in out})!=len(out) or len({r["path"] for r in out})!=len(out):
        raise ValueError("DUPLICATE_SOURCE")
    return out
FOLLOWUP_KNOWN={
    **KNOWN,
    "benchmarks/MQTBench/Random_Pauli/qpeinexact_nativegates_ibm_qiskit_opt0_20.qasm":
        "6a3d579f9776cf25570462f8318e04ecd69f7a5a6cde84a2afc789b5e0672c80",
    "benchmarks/MQTBench/Random_Pauli/graphstate_nativegates_ibm_qiskit_opt0_16.qasm":
        "7f874f932882d7a39ff5c7ed9e581ab95b8dac3f5d4b08ee9c880b0fb4a8b8a2",
}
def followup_candidates(root):
    paths=subprocess.check_output(["git","-C",str(root),"ls-tree","-r","--name-only",QU_PRS_SHA,
        "--","benchmarks/MQTBench"],text=True).splitlines()
    if not set(FOLLOWUP_KNOWN)<=set(paths):raise ValueError("MISSING_FOLLOWUP_CONTROL")
    out=[item(p,"known_followup_control") for p in FOLLOWUP_KNOWN]
    # New Rotation variants, distinct from the first campaign's Pauli cases.
    for fam,target in [("qnn",8),("wstate",16),("qaoa",11),("grover-noancilla",6),
                       ("qwalk-noancilla",5),("graphstate",16),("qft",20),("vqe",16)]:
        pattern=re.compile(re.escape(fam)+r"_nativegates_ibm_qiskit_opt0_(\d+)\.qasm$")
        rows=[item(p,"new_rotation_holdout") for p in paths
              if "/Random_Rotation/" in p and p not in FOLLOWUP_KNOWN and pattern.fullmatch(Path(p).name)]
        rows=[r for r in rows if 4<=r["size"]<=24]
        if rows:out.append(min(rows,key=lambda r:(abs(r["size"]-target),r["size"],r["path"])))
    if len({r["case"] for r in out})!=len(out):raise ValueError("DUPLICATE_FOLLOWUP")
    return out

def guarded_graphstate(args,row,cnf,persist):
    from qwalk_original_interval import solve,compare
    source=row["cnf_sha256"]
    witness=solve(cnf.read_bytes(),source_sha=source)
    row["independent_interval"]=witness
    for digits,bits in BITS.items():
        working=digits+30
        working_bits={60:200,130:432}[working]
        cohort={"digits":digits,"bits":bits,"working_digits":working,"working_bits":working_bits,"trials":[]}
        row["cohorts"].append(cohort)
        for arm in (("gpmc","adaptive") if bits==100 else ("adaptive","gpmc")):
            if arm=="gpmc":
                cmd=command(args,row["case"],cnf,digits,"gpmc")
                cmd[2]=f"-prec={working}"
            else:
                cmd=[str(args.mpfr.resolve()),"--adaptive",str(working),str(cnf.resolve())]
            r=native(cmd,arm if arm=="gpmc" else "candidate",PAIR_S,working,expected_bits=working_bits)
            r["arm"]=arm
            if r["status"]=="COMPLETED":
                r["independent_witness"]=compare(r,witness,digits,source_sha=source)
                r["agreement"]=r["independent_witness"]["pass"] and (arm=="gpmc" or result_route(r) is not None)
            cohort["trials"].append(r);persist()
            print("MQT_GRAPHSTATE_GUARDED",bits,arm,r["status"],r.get("agreement"),r.get("wall_ns"),flush=True)
    row["status"]="GUARDED_REFERENCE_PASS" if all(r.get("agreement") for c in row["cohorts"] for r in c["trials"]) else "GUARDED_REFERENCE_INCONCLUSIVE"
    persist()

def result_route(row):
    if row["status"]!="COMPLETED":return None
    match=re.search(r"^RESEARCH_MPFR_ADAPTIVE route=(cardinality|definitional|conditioned|trail) hybrid=(true|false) completed=true$",row.get("stderr_tail",""),re.M)
    return match[1] if match else None
def main():
    ap=argparse.ArgumentParser()
    for arg in ("quprs","gpmc-source","exporter","gpmc","mpfr","corpus","report"):
        ap.add_argument("--"+arg,type=Path,required=True)
    ap.add_argument("--shard",type=int,choices=range(SHARDS),required=True)
    ap.add_argument("--followup",action="store_true")
    args=ap.parse_args()
    if git_sha(args.quprs)!=QU_PRS_SHA or git_sha(args.gpmc_source)!=GPMC_SHA:
        raise SystemExit("SOURCE_LOCK_DRIFT")
    args.corpus.mkdir(parents=True,exist_ok=True)
    manifest=followup_candidates(args.quprs) if args.followup else candidates(args.quprs)
    report={"study":"MQT original-source adaptive router and generic hybrid ablation",
        "source_revisions":{"quprs":QU_PRS_SHA,"gpmc":GPMC_SHA},
        "limits":{"wall_s":PAIR_S,"AS_bytes":AS_BYTES,"max_candidates":MAX_CASES},
        "selection":"predeclared source families and sizes; both GPMC precisions must complete; long >=10s both; short regression only",
        "router_input":"original weighted CNF only; no family ID",
        "shard":args.shard,"manifest":manifest,"cases":[],"status":"IN_PROGRESS",
        "independent_full_wmc_reference":False}
    def persist():args.report.write_text(json.dumps(report,indent=2)+"\n")
    persist()
    for item_ in manifest[args.shard::SHARDS]:
        case=item_["case"]
        src=args.quprs/item_["path"]
        cnf=args.corpus/f"{case}-gpmc.cnf"
        copy=args.corpus/f"{case}.qasm"
        row={**item_,"source_qasm_sha256":sha(src),"screen":[],"cohorts":[]}
        report["cases"].append(row)
        row["export"]=native([str(args.exporter.resolve()),case,str(args.quprs.resolve()),str(args.corpus.resolve()),item_["path"]],"source",EXPORT_S,None)
        if row["export"]["status"]!="COMPLETED" or not cnf.is_file() or not copy.is_file():
            row["source_status"]="CENSORED_EXPORT"
        elif copy.read_bytes()!=src.read_bytes():raise SystemExit("ORIGINAL_QASM_DRIFT")
        elif not 0<cnf.stat().st_size<=CNF_MAX_BYTES:row["source_status"]="CENSORED_CNF_SIZE"
        else:
            try:
                parsed=parse_cnf(cnf,"gpmc")
                row.update(source_status="SOURCE_EXACT",cnf_sha256=sha(cnf),cnf_variables=parsed["variables"],cnf_clauses=len(parsed["clauses"]))
            except (ValueError,OSError) as e:row.update(source_status="CENSORED_INVALID_CNF",error=str(e)[:300])
        persist()
        if row["source_status"]!="SOURCE_EXACT":
            print("MQT_ADAPTIVE_EXPORT",case,row["source_status"],flush=True)
            continue
        locks=FOLLOWUP_KNOWN if args.followup else KNOWN
        if item_["path"] in locks and row["cnf_sha256"]!=locks[item_["path"]]:
            raise SystemExit("KNOWN_CONTROL_CNF_DRIFT")
        if args.followup and item_["family"]=="graphstate" and item_["path"] in FOLLOWUP_KNOWN:
            guarded_graphstate(args,row,cnf,persist)
            continue
        for digits,bits in BITS.items():
            r=native(command(args,case,cnf,digits,"gpmc"),"gpmc",PAIR_S,digits)
            row["screen"].append({"digits":digits,"bits":bits,**r})
            persist()
            print("MQT_ADAPTIVE_GPMC",case,bits,r["status"],r.get("wall_ns"),flush=True)
            if r["status"]!="COMPLETED":break
        if len(row["screen"])!=2 or any(r["status"]!="COMPLETED" for r in row["screen"]):
            row["status"]="CONTROL_CENSORED_NO_RUST_PRIORITY";persist();continue
        row["class"]="LONG_COMPLETER" if min(r["wall_ns"] for r in row["screen"])>=10*10**9 else "SHORT_REGRESSION"
        for control in row["screen"]:
            digits,bits=control["digits"],control["bits"]
            cohort={"digits":digits,"bits":bits,"control":control,"trials":[]}
            row["cohorts"].append(cohort)
            arms=("baseline","adaptive","hybrid") if bits==100 else ("hybrid","adaptive","baseline")
            for arm in arms:
                if sha(cnf)!=row["cnf_sha256"]:raise SystemExit("CNF_DRIFT")
                cmd=command(args,case,cnf,digits,"inline_full") if arm=="baseline" else [
                    str(args.mpfr.resolve()),"--adaptive" if arm=="adaptive" else "--adaptive-hybrid",
                    str(digits),str(cnf.resolve())]
                r=native(cmd,"candidate",PAIR_S,digits);r["arm"]=arm
                if arm!="baseline":
                    r["route"]=result_route(r)
                    r["route_verified"]=r["route"] is not None and f"hybrid={'true' if arm=='hybrid' else 'false'} completed=true" in r.get("stderr_tail","")
                else:r["route_verified"]=True
                if r["status"]=="COMPLETED":
                    w=complex_relative_witness(control,r,digits);r["witness"]=w
                    r["agreement"]=bool(w["pass"] and r["sat"]==control["sat"] and r["route_verified"])
                    if r["agreement"]:r["gpmc_over_rust"]=control["wall_ns"]/r["wall_ns"]
                cohort["trials"].append(r);persist()
                print("MQT_ADAPTIVE_PAIR",case,bits,arm,r["status"],r.get("route"),r.get("agreement"),r.get("wall_ns"),flush=True)
            trial={r["arm"]:r for r in cohort["trials"]}
            a,b=trial["adaptive"],trial["hybrid"]
            if a.get("agreement") and b.get("agreement") and a.get("route")==b.get("route")=="trail":
                cohort["generic_hybrid_ablation_ratio"]=a["wall_ns"]/b["wall_ns"]
            persist()
        row["status"]="EXPLORATORY_COMPLETE"
        persist()
    report["status"]="MQT_ADAPTIVE_SCOUT_COMPLETE";persist()
    print("MQT_ADAPTIVE_FINAL",args.shard,len(manifest),len(report["cases"]),flush=True)
if __name__=="__main__":
    main()
