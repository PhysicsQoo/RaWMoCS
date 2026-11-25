#!/usr/bin/env python3
"""Release transfer gate: previously qualified source bytes and independent interval.
Not a new same-runner GPMC benchmark. Never treat a resource stop as a count.
"""
import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time

# Original independently checked, seven-pair research cohorts.
CASES = (
    ("qpeinexact22-200", 45561414435, "original-prejoin-frontier-numeric-repeat-0", "6fb69626195bee5b89ed3503ae8b1b4801d9267e7bd5ff3684af152afcd885e1", 200),
    ("qpeinexact22-432", 45561414435, "original-prejoin-frontier-numeric-repeat-1", "6fb69626195bee5b89ed3503ae8b1b4801d9267e7bd5ff3684af152afcd885e1", 432),
    ("qpeexact22-432", 45561414435, "original-prejoin-frontier-numeric-repeat-3", "00313f034134215fa7bd48a8667f2b55f864a13940f9be5cff72ae09231abf25", 432),
    ("qnn12-432", 45556128434, "original-prejoin-unseen-certified-repeat-3", "079f86a832056412d015759dd20a556a998ab4d596bea774a9ff5ebaed3484b7", 432),
    ("grover6-432", 45551765787, "original-incremental-grover-repeat-1", "a9757cd5bd730b6f769d2970e3a16ac66fee7157b55f81208c0a7c2cece0023b", 432),
    ("wstate16-432", 45553233760, "original-prejoin-cross-family-ab-2", "600ce6716a69e31e17ffbc571cba581fc58a5b5ed3b7d60227ad41d857374efe", 432),
    ("gf2-7-333", 45553233760, "original-prejoin-cross-family-ab-0", "d9f60f1ee2a85ec56b5157a65535778ed795d2486cea06e503d8aa202e614ac1", 333),
)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def unique(root, name):
    p, = root.rglob(name)
    return p

def check_count(row, ref, target):
    r = list(map(Fraction, ref["real"]))
    i = list(map(Fraction, ref["imag"]))
    a, b = Fraction(row["real"]), Fraction(row["imag"])
    scale = max(Fraction(1), abs(a), abs(b),
                min(abs(r[0]), abs(r[1])) if r[0]*r[1]>0 else Fraction(0),
                min(abs(i[0]), abs(i[1])) if i[0]*i[1]>0 else Fraction(0))
    upper = max(abs(a-r[0]), abs(a-r[1]), abs(b-i[0]), abs(b-i[1]))/scale
    assert row["sat"] == ref["sat"] and upper <= Fraction(1, 10**(target-6))
    return str(upper)

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--bin", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--repo", required=True)
    a=p.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    report={"status":"IN_PROGRESS", "binary_sha256":sha(a.bin), "cohorts":[]}
    def save():
        (a.out/"release-transfer.json").write_text(json.dumps(report,indent=2)+"\n")
    save()
    for name,run,artifact,expected,bits in CASES:
        root=a.out/name
        root.mkdir()
        subprocess.run(["gh","run","download",str(run),"--repo",a.repo,
                        "--name",artifact,"--dir",str(root)],check=True,timeout=90)
        receipt=json.loads(unique(root,"prejoin-native-pilot.json").read_text())
        refpath=unique(root,"prejoin-reference.json")
        ref=json.loads(refpath.read_text())
        assert receipt["source_sha256"]==ref["source_sha256"]==expected
        assert receipt["reference_sha256"]==sha(refpath)
        assert receipt["status"]=="REPEAT_COMPLETE"
        assert ref["status"] in ("INDEPENDENT_INTERVAL_COMPLETE","INDEPENDENT_FIXED_INTERVAL_COMPLETE")
        assert ref["bits"]==550 and ref["width"]<=19 and ref["work"]<=40000000 and ref["peak"]<=2000000
        if ref["status"]=="INDEPENDENT_FIXED_INTERVAL_COMPLETE":
            assert ref["order_policy"] in ("fill-reverse","degree-reverse")
        cohort,=[c for c in receipt["cohorts"] if c["bits"]==bits]
        assert cohort["status"]=="SEVEN_MATCHED_PAIRS_PASS" and len(cohort["trials"])==7
        expected_row=cohort["trials"][0]["rows"]["prejoin"]
        for t in cohort["trials"]:
            for arm in ("prejoin","gpmc"):
                assert t["rows"][arm]["status"]=="COMPLETED" and t["rows"][arm]["witness"]["pass"]
            assert t["rows"]["prejoin"]["route_pass"]
            assert all(t["rows"]["prejoin"][k]==expected_row[k] for k in ("sat","real","imag"))
        cnf,=[x for x in root.rglob("*-gpmc.cnf") if sha(x)==expected]
        digits=cohort["working_digits"]
        target=cohort["target_digits"]
        start=time.monotonic_ns()
        r=subprocess.run(["/usr/bin/time","-v","-o",str(root/"candidate.resources"),
                          "timeout","-k","2s","60s","prlimit","--as=3221225472","--",
                          str(a.bin.resolve()),"--digits",str(digits),str(cnf.resolve())],
                          text=True,capture_output=True,timeout=65)
        wall_ns=time.monotonic_ns()-start
        (root/"candidate.stdout").write_text(r.stdout)
        (root/"candidate.stderr").write_text(r.stderr)
        assert r.returncode==0, (name,r.returncode,r.stderr[-500:])
        lines=r.stdout.splitlines()
        assert len(lines)==2 and lines[0] in ("s SATISFIABLE","s UNSATISFIABLE")
        f=lines[1].split()
        assert len(f)==5 and f[:3]==["c","r","mpfr"]
        row={"sat":lines[0]=="s SATISFIABLE","real":f[3],"imag":f[4]}
        assert row["sat"]==expected_row["sat"]
        assert all(Fraction(row[k])==Fraction(expected_row[k]) for k in ("real","imag"))
        trace=re.findall(r"^PREJOIN_COMPLETE residual=(\d+) cuts=(\d+) leaves=(\d+) width=(\d+) work=(\d+) peak=(\d+) joins=(\d+) plan_ns=(\d+)$",r.stderr,re.M)
        assert len(trace)==1
        assert list(map(int,trace[0][:7]))==cohort["plan_signature"]
        assert f"RAWMOCS_COMPLETE route=prejoin digits={digits} bits={bits} " in r.stderr
        bound=check_count(row,ref,target)
        assert sha(cnf)==expected and sha(refpath)==receipt["reference_sha256"]
        report["cohorts"].append(dict(case=name,source_sha256=expected,bits=bits,target=target,
            plan_signature=cohort["plan_signature"],witness_upper=bound,wall_ns=wall_ns,
            research_output_equal=True,status="TRANSFER_PASS"))
        save()
        print("RELEASE_TRANSFER_PASS",name,bits,wall_ns,cohort["plan_signature"],flush=True)
    report["status"]="TRANSFER_COMPLETE"
    save()
    print("RELEASE_TRANSFER_COMPLETE",len(report["cohorts"]),flush=True)

if __name__=="__main__":
    main()
