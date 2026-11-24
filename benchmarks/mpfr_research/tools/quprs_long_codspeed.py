#!/usr/bin/env python3
"""Focused original-CNF native-executable Simulation qualification.

Generate a separate two-arm configuration; preserve legacy benchmark identities.
A green CodSpeed upload alone is not a qualified full-solver comparison.
"""
import argparse
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

from original_fixed_interval import compare
from quprs_long_tail_paired import (
    AS_BYTES, BITS, CNF_MAX_BYTES, EXPORT_S, GPMC_SHA, PAIR_S, QU_PRS_SHA,
    git_sha, native, parse_out, sha,
)
from quprs_mqt_adaptive_scout import result_route

CASES = {
    "grover6_large": (
        "qiskit-2.6.1-opt0/Random_Rotation/grover-noancilla_nativegates_ibm_qiskit_opt0_6.qasm",
        "a9757cd5bd730b6f769d2970e3a16ac66fee7157b55f81208c0a7c2cece0023b",
    ),
    "wstate16": (
        "qiskit-2.6.1-opt0/Random_Rotation/wstate_nativegates_ibm_qiskit_opt0_16.qasm",
        "600ce6716a69e31e17ffbc571cba581fc58a5b5ed3b7d60227ad41d857374efe",
    ),
}
ROOT = Path(".research/codspeed-long")
CONFIG = Path("codspeed-long.generated.yml")
ARMS = ("gpmc", "adaptive")
PROFILE = re.compile(r"^ADAPTIVE_CONTRACTION width=(\d+) work=(\d+) peak=(\d+) planned_width=(\d+) planned_work=(\d+) planned_peak=(\d+)$", re.M)
PRECISION = re.compile(r"^RESEARCH_MPFR_PHASE_NS .* digits=(\d+) bits=(\d+)$", re.M)

def trace(stderr, digits):
    row = {"status": "COMPLETED", "stderr_tail": stderr}
    p = PROFILE.findall(stderr)
    b = PRECISION.findall(stderr)
    if (result_route(row) != "definitional" or "ADAPTIVE_FALLBACK" in stderr
            or len(p) != 1 or len(b) != 1 or tuple(map(int,b[0])) != (digits,BITS[digits])):
        raise ValueError("ROUTE_OR_PRECISION_DRIFT")
    counts = list(map(int,p[0]))
    if counts[:3] != counts[3:]:
        raise ValueError("PLANNED_EXECUTION_DRIFT")
    return counts

def prepare(case, digits):
    rel, expected = CASES[case]
    rel = "benchmarks/MQTBench/" + rel
    quprs = Path(".upstream/quprs")
    gpmc = Path(".upstream/gpmc")
    if git_sha(quprs) != QU_PRS_SHA or git_sha(gpmc) != GPMC_SHA:
        raise ValueError("SOURCE_REVISION_DRIFT")
    ROOT.mkdir(parents=True, exist_ok=True)
    exporter = quprs / "rust/target/release/examples/quprs_long_tail_source_bridge"
    # Use the existing exporter directly on the original tracked QASM.
    export_id = "extended_" + case
    cnf = ROOT / (export_id + "-gpmc.cnf")
    qasm = ROOT / (export_id + ".qasm")
    export = native([str(exporter.resolve()),export_id,str(quprs.resolve()),str(ROOT.resolve()),rel],
                    "source",EXPORT_S,None)
    if (export["status"] != "COMPLETED" or not cnf.is_file()
            or not qasm.is_file() or sha(qasm) != sha(quprs / rel)
            or not 0 < cnf.stat().st_size <= CNF_MAX_BYTES or sha(cnf) != expected):
        raise ValueError("FROZEN_SOURCE_DRIFT")
    oracle = Path(__file__).with_name("original_fixed_interval.py")
    refproc = subprocess.run(
        ["/usr/bin/timeout","-k","2s","190s","prlimit",f"--as={AS_BYTES}","--",
         sys.executable,str(oracle),str(cnf),expected],
        check=True,capture_output=True,text=True,timeout=197)
    ref = json.loads(refproc.stdout)
    if ref["source_sha256"] != expected or ref["bits"] != 550:
        raise ValueError("REFERENCE_DRIFT")
    commands = {
        "gpmc": [str(gpmc / "build/gpmc"),"-mode=1",f"-prec={digits}",str(cnf)],
        "adaptive": ["benchmarks/mpfr_research/target/release/rawmocs-mpfr-research",
                     "--adaptive",str(digits),str(cnf)],
    }
    rows = {}
    config = ["options:", '  max-time: "30s"', "benchmarks:"]
    for arm in ARMS:
        cmd = commands[arm]
        row = native(cmd,"gpmc" if arm=="gpmc" else "candidate",PAIR_S,digits,expected_bits=BITS[digits])
        if row["status"] != "COMPLETED" or not compare(row,ref,digits)["pass"]:
            raise ValueError("NATIVE_PREFLIGHT_INCONCLUSIVE")
        if arm == "adaptive":
            row["contraction_trace"] = trace(row["stderr_tail"],digits)
        rows[arm] = row
        # Identical minimal launch boundaries. exec replaces the shell;
        # output redirection preserves the measured process's actual evidence.
        wrapper = ROOT / (arm + ".sh")
        out, err = ROOT / (arm + ".stdout"), ROOT / (arm + ".stderr")
        for f in (out,err):
            f.unlink(missing_ok=True)
        wrapper.write_text("#!/bin/sh\nexec " + shlex.join(cmd)
                           + " >" + shlex.quote(str(out)) + " 2>" + shlex.quote(str(err)) + "\n")
        wrapper.chmod(0o755)
        config.extend([f'  - name: "original_long/{case}/{arm}_mpfr{BITS[digits]}"',
                       f'    exec: "./{wrapper}"'])
    report = {"status":"NATIVE_PREFLIGHT_PASS","case":case,"digits":digits,"bits":BITS[digits],
              "source_path":rel,"source_cnf_sha256":expected,"source_qasm_sha256":sha(qasm),
              "raWmocs_checkout":git_sha(Path(".")),"quprs":QU_PRS_SHA,"gpmc":GPMC_SHA,
              "reference":ref,"native":rows,"commands":commands,
              "simulation_note":"No native AS cap on instrumentation; fixed job deadline. No solver internal limits changed."}
    (ROOT / "qualification.json").write_text(json.dumps(report,indent=2)+"\n")
    CONFIG.write_text("\n".join(config)+"\n")
    print("LONG_CODSPEED_PREFLIGHT_PASS",case,BITS[digits],flush=True)

