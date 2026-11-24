#!/usr/bin/env python3
"""Focused resource-qualified QPE experiment plus independent Graphstate precision gate."""
import argparse
import json
from pathlib import Path
import re
import statistics
import subprocess
import sys
from original_fixed_interval import compare as fixed_compare
from quprs_long_tail_paired import (
    QU_PRS_SHA,GPMC_SHA,BITS,AS_BYTES,EXPORT_S,PAIR_S,
    native,command,sha,git_sha,parse_cnf,complex_relative_witness,
)
from quprs_mqt_adaptive_scout import item,result_route
import qwalk_original_interval as oracle

CASES=(
    ("Random_Pauli","qpeexact",20,"41a34f6f574adf235233c1616890123dbbe56ac7d14f05399e822595f4571368"),
    ("Random_Rotation","qpeexact",20,"5cbea91349fe41f0955873bf6539354fff7714d7bef3167d53bbbf5368b347b6"),
    ("Random_Pauli","qpeinexact",20,"6a3d579f9776cf25570462f8318e04ecd69f7a5a6cde84a2afc789b5e0672c80"),
    ("Random_Pauli","graphstate",16,oracle.GRAPHSTATE_SHA),
)

def main():
    ap=argparse.ArgumentParser()
    for arg in ("quprs","gpmc-source","exporter","gpmc","mpfr","corpus","report"):
        ap.add_argument("--"+arg,type=Path,required=True)
    ap.add_argument("--shard",type=int,choices=range(len(CASES)),required=True)
    ap.add_argument("--certified-repeat",action="store_true")
    args=ap.parse_args()
    if git_sha(args.quprs)!=QU_PRS_SHA or git_sha(args.gpmc_source)!=GPMC_SHA:
        raise SystemExit("SOURCE_LOCK_DRIFT")
    variant,family,size,expected=CASES[args.shard]
    rel=f"benchmarks/MQTBench/{variant}/{family}_nativegates_ibm_qiskit_opt0_{size}.qasm"
    source=item(rel,"locked_precision_control" if family=="graphstate" else "locked_long_control")
    case=source["case"]
    args.corpus.mkdir(parents=True,exist_ok=True)
    cnf=args.corpus/f"{case}-gpmc.cnf"
    qasm=args.corpus/f"{case}.qasm"
    src=args.quprs/rel
    report={"source":source,"source_revisions":{"quprs":QU_PRS_SHA,"gpmc":GPMC_SHA},
            "qasm_sha256":sha(src),"expected_cnf_sha256":expected,"status":"IN_PROGRESS",
            "limits":{"wall_s":PAIR_S,"AS_bytes":AS_BYTES},"cohorts":[]}
    def persist():args.report.write_text(json.dumps(report,indent=2)+"\n")
    persist()
    report["export"]=native([str(args.exporter.resolve()),case,str(args.quprs.resolve()),str(args.corpus.resolve()),rel],"source",EXPORT_S,None)
    persist()
    if report["export"]["status"]!="COMPLETED" or not cnf.is_file() or not qasm.is_file():
        raise SystemExit("SOURCE_EXPORT_CENSORED")
    if qasm.read_bytes()!=src.read_bytes() or sha(cnf)!=expected:
        raise SystemExit("ORIGINAL_SOURCE_DRIFT")
    p=parse_cnf(cnf,"gpmc")
    report.update(cnf_sha256=sha(cnf),cnf_variables=p["variables"],cnf_clauses=len(p["clauses"]))
    persist()
    if family=="graphstate":
        ref=oracle.solve(cnf.read_bytes(),source_sha=expected)
        report["independent_witness"]=ref
        persist()
        good=True
        for target,digits,bits in ((30,60,200),(100,130,432)):
            cohort={"target_digits":target,"working_digits":digits,"working_bits":bits,"trials":[]}
            report["cohorts"].append(cohort)
            for trial in range(3):
                arms=("gpmc","definitional") if (trial+(target==100))%2==0 else ("definitional","gpmc")
                for arm in arms:
                    cmd=([str(args.gpmc.resolve()),"-mode=1",f"-prec={digits}",str(cnf.resolve())]
                         if arm=="gpmc" else [str(args.mpfr.resolve()),"--factor-definitional",str(digits),str(cnf.resolve())])
                    r=native(cmd,arm,PAIR_S,digits,expected_bits=bits)
                    r["trial"]=trial
                    if r["status"]=="COMPLETED":
                        r["independent_witness"]=oracle.compare(r,ref,target,source_sha=expected)
                        r["route_complete"]=arm=="gpmc" or "RESEARCH_MPFR_DEFINITIONAL " in r.get("stderr_tail","")
                    good=good and bool(r.get("route_complete") and r.get("independent_witness",{}).get("pass"))
                    cohort["trials"].append(r);persist()
                    print("GRAPHSTATE_GUARDED",target,bits,trial,arm,r["status"],r.get("independent_witness",{}).get("pass"),flush=True)
        report["status"]="GUARDED_ACCURACY_COMPLETE" if good else "GUARDED_ACCURACY_FAIL"
        persist()
        if not good:raise SystemExit("GRAPHSTATE_GUARDED_ACCURACY_FAIL")
        return
    if args.certified_repeat:
        # Independent oracle is excluded from both timed solver arms and
        # subject to its own finite work, time and address-space bounds.
        witness_cmd=["/usr/bin/timeout","-k","2s","190s","prlimit",f"--as={AS_BYTES}","--",
            sys.executable,str(Path(__file__).with_name("original_fixed_interval.py")),str(cnf.resolve()),expected]
        try:
            run=subprocess.run(witness_cmd,capture_output=True,text=True,timeout=197)
            if run.returncode!=0:raise ValueError(f"interval exit {run.returncode}: {run.stderr[-400:]}")
            ref=json.loads(run.stdout)
            if ref["source_sha256"]!=expected or ref["bits"]!=550:raise ValueError("WITNESS_DRIFT")
        except (ValueError,OSError,subprocess.TimeoutExpired) as e:
            report["status"]="INDEPENDENT_WITNESS_INCONCLUSIVE";report["witness_error"]=str(e)
            persist();print("QPE_CERTIFIED_WITNESS_INCONCLUSIVE",case,str(e),flush=True);return
        report["independent_witness"]=ref;report["independent_full_wmc_reference"]=True
        persist()
        print("QPE_CERTIFIED_WITNESS",case,ref["width"],ref["work"],ref["peak"],ref["wall_s"],flush=True)
        # Rotation previously failed fixed precision. Do not change either
        # arm alone, and keep guarded and fixed precision cohorts distinct.
        specs=((30,60,200),(100,130,432)) if variant=="Random_Rotation" else ((30,30,100),(100,100,333))
        for target,digits,bits in specs:
            cohort={"target_digits":target,"working_digits":digits,"working_bits":bits,"trials":[]}
            report["cohorts"].append(cohort)
            for trial in range(7):
                pair={}
                arms=("gpmc","adaptive") if (trial+(target==100))%2==0 else ("adaptive","gpmc")
                for arm in arms:
                    if sha(cnf)!=expected:raise SystemExit("CNF_DRIFT")
                    cmd=([str(args.gpmc.resolve()),"-mode=1",f"-prec={digits}",str(cnf.resolve())]
                         if arm=="gpmc" else [str(args.mpfr.resolve()),"--adaptive",str(digits),str(cnf.resolve())])
                    r=native(cmd,"gpmc" if arm=="gpmc" else "candidate",PAIR_S,digits,expected_bits=bits)
                    r["arm"]=arm;r["trial"]=trial
                    if r["status"]=="COMPLETED":
                        r["independent_witness"]=fixed_compare(r,ref,target)
                        if arm=="adaptive":
                            r["route"]=result_route(r)
                            m=re.search(r"ADAPTIVE_CONTRACTION width=(\d+) work=(\d+) peak=(\d+) planned_width=(\d+) planned_work=(\d+) planned_peak=(\d+)",r.get("stderr_tail",""))
                            if m:r["contraction"]=dict(zip(("width","work","peak","planned_width","planned_work","planned_peak"),map(int,m.groups())))
                            c=r.get("contraction",{})
                            r["route_verified"]=(r["route"]=="definitional" and c.get("width")==c.get("planned_width")
                                and c.get("work")==c.get("planned_work") and c.get("peak",2_000_001)<=c.get("planned_peak",0))
                        else:r["route_verified"]=True
                    pair[arm]=r;cohort["trials"].append(r);persist()
                    print("QPE_CERTIFIED_PAIR",case,target,bits,trial,arm,r["status"],r.get("independent_witness",{}).get("pass"),r.get("wall_ns"),flush=True)
                good=all(r.get("route_verified") and r.get("independent_witness",{}).get("pass") for r in pair.values())
                if not good:
                    cohort["status"]="CENSORED_OR_ACCURACY_INCONCLUSIVE"
                    break
            else:
                g=statistics.median(r["wall_ns"] for r in cohort["trials"] if r["arm"]=="gpmc")
                t=statistics.median(r["wall_ns"] for r in cohort["trials"] if r["arm"]=="adaptive")
                cohort.update(status="CERTIFIED_SEVEN_MATCHED_PAIRS",gpmc_median_s=g/1e9,rust_median_s=t/1e9,gpmc_over_rust=g/t)
            persist()
            print("QPE_CERTIFIED_FINAL",case,target,bits,cohort["status"],cohort.get("gpmc_over_rust"),flush=True)
        report["status"]="CERTIFIED_COMPLETE" if all(c["status"]=="CERTIFIED_SEVEN_MATCHED_PAIRS" for c in report["cohorts"]) else "CERTIFIED_INCONCLUSIVE"
        persist();return
    report["independent_full_wmc_reference"]=False
    for digits,bits in BITS.items():
        cohort={"digits":digits,"bits":bits,"trials":[]}
        report["cohorts"].append(cohort)
        arms=("gpmc","adaptive") if bits==100 else ("adaptive","gpmc")
        for arm in arms:
            if sha(cnf)!=expected:raise SystemExit("CNF_DRIFT")
            cmd=command(args,case,cnf,digits,"gpmc") if arm=="gpmc" else [
                str(args.mpfr.resolve()),"--adaptive",str(digits),str(cnf.resolve())]
            r=native(cmd,"gpmc" if arm=="gpmc" else "candidate",PAIR_S,digits)
            r["arm"]=arm
            if arm=="adaptive":
                r["route"]=result_route(r)
                m=re.search(r"ADAPTIVE_CONTRACTION width=(\d+) work=(\d+) peak=(\d+) planned_width=(\d+) planned_work=(\d+) planned_peak=(\d+)",r.get("stderr_tail",""))
                if m:r["contraction"]=dict(zip(("width","work","peak","planned_width","planned_work","planned_peak"),map(int,m.groups())))
            cohort["trials"].append(r);persist()
            print("QPE_RESOURCE_PAIR",case,bits,arm,r["status"],r.get("wall_ns"),r.get("route"),r.get("contraction"),flush=True)
        pair={r["arm"]:r for r in cohort["trials"]}
        a,b=pair["gpmc"],pair["adaptive"]
        if a["status"]==b["status"]=="COMPLETED":
            w=complex_relative_witness(a,b,digits)
            cohort["witness"]=w
            c=b.get("contraction",{})
            route_ok=(b.get("route")=="definitional" and c.get("width")==c.get("planned_width")
                and c.get("work")==c.get("planned_work") and c.get("peak",2_000_001)<=c.get("planned_peak",0))
            cohort["route_verified"]=route_ok
            if w["pass"] and a["sat"]==b["sat"] and route_ok:
                cohort["status"]="SINGLE_MATCHED_RESOURCE_PAIR"
                cohort["gpmc_over_rust"]=a["wall_ns"]/b["wall_ns"]
            else:cohort["status"]="NUMERIC_OR_RESOURCE_PLAN_INCONCLUSIVE"
        else:cohort["status"]="CENSORED_PAIR"
        persist()
        print("QPE_RESOURCE_FINAL",case,bits,cohort["status"],cohort.get("gpmc_over_rust"),flush=True)
    report["status"]="EXPLORATORY_COMPLETE"
    persist()

if __name__=="__main__":
    main()
