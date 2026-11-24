#!/usr/bin/env python3
"""Original-source, GPMC-completer-first scout. No solver-win-based selection."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import subprocess

from quprs_long_tail_paired import (
    QU_PRS_SHA, GPMC_SHA, BITS, AS_BYTES, CNF_MAX_BYTES, EXPORT_S, PAIR_S,
    native, command, sha, git_sha, parse_cnf, complex_relative_witness,
)

# Predeclared families, near the previously measured transition region.
# Only existing tracked files enter; no interpolation or circuit synthesis.
MQT = (
    ("qnn", 9, 11), ("wstate", 17, 24), ("qaoa", 12, 16),
    ("qpeexact", 17, 24), ("qft", 17, 24),
    ("grover-noancilla", 7, 9), ("qwalk-noancilla", 6, 8),
    ("vqe", 12, 24), ("qpeinexact", 12, 24),
)
FEYNMAN = (("gf2", 7, 9), ("adder", 9, 16), ("qcla_mod", 8, 16),
           ("barenco_tof", 11, 20))
MAX_CASES = 12
SHARDS = 3
MIN_SECONDS = 10

def candidates(root):
    paths = subprocess.check_output(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", QU_PRS_SHA,
         "--", "benchmarks/Feynman", "benchmarks/MQTBench"], text=True).splitlines()
    groups = []
    for family, lo, hi in MQT:
        found = []
        pattern = re.compile(re.escape(family)+r"_nativegates_ibm_qiskit_opt0_(\d+)\.qasm$")
        for rel in paths:
            if not rel.startswith("benchmarks/MQTBench/"):
                continue
            m = pattern.fullmatch(Path(rel).name)
            if m and lo <= int(m[1]) <= hi:
                found.append((int(m[1]), rel))
        groups.append(("mqt_"+family.replace("-","_"), sorted(found)))
    for family, lo, hi in FEYNMAN:
        found = []
        pattern = re.compile(r"gf2\^(\d+)_mult\.qasm$" if family=="gf2" else re.escape(family)+r"_(\d+)\.qasm$")
        for rel in paths:
            if not rel.startswith("benchmarks/Feynman/"):
                continue
            m = pattern.fullmatch(Path(rel).name)
            if m and lo <= int(m[1]) <= hi and not (family=="gf2" and int(m[1])==8):
                found.append((int(m[1]), rel))
        groups.append(("feynman_"+family, sorted(found)))
    out=[]
    # Round-robin families, smallest available size first, no timing input.
    for j in range(2):
        for family, group in groups:
            if len(group)>j:
                size, rel=group[j]
                variant=re.sub(r"[^a-z0-9]+","_",str(Path(rel).parent).lower()).strip("_")
                suffix=sha256(rel.encode()).hexdigest()[:12]
                out.append({"case":f"extended_{family}_{size}_{suffix}", "family":family, "size":size, "variant":variant,"path":rel})
    if len({r["case"] for r in out})!=len(out):raise ValueError("DUPLICATE_SOURCE_ID")
    return out[:MAX_CASES]

def qualified(rows):
    return (len(rows)==2 and all(r["status"]=="COMPLETED" and r["wall_ns"]>=MIN_SECONDS*10**9 for r in rows))

def rust_command(args,item,cnf,digits):
    family=item["family"]
    if family=="mqt_qnn":
        mode="--factor-cardinality"
    elif family in ("mqt_wstate","mqt_qwalk_noancilla"):
        mode="--factor-definitional"
    elif family=="feynman_gf2":
        mode="--factor-taylor-strict"
    else:
        return command(args,item["case"],cnf,digits,"inline_full"),"inline_full"
    return [str(args.mpfr.resolve()),mode,str(digits),str(cnf.resolve())],mode

def main():
    ap=argparse.ArgumentParser()
    for arg in ("quprs","gpmc-source","exporter","gpmc","mpfr","corpus","report"):
        ap.add_argument("--"+arg,type=Path,required=True)
    ap.add_argument("--shard",type=int,choices=range(SHARDS),required=True)
    args=ap.parse_args()
    if git_sha(args.quprs)!=QU_PRS_SHA or git_sha(args.gpmc_source)!=GPMC_SHA:
        raise SystemExit("SOURCE_LOCK_DRIFT")
    args.corpus.mkdir(parents=True,exist_ok=True)
    manifest=candidates(args.quprs)
    selected=manifest[args.shard::SHARDS]
    report={"study":"Original GPMC-completer long-case discovery",
            "source_revisions":{"quprs":QU_PRS_SHA,"gpmc":GPMC_SHA},
            "selection":"both 100/333-bit GPMC screens completed >=10s; rank by min GPMC wall; no Rust-dependent selection",
            "limits":{"wall_s":PAIR_S,"AS_bytes":AS_BYTES,"max_candidates":MAX_CASES},
            "shard":args.shard,"manifest":manifest,"cases":[],"pilot":None,
            "status":"IN_PROGRESS","independent_full_wmc_reference":False}
    def persist():
        args.report.write_text(json.dumps(report,indent=2)+"\n")
    persist()
    for item in selected:
        case=item["case"]
        src=args.quprs/item["path"]
        cnf=args.corpus/f"{case}-gpmc.cnf"
        copy=args.corpus/f"{case}.qasm"
        row={**item,"source_qasm_sha256":sha(src),"screen":[]}
        report["cases"].append(row)
        row["export"]=native([str(args.exporter.resolve()),case,str(args.quprs.resolve()),str(args.corpus.resolve()),item["path"]],"source",EXPORT_S,None)
        if row["export"]["status"]!="COMPLETED" or not cnf.is_file() or not copy.is_file():
            row["source_status"]="CENSORED_EXPORT"
        elif copy.read_bytes()!=src.read_bytes():
            raise SystemExit("ORIGINAL_QASM_DRIFT")
        elif not 0<cnf.stat().st_size<=CNF_MAX_BYTES:
            row["source_status"]="CENSORED_CNF_SIZE"
        else:
            try:
                parsed=parse_cnf(cnf,"gpmc")
                row.update(source_status="SOURCE_EXACT",cnf_sha256=sha(cnf),cnf_variables=parsed["variables"],cnf_clauses=len(parsed["clauses"]))
            except (ValueError,OSError) as e:
                row.update(source_status="CENSORED_INVALID_CNF",error=str(e)[:300])
        persist()
        if row["source_status"]=="SOURCE_EXACT":
            for digits,bits in BITS.items():
                result=native(command(args,case,cnf,digits,"gpmc"),"gpmc",PAIR_S,digits)
                row["screen"].append({"digits":digits,"bits":bits,**result})
                persist()
                print("COMPLETER_SCREEN",case,bits,result["status"],result.get("wall_ns"),flush=True)
                if result["status"]!="COMPLETED":
                    break
        row["qualified"]=qualified(row["screen"])
        persist()
    eligible=sorted((r for r in report["cases"] if r["qualified"]),
                    key=lambda r:(-min(x["wall_ns"] for x in r["screen"]),r["case"]))
    report["qualified_cases"]=[r["case"] for r in eligible]
    # At most one exploratory pair per precision per shard, after control-only selection.
    if eligible:
        item=eligible[0]
        case=item["case"]
        cnf=args.corpus/f"{case}-gpmc.cnf"
        if sha(cnf)!=item["cnf_sha256"]:raise SystemExit("CNF_DRIFT")
        pilot={"case":case,"cohorts":[],"status":"EXPLORATORY_ONE_PAIR_PER_PRECISION"}
        report["pilot"]=pilot
        for digits,bits in BITS.items():
            rust,route=rust_command(args,item,cnf,digits)
            cohort={"digits":digits,"bits":bits,"route":route,"trials":[]}
            pilot["cohorts"].append(cohort)
            arms=("gpmc","rust") if bits==100 else ("rust","gpmc")
            for arm in arms:
                cmd=command(args,case,cnf,digits,"gpmc") if arm=="gpmc" else rust
                r=native(cmd,"gpmc" if arm=="gpmc" else "candidate",PAIR_S,digits)
                r["arm"]=arm
                cohort["trials"].append(r)
                persist()
                print("COMPLETER_PILOT",case,bits,arm,r["status"],r.get("wall_ns"),flush=True)
            pair={r["arm"]:r for r in cohort["trials"]}
            a,b=pair["gpmc"],pair["rust"]
            if a["status"]==b["status"]=="COMPLETED":
                witness=complex_relative_witness(a,b,digits)
                cohort["witness"]=witness
                route_marker={"--factor-cardinality":"RESEARCH_MPFR_CARDINALITY ",
                              "--factor-definitional":"RESEARCH_MPFR_DEFINITIONAL ",
                              "--factor-taylor-strict":"RESEARCH_MPFR_TAYLOR_STRICT "}.get(route)
                route_ok=route_marker is None or route_marker in b.get("stderr_tail","")
                if route=="--factor-taylor-strict":
                    route_ok=route_ok and "certified=true" in b.get("stderr_tail","")
                if witness["pass"] and a["sat"]==b["sat"] and route_ok:
                    cohort["status"]="SINGLE_MATCHED_PAIR"
                    cohort["gpmc_over_rust"]=a["wall_ns"]/b["wall_ns"]
                else:
                    cohort["status"]="NUMERIC_OR_ROUTE_INCONCLUSIVE"
            else:
                cohort["status"]="CENSORED_PAIR"
            persist()
    report["status"]="SCREEN_COMPLETE_EXPLORATORY"
    persist()
    print("COMPLETER_FINAL",args.shard,"manifest",len(manifest),"qualified",report["qualified_cases"],flush=True)

if __name__=="__main__":
    main()
