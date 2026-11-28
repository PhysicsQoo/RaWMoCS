#!/usr/bin/env python3
"""R6-Light: bounded same-family current-transport correctness, not performance."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from small_family_qualification import BASELINE, CASES, PIN, bounded, digest, git, validate_trial
from regenerate_release import DIGITS

LABELS = ("qpeinexact8", "grover4", "gf2_4")
MODES = ("auto", "auto-frontier")
ARMS = ("cli", "file", "text")


def verify_parity(rows):
    if set(rows) != set(ARMS):
        raise ValueError("incomplete transport arms")
    first = rows["cli"]
    for arm in ARMS:
        row = rows[arm]
        if row["status"] != "PASS":
            raise ValueError("failed transport arm")
        if any(row[k] != first[k] for k in ("stdout_sha256", "plan_signature", "count")):
            raise ValueError("transport count or plan mismatch")


def run_case(a, label):
    _, rel, blob = next(x for x in CASES if x[0] == label)
    qasm_sha, cnf_sha, plan = BASELINE[label]
    root = a.out / label
    root.mkdir()
    assert git(a.quprs, "rev-parse", "HEAD:" + rel) == blob
    stem = "extended_v022_" + label
    export, _ = bounded([a.exporter, stem, a.quprs, root, rel], 20, root, "export")
    assert export.returncode == 0, "source exporter failed"
    qasm, cnf = root / (stem + ".qasm"), root / (stem + "-gpmc.cnf")
    assert qasm.read_bytes() == (a.quprs / rel).read_bytes()
    assert digest(qasm) == qasm_sha and digest(cnf) == cnf_sha, "source drift"
    oracle = Path(__file__).resolve().parent / "repro" / "original_fixed_interval.py"
    result, _ = bounded([sys.executable, oracle, cnf, cnf_sha], 25, root, "oracle")
    assert result.returncode == 0, "independent oracle failed"
    ref = json.loads(result.stdout)
    assert ref["source_sha256"] == cnf_sha and ref["bits"] == 550
    assert ref["order_policy"] in ("fill-reverse", "degree-reverse")
    assert ref["width"] <= 19 and ref["work"] <= 40000000 and ref["peak"] <= 2000000
    refpath = root / "independent-reference.json"
    refpath.write_text(json.dumps(ref, indent=2) + "\n")
    reference_sha = digest(refpath)
    bits = 432 if label != "gf2_4" else 333
    digits, target = DIGITS[bits]
    report = {"label": label, "source_sha256": cnf_sha, "qasm_sha256": qasm_sha,
              "reference_sha256": reference_sha, "bits": bits, "digits": digits,
              "target_digits": target, "cohorts": []}
    for mode in MODES:
        rows = {}
        cmds = {"cli": [a.bin, "--digits", digits, "--solver", mode, "--timeout-ms", 5000, cnf],
                **{arm: [a.probe, cnf, arm, mode, digits] for arm in ("file", "text")}}
        for arm in ARMS:
            name = mode + "-" + arm
            proc, elapsed = bounded(cmds[arm], 8, root, name)
            assert proc.returncode == 0, ("transport failed", label, mode, arm, proc.returncode)
            route, count, upper, signature, _ = validate_trial(
                proc.stdout, proc.stderr, digits, bits, target, ref, plan)
            assert digest(cnf) == cnf_sha and digest(refpath) == reference_sha
            rows[arm] = {"status": "PASS", "route": route, "count": count,
                         "witness_upper": upper, "plan_signature": signature,
                         "stdout_sha256": digest(root / (name + ".stdout")),
                         "wall_ns_diagnostic_only": elapsed}
        verify_parity(rows)
        report["cohorts"].append({"mode": mode, "rows": rows, "status": "PASS"})
        print("R6_LIGHT_COHORT_PASS", label, mode, flush=True)
    return report


def main():
    p = argparse.ArgumentParser()
    for name in ("quprs", "exporter", "bin", "probe", "out"):
        p.add_argument("--" + name, type=Path, required=True)
    a = p.parse_args()
    for name in ("quprs", "exporter", "bin", "probe", "out"):
        setattr(a, name, getattr(a, name).resolve())
    assert git(a.quprs, "rev-parse", "HEAD") == PIN
    a.out.mkdir(parents=True, exist_ok=False)
    report = {"status": "IN_PROGRESS", "revision": git(Path("."), "rev-parse", "HEAD"),
              "quprs_revision": PIN, "binary_sha256": digest(a.bin),
              "probe_sha256": digest(a.probe), "exporter_sha256": digest(a.exporter),
              "performance_admitted": False, "release_authorized": False, "cases": []}
    def save():
        (a.out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    save()
    try:
        for label in LABELS:
            report["cases"].append(run_case(a, label))
            save()
    except (AssertionError, ValueError, subprocess.SubprocessError, OSError) as exc:
        report.update(status="NOT_ADMITTED", error=str(exc))
        save()
        raise
    assert sum(len(c["rows"]) for case in report["cases"] for c in case["cohorts"]) == 18
    report["status"] = "R6_LIGHT_PASS"
    save()
    print("R6_LIGHT_PASS 18/18; numeric parity only", flush=True)


if __name__ == "__main__":
    main()
