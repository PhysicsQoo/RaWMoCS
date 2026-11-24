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

HOLDOUT_MAX=16
HOLDOUT_SHARDS=4
# Existing measurements identify a transition near QPE20 and QNN/Wstate
# moderate sizes. Sample both perturbed variants and other families.
# Neither solver's outcome on a new case enters this selection.
HOLDOUT_TARGETS=(("qpeinexact",20),("qpeexact",22),("wstate",16),
                 ("qnn",8),("grover-noancilla",6),("qwalk-noancilla",5),
                 ("qaoa",11),("vqe",16),("qft",24),("graphstate",24),
                 ("bv",24),("ghz",24))

def select_mqt_holdouts(paths,excluded):
    from quprs_mqt_adaptive_scout import item
    groups=[]
    for family,target in HOLDOUT_TARGETS:
        pattern=re.compile(re.escape(family)+r"_nativegates_ibm_qiskit_opt0_(\\d+)\\.qasm$")
        rows=[]
        for rel in paths:
            if rel in excluded or not rel.startswith("benchmarks/MQTBench/"):
                continue
            m=pattern.fullmatch(Path(rel).name)
            if not m or not 4<=int(m[1])<=32:
                continue
            # Only original weighted-equivalence perturbation variants.
            if Path(rel).parent.name not in ("Random_Pauli","Random_Rotation"):
                continue
            row=item(rel,"new_long_control_holdout")
            rows.append(row)
        rows.sort(key=lambda r:(abs(r["size"]-target),
                                r["variant"].endswith("Random_Pauli"),
                                r["size"],r["path"]))
        groups.append(rows)
    out=[]
    for j in range(2):
        for group in groups:
            if len(group)>j:
                out.append(group[j])
    out=out[:HOLDOUT_MAX]
    if len({r["path"] for r in out})!=len(out) or len({r["case"] for r in out})!=len(out):
        raise ValueError("DUPLICATE_HOLDOUT")
    return out

def mqt_holdouts(root):
    from quprs_mqt_adaptive_scout import candidates as previous_mqt
    paths=subprocess.check_output(["git","-C",str(root),"ls-tree","-r","--name-only",QU_PRS_SHA,
        "--","benchmarks/MQTBench"],text=True).splitlines()
    excluded={r["path"] for r in candidates(root)+previous_mqt(root)}
    return select_mqt_holdouts(paths,excluded)

def holdout_self_test():
    pauli="benchmarks/MQTBench/Random_Pauli/qpeexact_nativegates_ibm_qiskit_opt0_22.qasm"
    rotation=pauli.replace("Random_Pauli","Random_Rotation")
    wstate="benchmarks/MQTBench/Random_Rotation/wstate_nativegates_ibm_qiskit_opt0_16.qasm"
    canonical="benchmarks/MQTBench/qpeexact_nativegates_ibm_qiskit_opt0_22.qasm"
    paths=[pauli,rotation,wstate,canonical]
    a=select_mqt_holdouts(paths,{pauli})
    b=select_mqt_holdouts(list(reversed(paths)),{pauli})
    assert a==b
    assert {r["path"] for r in a}=={rotation,wstate}
    assert not qualified([{"status":"CENSORED_TIME","wall_ns":120*10**9},
                          {"status":"COMPLETED","wall_ns":20*10**9}])
    assert qualified([{"status":"COMPLETED","wall_ns":20*10**9},
                      {"status":"COMPLETED","wall_ns":30*10**9}])

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
    ap.add_argument("--shard",type=int,choices=range(HOLDOUT_SHARDS),required=True)
    ap.add_argument("--mqt-holdout",action="store_true")
    args=ap.parse_args()
    holdout_self_test()
    shards=HOLDOUT_SHARDS if args.mqt_holdout else SHARDS
    if args.shard>=shards:ap.error("shard outside selected cohort")
    if git_sha(args.quprs)!=QU_PRS_SHA or git_sha(args.gpmc_source)!=GPMC_SHA:
        raise SystemExit("SOURCE_LOCK_DRIFT")
    args.corpus.mkdir(parents=True,exist_ok=True)
    manifest=mqt_holdouts(args.quprs) if args.mqt_holdout else candidates(args.quprs)
    selected=manifest[args.shard::shards]
    report={"study":"Original GPMC-completer long-case discovery",
            "source_revisions":{"quprs":QU_PRS_SHA,"gpmc":GPMC_SHA},
            "selection":"both 100/333-bit GPMC screens completed >=10s; rank by min GPMC wall; no Rust-dependent selection",
            "limits":{"wall_s":PAIR_S,"AS_bytes":AS_BYTES,"max_candidates":HOLDOUT_MAX if args.mqt_holdout else MAX_CASES},
            "shard":args.shard,"mqt_holdout":args.mqt_holdout,"manifest":manifest,"cases":[],"pilot":None,
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
            if args.mqt_holdout:
                route="--adaptive"
                rust=[str(args.mpfr.resolve()),route,str(digits),str(cnf.resolve())]
            else:
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
                if route=="--adaptive":
                    from quprs_mqt_adaptive_scout import result_route
                    cohort["selected_route"]=result_route(b)
                    route_ok=cohort["selected_route"] is not None
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
