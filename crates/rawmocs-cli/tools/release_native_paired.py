#!/usr/bin/env python3
"""Release binary versus pinned native GPMC: same-source, same-precision pairs.

Only fresh complete cohorts qualify for a speed ratio. Preserve all attempted
rows, even if a solver exceeds the shared limit or fails the numeric witness.
"""
import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import re
import statistics
import subprocess
import time

from release_transfer import CASES, check_count
from regenerate_release import DIGITS, PIN

GPMC_PIN = "89bbc89d0de918e7a5385740355df55765039230"
NUM = r"[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?"
IMAG = r"[+-](?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?"
COMPLEX = re.compile(rf"^({NUM})({IMAG})i$")
LIMIT = 120
ARMS = ("gpmc", "rawmocs")


def digest(p):
    return sha256(p.read_bytes()).hexdigest()


def parse(stdout, arm):
    sat, = re.findall(r"^s (SATISFIABLE|UNSATISFIABLE)\s*$", stdout, re.M)
    if arm == "gpmc":
        if sat == "UNSATISFIABLE":
            return {"sat": False, "real": "0", "imag": "0"}
        value, = re.findall(r"^c s exact double prec-sci (.+?)\s*$", stdout, re.M)
        match = COMPLEX.fullmatch(value.strip())
        if match is None:
            raise ValueError("invalid GPMC count")
        a, b = match.groups()
    else:
        lines = stdout.splitlines()
        if len(lines) != 2 or lines[0] != "s " + sat:
            raise ValueError("invalid release protocol")
        fields = lines[1].split()
        if len(fields) != 5 or fields[:3] != ["c", "r", "mpfr"]:
            raise ValueError("invalid release count")
        a, b = fields[3:]
    a, b = Fraction(a), Fraction(b)
    if sat == "UNSATISFIABLE" and (a or b):
        raise ValueError("nonzero UNSAT")
    return {"sat": sat == "SATISFIABLE", "real": str(a), "imag": str(b)}


def run(cmd, arm, prefix, bits, digits, ref, target):
    rss = prefix.with_suffix(".rss")
    started = time.perf_counter_ns()
    row = {"arm": arm, "command": cmd, "status": "IN_PROGRESS"}
    try:
        proc = subprocess.run(
            ["/usr/bin/timeout", "-k", "2s", f"{LIMIT}s", "prlimit",
             "--as=3221225472", "--fsize=16777216", "--", "/usr/bin/time",
             "-f", "%M", "-o", str(rss), *cmd],
            capture_output=True, text=True, timeout=LIMIT + 7,
        )
    except subprocess.TimeoutExpired:
        return {**row, "status": "CENSORED_OUTER_TIMEOUT", "wall_ns_lower_bound": LIMIT * 10**9}
    row.update(wall_ns=time.perf_counter_ns() - started, exit_code=proc.returncode)
    prefix.with_suffix(".stdout").write_text(proc.stdout)
    prefix.with_suffix(".stderr").write_text(proc.stderr)
    row["stdout_sha256"] = digest(prefix.with_suffix(".stdout"))
    row["stderr_sha256"] = digest(prefix.with_suffix(".stderr"))
    if rss.exists():
        try:
            row["peak_rss_kib"] = int(rss.read_text().strip().splitlines()[-1])
        except (ValueError, IndexError):
            pass
    if proc.returncode:
        row["status"] = ("CENSORED_TIME" if proc.returncode == 124 else
                         "CENSORED_INTERNAL_TIMEOUT" if arm == "rawmocs" and proc.returncode == 4 else
                         "CENSORED_EXIT_OR_RESOURCE")
        return row
    try:
        row.update(parse(proc.stdout, arm))
        row["witness_upper"] = check_count(row, ref, target)
        assert row.get("peak_rss_kib", 0) > 0
        if arm == "rawmocs":
            assert re.findall(r"^RAWMOCS_COMPLETE route=prejoin digits=(\d+) bits=(\d+) ", proc.stderr, re.M) == [(str(digits), str(bits))]
            trace, = re.findall(r"^PREJOIN_COMPLETE residual=(\d+) cuts=(\d+) leaves=(\d+) width=(\d+) work=(\d+) peak=(\d+) joins=(\d+) plan_ns=(\d+)$", proc.stderr, re.M)
            row["plan_signature"] = list(map(int, trace[:7]))
        row["status"] = "COMPLETED"
    except (ValueError, AssertionError, ZeroDivisionError) as e:
        row["status"] = "NUMERIC_OR_PROTOCOL_INCONCLUSIVE"
        row["error"] = str(e)
    return row


def p90(xs):
    xs = sorted(xs)
    assert len(xs) == 7
    return xs[5] + 0.4 * (xs[6] - xs[5])