def verify():
    path = ROOT / "qualification.json"
    report = json.loads(path.read_text())
    if report["status"] != "NATIVE_PREFLIGHT_PASS":
        raise ValueError("MISSING_PREFLIGHT")
    case,digits = report["case"],report["digits"]
    if sha(ROOT / ("extended_"+case+"-gpmc.cnf")) != CASES[case][1] or git_sha(Path(".")) != report["raWmocs_checkout"]:
        raise ValueError("POST_SIMULATION_SOURCE_DRIFT")
    rows = {}
    report["simulated_outputs"] = rows
    report["status"] = "SIMULATION_OUTPUT_INCONCLUSIVE"
    try:
        for arm in ARMS:
            stdout = (ROOT / (arm+".stdout")).read_text()
            stderr = (ROOT / (arm+".stderr")).read_text()
            sat,(real,imag) = parse_out(stdout,"gpmc" if arm=="gpmc" else "rawmocs_mpfr")
            row = {"status":"COMPLETED","sat":sat,"real":str(real),"imag":str(imag)}
            rows[arm] = row
            row["independent_witness"] = compare(row,report["reference"],digits)
            before = report["native"][arm]
            if not row["independent_witness"]["pass"] or any(row[k]!=before[k] for k in ("sat","real","imag")):
                raise ValueError("SIMULATION_OUTPUT_DRIFT")
            if arm=="adaptive":
                row["contraction_trace"] = trace(stderr,digits)
                if row["contraction_trace"]!=before["contraction_trace"]:
                    raise ValueError("SIMULATION_PLAN_DRIFT")
        report["status"]="SIMULATION_OUTPUT_ROUTE_PASS"
        print("LONG_CODSPEED_OUTPUT_ROUTE_PASS",case,BITS[digits],flush=True)
    finally:
        path.write_text(json.dumps(report,indent=2)+"\n")

def self_test():
    good=("ADAPTIVE_CONTRACTION width=18 work=10 peak=5 planned_width=18 planned_work=10 planned_peak=5\n"
          "RESEARCH_MPFR_PHASE_NS read=1 parse=1 solve=1 output=1 nodes=0 cache_hits=0 decompositions=0 digits=30 bits=100\n"
          "RESEARCH_MPFR_ADAPTIVE route=definitional hybrid=false completed=true\n")
    assert trace(good,30)==[18,10,5,18,10,5]
    for bad in (good.replace("bits=100","bits=333"),good.replace("planned_work=10","planned_work=11"),
                good.replace("route=definitional","route=trail"),good+"ADAPTIVE_FALLBACK from=definitional\n"):
        try: trace(bad,30)
        except ValueError: pass
        else: raise AssertionError("drift accepted")

if __name__ == "__main__":
    self_test()
    ap=argparse.ArgumentParser()
    ap.add_argument("stage",choices=("prepare","verify","self-test"))
    ap.add_argument("--case",choices=CASES,default="grover6_large")
    ap.add_argument("--digits",type=int,choices=BITS,default=30)
    a=ap.parse_args()
    if a.stage=="prepare":prepare(a.case,a.digits)
    elif a.stage=="verify":verify()
    else:print("LONG_CODSPEED_SELF_TEST PASS")
