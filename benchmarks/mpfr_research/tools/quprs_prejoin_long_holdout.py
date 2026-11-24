"""GPMC-completer-first long holdout scout; one pair is exploratory."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
from original_fixed_interval import compare
from quprs_long_tail_paired import AS_BYTES, CNF_MAX_BYTES, EXPORT_S, GPMC_SHA, PAIR_S, QU_PRS_SHA, git_sha, native, sha
from quprs_prejoin_native_pilot import TRACE

# Original tracked source blobs, not generated or simplified benchmarks.
CASES=(
    ("extended_gf2_7_prejoin_scout","benchmarks/Feynman/gf2^7_mult.qasm","640d6fe1c8869858d212c74426b85c7e2768096e"),
    ("extended_qnn12_pauli_prejoin_scout","benchmarks/MQTBench/Random_Pauli/qnn_nativegates_ibm_qiskit_opt0_12.qasm","3faaab08384c0548915e52080d4259c04c0b0f32"),
)

def main():
    ap=argparse.ArgumentParser()
    for name in ("quprs","gpmc-source","exporter","gpmc","mpfr","corpus","report"):
        ap.add_argument("--"+name,type=Path,required=True)
    ap.add_argument("--shard",type=int,choices=range(len(CASES)),required=True)
    a=ap.parse_args()
    case,rel,blob=CASES[a.shard]
    a.corpus.mkdir(parents=True,exist_ok=True)
    cnf=a.corpus/f"{case}-gpmc.cnf";qasm=a.corpus/f"{case}.qasm"
    report={"case":case,"original_qasm":rel,"source_blob":blob,"status":"SOURCE_PENDING",
            "quprs":QU_PRS_SHA,"gpmc":GPMC_SHA,"limits":{"native_s":PAIR_S,"reference_s":190,"AS_bytes":AS_BYTES},
            "scope":"single matched pair exploratory; no certification of repeated performance"}
    def persist():
        a.report.write_text(json.dumps(report,indent=2)+"\n")
    def finish(status):
        report["status"]=status;persist()
        print("LONG_PREJOIN_SCOUT_FINAL",case,status,flush=True)
    persist()
    if git_sha(a.quprs)!=QU_PRS_SHA or git_sha(a.gpmc_source)!=GPMC_SHA:
        raise ValueError("SOURCE_REVISION_DRIFT")
    if subprocess.check_output(["git","-C",str(a.quprs),"rev-parse",f"{QU_PRS_SHA}:{rel}"],text=True).strip()!=blob:
        raise ValueError("QASM_BLOB_DRIFT")
    export=native([str(a.exporter.resolve()),case,str(a.quprs.resolve()),str(a.corpus.resolve()),rel],"source",EXPORT_S,None)
    report["export"]=export;persist()
    if export["status"]!="COMPLETED":
        finish("EXPORT_CENSORED");return
    if not cnf.is_file() or not qasm.is_file() or sha(qasm)!=sha(a.quprs/rel) or not 0<cnf.stat().st_size<=CNF_MAX_BYTES:
        raise ValueError("ORIGINAL_SOURCE_DRIFT")
    expected=sha(cnf)
    report.update(source_cnf_sha256=expected,source_qasm_sha256=sha(qasm),cnf_lock_status="FIRST_FREEZE_SCOUT",
                  gpmc_binary_sha256=sha(a.gpmc),rust_binary_sha256=sha(a.mpfr),cohorts=[])
    persist()
    # GPMC must actually complete; never spend oracle or Rust budget on a
    # mutually hard source merely because its circuit has more qubits.
    def run(arm,digits,bits):
        if sha(cnf)!=expected:raise ValueError("CNF_DRIFT")
        cmd=([str(a.gpmc.resolve()),"-mode=1",f"-prec={digits}",str(cnf.resolve())] if arm=="gpmc" else
             [str(a.mpfr.resolve()),"--factor-prejoin",str(digits),str(cnf.resolve())])
        return native(cmd,"gpmc" if arm=="gpmc" else "candidate",PAIR_S,digits,expected_bits=bits)
    g=run("gpmc",100,333)
    cohort={"target_digits":100,"working_digits":100,"bits":333,"gpmc":g}
    report["cohorts"].append(cohort);persist()
    print("LONG_PREJOIN_GPMC",case,g["status"],g.get("wall_ns"),flush=True)
    if g["status"]!="COMPLETED":
        finish("GPMC_CENSORED_SKIP");return
    report["gpmc_ge_10s"]=g["wall_ns"]>=10_000_000_000
    # Independent oracle before any speed claim. Its failure is recorded
    # explicitly and still permits a bounded diagnostic Rust run.
    oracle=Path(__file__).with_name("original_exact_prejoin.py")
    rp=subprocess.run(["/usr/bin/timeout","-k","2s","190s","prlimit",f"--as={AS_BYTES}","--",
                       sys.executable,str(oracle),str(cnf.resolve()),expected],
                      capture_output=True,text=True,timeout=197)
    report["reference_process"]={"exit_code":rp.returncode,"stderr_tail":rp.stderr[-1500:]}
    ref=None
    if rp.returncode==0:
        ref=json.loads(rp.stdout)
        if (ref["source_sha256"]!=expected or ref["status"]!="INDEPENDENT_INTERVAL_COMPLETE" or
            ref["bits"]!=550 or ref["width"]>19 or ref["work"]>40_000_000 or ref["peak"]>2_000_000):
            raise ValueError("REFERENCE_DRIFT")
        report["reference"]=ref
    persist()
    for digits,bits in [(100,333),(130,432)]:
        if digits!=100:
            g=run("gpmc",digits,bits)
            cohort={"target_digits":100,"working_digits":digits,"bits":bits,"gpmc":g}
            report["cohorts"].append(cohort);persist()
            if g["status"]!="COMPLETED":
                finish("GUARDED_GPMC_CENSORED");return
        r=run("prejoin",digits,bits);cohort["prejoin"]=r;persist()
        if r["status"]!="COMPLETED":
            finish("RUST_CENSORED_OR_REJECTED");return
        matches=TRACE.findall(r["stderr_tail"])
        if len(matches)!=1:raise ValueError("ROUTE_DRIFT")
        residual,cuts,leaves,width,work,peak,joins,plan_ns=map(int,matches[0])
        if residual>64 or cuts>2 or leaves!=1<<cuts or width>19 or work>40_000_000 or peak>2_000_000:
            raise ValueError("RESOURCE_DRIFT")
        cohort["trace"]=[residual,cuts,leaves,width,work,peak,joins]
        if ref is None:
            finish("BOTH_COMPLETE_REFERENCE_INCONCLUSIVE");return
        cohort["witness"]={k:compare(v,ref,100) for k,v in (("gpmc",g),("prejoin",r))}
        if all(x["pass"] for x in cohort["witness"].values()):
            if not all(x.get("peak_rss_kib",0)>0 for x in (g,r)):
                finish("RSS_INCONCLUSIVE");return
            cohort["gpmc_over_rust"]=g["wall_ns"]/r["wall_ns"]
            print("LONG_PREJOIN_PAIR",case,bits,g["wall_ns"],r["wall_ns"],cohort["gpmc_over_rust"],flush=True)
            finish("ONE_MATCHED_PAIR_PASS");return
        persist()
    finish("ACCURACY_INCONCLUSIVE")

if __name__=="__main__":main()