def main():
    ap = argparse.ArgumentParser()
    for name in ("corpus", "bin", "gpmc", "gpmc-source", "out"):
        ap.add_argument("--" + name, type=Path, required=True)
    ap.add_argument("--case", required=True, choices=[c[0] for c in CASES])
    a = ap.parse_args()
    assert subprocess.check_output(["git", "-C", str(a.gpmc_source), "rev-parse", "HEAD"], text=True).strip() == GPMC_PIN
    label, _, _, expected, bits = next(c for c in CASES if c[0] == a.case)
    digits, target = DIGITS[bits]
    root = a.corpus / label.rsplit("-", 1)[0]
    cnf, = root.glob("*-gpmc.cnf")
    cnf = cnf.resolve()
    refpath = root / "independent-reference.json"
    ref = json.loads(refpath.read_text())
    regen = json.loads((a.corpus / "regeneration.json").read_text())
    assert regen["status"] == "REGENERATION_COMPLETE" and regen["quprs_revision"] == PIN
    certified, = [x for x in regen["cohorts"] if x["label"] == label]
    assert certified["status"] == "PASS" and certified["bits"] == bits and certified["target"] == target
    assert certified["reference_sha256"] == digest(refpath)
    assert digest(cnf) == expected == ref["source_sha256"]
    assert ref["status"] in ("INDEPENDENT_INTERVAL_COMPLETE", "INDEPENDENT_FIXED_INTERVAL_COMPLETE")
    assert ref["bits"] == 550 and ref["width"] <= 19 and ref["work"] <= 40000000 and ref["peak"] <= 2000000
    if ref["status"] == "INDEPENDENT_FIXED_INTERVAL_COMPLETE":
        assert ref["order_policy"] in ("fill-reverse", "degree-reverse")
    a.out.mkdir(parents=True, exist_ok=True)
    report = {"status": "IN_PROGRESS", "case": label, "source_sha256": expected,
              "reference_sha256": digest(refpath), "quprs_revision": PIN,
              "gpmc_revision": GPMC_PIN, "bits": bits, "working_digits": digits,
              "target_digits": target, "limit_s": LIMIT, "as_bytes": 3221225472,
              "binary_sha256": {"gpmc": digest(a.gpmc), "rawmocs": digest(a.bin)},
              "trials": []}

    def save():
        (a.out / "paired.json").write_text(json.dumps(report, indent=2) + "\n")

    save()
    cmds = {"gpmc": [str(a.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(cnf)],
            "rawmocs": [str(a.bin.resolve()), "--digits", str(digits), "--timeout-ms", str(LIMIT * 1000), str(cnf)]}
    for trial in range(7):
        # Reverse starting arm by cohort; never select the faster order.
        order = ARMS if (trial + (bits == 432)) % 2 == 0 else ARMS[::-1]
        sample = {"trial": trial, "order": order, "rows": {}}
        report["trials"].append(sample)
        for arm in order:
            assert digest(cnf) == expected and digest(refpath) == report["reference_sha256"]
            row = run(cmds[arm], arm, a.out / f"{trial}-{arm}", bits, digits, ref, target)
            sample["rows"][arm] = row
            save()
            print("RELEASE_NATIVE_ROW", label, trial, arm, row["status"], row.get("wall_ns"), flush=True)
        if any(x["status"] != "COMPLETED" for x in sample["rows"].values()):
            report["status"] = "INCONCLUSIVE"
            save()
            raise SystemExit("paired cohort censored or invalid; no speed ratio")
    for arm in ARMS:
        rows = [t["rows"][arm] for t in report["trials"]]
        assert all((r["sat"], r["real"], r["imag"]) == (rows[0]["sat"], rows[0]["real"], rows[0]["imag"]) for r in rows)
        if arm == "rawmocs":
            assert all(r["plan_signature"] == rows[0]["plan_signature"] for r in rows)
    vals = {arm: [t["rows"][arm]["wall_ns"] for t in report["trials"]] for arm in ARMS}
    report["summary"] = {
        arm: {"p50_s": statistics.median(vals[arm]) / 1e9,
              "p90_s": p90(vals[arm]) / 1e9,
              "median_peak_rss_kib": statistics.median(t["rows"][arm]["peak_rss_kib"] for t in report["trials"])}
        for arm in ARMS}
    report["summary"]["gpmc_over_rawmocs_p50"] = statistics.median(vals["gpmc"]) / statistics.median(vals["rawmocs"])
    report["summary"]["paired_ratio_median"] = statistics.median(g / r for g, r in zip(vals["gpmc"], vals["rawmocs"]))
    report["summary"]["rawmocs_paired_wins"] = sum(r < g for g, r in zip(vals["gpmc"], vals["rawmocs"]))
    report["status"] = "SEVEN_MATCHED_PAIRS_PASS"
    save()
    print("RELEASE_NATIVE_FINAL", label, json.dumps(report["summary"], sort_keys=True), flush=True)


if __name__ == "__main__":
    main()
