"""Bounded matched QPE pairs, independently certified; guarded repeats opt-in."""
import argparse
import json
import re
import statistics
from pathlib import Path
from original_fixed_interval import compare
from quprs_long_tail_paired import native, sha, PAIR_S

TRACE = re.compile(r"^PREJOIN_COMPLETE residual=(\d+) cuts=(\d+) leaves=(\d+) width=(\d+) work=(\d+) peak=(\d+) joins=(\d+) plan_ns=(\d+)$", re.M)

def main():
    ap=argparse.ArgumentParser()
    for name in ("cnf","reference","gpmc","mpfr","report"):
        ap.add_argument("--"+name,type=Path,required=True)
    repeats=ap.add_mutually_exclusive_group()
    repeats.add_argument("--repeat-target",type=int,choices=[30,100])
    repeats.add_argument("--repeat-fixed-target",type=int,choices=[30,100],
                         help="Repeat an already qualified original 100/333-bit cohort without precision drift")
    a=ap.parse_args()
    ref=json.loads(a.reference.read_text())
    expected=ref["source_sha256"]
    if (sha(a.cnf)!=expected or ref["status"]!="INDEPENDENT_INTERVAL_COMPLETE" or ref["bits"]!=550
        or ref["width"]>19 or ref["work"]>40_000_000 or ref["peak"]>2_000_000):
        raise ValueError("REFERENCE_DRIFT")
    fixed=a.repeat_fixed_target is not None
    repeat=fixed or a.repeat_target is not None
    count=7 if repeat else 1
    targets=([a.repeat_fixed_target if fixed else a.repeat_target] if repeat else [30,100])
    report={"status":"IN_PROGRESS","source_sha256":expected,"reference_sha256":sha(a.reference),
            "gpmc_binary_sha256":sha(a.gpmc),"rust_binary_sha256":sha(a.mpfr),
            "repeat_count":count,"repeat_precision_policy":"fixed" if fixed else "guarded" if repeat else "pilot","cohorts":[]}
    def persist():
        a.report.write_text(json.dumps(report,indent=2)+"\n")
    persist()
    for target in targets:
        guarded=(60,200) if target==30 else (130,432)
        original=(target,100 if target==30 else 333)
        precisions=([original] if fixed else [guarded] if repeat else [original,guarded])
        for digits,bits in precisions:
            cohort={"target_digits":target,"working_digits":digits,"bits":bits,"trials":[]}
            report["cohorts"].append(cohort)
            signatures=[]
            for trial in range(count):
                rows={}
                arms=("gpmc","prejoin") if (trial+(target==100))%2==0 else ("prejoin","gpmc")
                cohort["trials"].append({"trial":trial,"order":arms,"rows":rows})
                for arm in arms:
                    if sha(a.cnf)!=expected or sha(a.reference)!=report["reference_sha256"]:
                        raise ValueError("INPUT_DRIFT")
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
                    rows[arm]=row
                    persist()
                    print("PREJOIN_NATIVE_ROW",target,bits,trial,arm,row["status"],row.get("wall_ns"),row.get("witness",{}).get("pass"),flush=True)
                g,r=(rows[k] for k in ("gpmc","prejoin"))
                if any(x["status"]!="COMPLETED" for x in (g,r)) or not r.get("route_pass"):
                    cohort["status"]="RESOURCE_OR_ROUTE_INCONCLUSIVE";break
                if not all(x["witness"]["pass"] for x in (g,r)):
                    cohort["status"]="ACCURACY_INCONCLUSIVE";break
                if not all(x.get("peak_rss_kib",0)>0 for x in (g,r)):
                    cohort["status"]="RSS_INCONCLUSIVE";break
                sig=tuple(r["trace"][k] for k in ("residual","cuts","leaves","width","work","peak","joins"))
                signatures.append(sig)
                if len(set(signatures))!=1:
                    cohort["status"]="PLAN_DRIFT";break
            else:
                def values(arm,key):
                    return [t["rows"][arm][key] for t in cohort["trials"]]
                def p90(xs):
                    xs=sorted(xs)
                    return xs[0] if len(xs)==1 else xs[5]+0.4*(xs[6]-xs[5])
                g,r=values("gpmc","wall_ns"),values("prejoin","wall_ns")
                cohort.update(status="SEVEN_MATCHED_PAIRS_PASS" if repeat else "ONE_MATCHED_PAIR_PASS",
                    gpmc_median_s=statistics.median(g)/1e9,rust_median_s=statistics.median(r)/1e9,
                    gpmc_p90_s=p90(g)/1e9,rust_p90_s=p90(r)/1e9,
                    gpmc_over_rust=statistics.median(g)/statistics.median(r),
                    paired_ratio_median=statistics.median(a/b for a,b in zip(g,r)),
                    paired_rust_wins=sum(b<a for a,b in zip(g,r)),
                    median_rss_kib={arm:statistics.median(values(arm,"peak_rss_kib")) for arm in ("gpmc","prejoin")},
                    plan_signature=signatures[0])
            persist()
            print("PREJOIN_NATIVE_FINAL",target,bits,cohort["status"],
                  {k:cohort.get(k) for k in ("gpmc_median_s","rust_median_s","gpmc_p90_s","rust_p90_s","gpmc_over_rust","paired_ratio_median","median_rss_kib","plan_signature")},flush=True)
            if cohort["status"]!="ACCURACY_INCONCLUSIVE":break
    ok="SEVEN_MATCHED_PAIRS_PASS" if repeat else "ONE_MATCHED_PAIR_PASS"
    passed={c["target_digits"] for c in report["cohorts"] if c["status"]==ok}
    report["status"]=("REPEAT_COMPLETE" if repeat else "PILOT_COMPLETE") if passed==set(targets) else "INCONCLUSIVE"
    persist()
    if report["status"]=="INCONCLUSIVE":raise SystemExit("PREJOIN_NATIVE_INCONCLUSIVE")

if __name__=="__main__":main()
