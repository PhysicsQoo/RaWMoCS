"""One bounded matched pair per QPE precision, independently certified."""
import argparse
import json
import re
from pathlib import Path
from original_fixed_interval import compare
from quprs_long_tail_paired import native, sha, PAIR_S

TRACE = re.compile(r"^PREJOIN_COMPLETE residual=(\d+) cuts=(\d+) leaves=(\d+) width=(\d+) work=(\d+) peak=(\d+) joins=(\d+) plan_ns=(\d+)$", re.M)

def main():
    ap=argparse.ArgumentParser()
    for name in ("cnf","reference","gpmc","mpfr","report"):
        ap.add_argument("--"+name,type=Path,required=True)
    a=ap.parse_args()
    ref=json.loads(a.reference.read_text())
    expected=ref["source_sha256"]
    if sha(a.cnf)!=expected or ref["status"]!="INDEPENDENT_INTERVAL_COMPLETE" or ref["bits"]!=550:
        raise ValueError("REFERENCE_DRIFT")
    report={"status":"IN_PROGRESS","source_sha256":expected,"reference_sha256":sha(a.reference),
            "gpmc_binary_sha256":sha(a.gpmc),"rust_binary_sha256":sha(a.mpfr),"cohorts":[]}
    def persist():
        a.report.write_text(json.dumps(report,indent=2)+"\n")
    persist()
    for target,precisions in [(30,[(30,100),(60,200)]),(100,[(100,333),(130,432)])]:
        for digits,bits in precisions:
            cohort={"target_digits":target,"working_digits":digits,"bits":bits,"rows":{}}
            report["cohorts"].append(cohort)
            arms=("gpmc","prejoin") if target==30 else ("prejoin","gpmc")
            for arm in arms:
                if sha(a.cnf)!=expected:raise ValueError("CNF_DRIFT")
                cmd=([str(a.gpmc.resolve()),"-mode=1",f"-prec={digits}",str(a.cnf.resolve())] if arm=="gpmc" else
                     [str(a.mpfr.resolve()),"--factor-prejoin",str(digits),str(a.cnf.resolve())])
                row=native(cmd,"gpmc" if arm=="gpmc" else "candidate",PAIR_S,digits,expected_bits=bits)
                if row["status"]=="COMPLETED":
                    row["witness"]=compare(row,ref,target)
                    if arm=="prejoin":
                        t=TRACE.findall(row["stderr_tail"])
                        if len(t)==1:
                            residual,cuts,leaves,width,work,peak,joins,plan_ns=map(int,t[0])
                            row["trace"]={"residual":residual,"cuts":cuts,"leaves":leaves,"width":width,
                                          "work":work,"peak":peak,"joins":joins,"plan_ns":plan_ns}
                            row["route_pass"]=(residual<=64 and cuts<=2 and leaves==1<<cuts and
                                               width<=19 and work<=40_000_000 and peak<=2_000_000)
                        else:row["route_pass"]=False
                cohort["rows"][arm]=row
                persist()
                print("PREJOIN_NATIVE_ROW",target,bits,arm,row["status"],row.get("wall_ns"),row.get("witness"),flush=True)
            g,r=(cohort["rows"][k] for k in ("gpmc","prejoin"))
            if any(x["status"]!="COMPLETED" for x in (g,r)) or not r.get("route_pass"):
                cohort["status"]="RESOURCE_OR_ROUTE_INCONCLUSIVE"
            elif not all(x["witness"]["pass"] for x in (g,r)):
                cohort["status"]="ACCURACY_INCONCLUSIVE"
            else:
                cohort.update(status="ONE_MATCHED_PAIR_PASS",gpmc_over_rust=g["wall_ns"]/r["wall_ns"])
            persist()
            print("PREJOIN_NATIVE_FINAL",target,bits,cohort["status"],cohort.get("gpmc_over_rust"),flush=True)
            if cohort["status"]!="ACCURACY_INCONCLUSIVE":break
    passed={c["target_digits"] for c in report["cohorts"] if c["status"]=="ONE_MATCHED_PAIR_PASS"}
    report["status"]="PILOT_COMPLETE" if passed=={30,100} else "PILOT_INCONCLUSIVE"
    persist()
    if report["status"]!="PILOT_COMPLETE":raise SystemExit("PREJOIN_PILOT_INCONCLUSIVE")

if __name__=="__main__":main()
