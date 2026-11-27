#!/usr/bin/env python3
"""R6: compare each requested route with the exact published v0.2.0 CLI.
Process wall/RSS includes input loading and output; solve_ns is not the gate.
Reuse the independent witness, resource guard and prejoin plan parser.
"""
import argparse
import json
from pathlib import Path
import statistics
import subprocess

from regenerate_release import DIGITS, PIN
from release_native_paired import digest, p90, run
from release_transfer import CASES

BASELINE = "e553bc7bd739ca912036d8fa9f3a1ddfe8d8859c"
ARMS = ("baseline", "cli", "file", "text")
LABELS = ("qpeinexact22-432", "grover6-432", "gf2-7-333")
MODES = ("auto", "auto-frontier")


def main():
    ap = argparse.ArgumentParser()
    for name in ("corpus", "baseline-source", "baseline", "bin", "probe", "out"):
        ap.add_argument("--" + name, type=Path, required=True)
    a = ap.parse_args()
    assert subprocess.check_output(
        ["git", "-C", str(a.baseline_source), "rev-parse", "HEAD"], text=True
    ).strip() == BASELINE
    assert subprocess.check_output([str(a.baseline.resolve()), "--version"], text=True).strip() == "rawmocs 0.2.0"
    regen = json.loads((a.corpus / "regeneration.json").read_text())
    assert regen["status"] == "REGENERATION_COMPLETE" and regen["quprs_revision"] == PIN
    a.out.mkdir(parents=True, exist_ok=True)
    report = {
        "status": "IN_PROGRESS", "baseline_revision": BASELINE, "quprs_revision": PIN,
        "binaries": {k: digest(v) for k, v in (
            ("baseline", a.baseline), ("cli", a.bin), ("native_probe", a.probe)
        )},
        "wall_regression_limit": 1.05, "rss_policy": "report_only",
        "wall_scope": "fresh process including loading and output", "cohorts": [],
    }

    def save():
        (a.out / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    save()
    admitted = True
    for label in LABELS:
        _, _, _, expected, bits = next(c for c in CASES if c[0] == label)
        digits, target = DIGITS[bits]
        root = a.corpus / label.rsplit("-", 1)[0]
        cnf, = root.glob("*-gpmc.cnf")
        cnf = cnf.resolve()
        refpath = root / "independent-reference.json"
        ref = json.loads(refpath.read_text())
        certified, = [c for c in regen["cohorts"] if c["label"] == label]
        assert certified["status"] == "PASS" and certified["bits"] == bits and certified["target"] == target
        assert certified["reference_sha256"] == digest(refpath)
        assert digest(cnf) == expected == ref["source_sha256"]
        assert ref["status"] in ("INDEPENDENT_INTERVAL_COMPLETE", "INDEPENDENT_FIXED_INTERVAL_COMPLETE")
        assert ref["bits"] == 550 and ref["width"] <= 19 and ref["work"] <= 40000000 and ref["peak"] <= 2000000
        if ref["status"] == "INDEPENDENT_FIXED_INTERVAL_COMPLETE":
            assert ref["order_policy"] in ("fill-reverse", "degree-reverse")
        for mode in MODES:
            directory = a.out / label / mode
            directory.mkdir(parents=True, exist_ok=True)
            cohort = {"label": label, "mode": mode, "bits": bits, "digits": digits,
                      "source_sha256": expected, "reference_sha256": digest(refpath),
                      "trials": [], "status": "IN_PROGRESS"}
            report["cohorts"].append(cohort)
            save()
            cli_args = ["--digits", str(digits), "--timeout-ms", "120000", "--solver", mode, str(cnf)]
            cmds = {"baseline": [str(a.baseline.resolve()), *cli_args],
                    "cli": [str(a.bin.resolve()), *cli_args]}
            for transport in ("file", "text"):
                cmds[transport] = [str(a.probe.resolve()), str(cnf), transport, mode, str(digits)]
            orders = [base[i:] + base[:i] for base in (ARMS, ARMS[::-1]) for i in range(4)]
            for trial in range(7):
                order = orders[(trial + (bits == 432)) % len(orders)]
                sample = {"trial": trial, "order": order, "rows": {}}
                cohort["trials"].append(sample)
                for arm in order:
                    assert digest(cnf) == expected and digest(refpath) == cohort["reference_sha256"]
                    row = run(cmds[arm], arm, directory / f"{trial}-{arm}", bits, digits, ref, target)
                    sample["rows"][arm] = row
                    save()
                    print("R6_NATIVE_ROW", label, mode, trial, arm, row["status"], row.get("wall_ns"), flush=True)
                if any(row["status"] != "COMPLETED" for row in sample["rows"].values()):
                    cohort["status"] = "INCONCLUSIVE"
                    break
            rows = [row for t in cohort["trials"] for row in t["rows"].values()]
            if len(rows) != 28 or any(row["status"] != "COMPLETED" for row in rows):
                admitted = False
                cohort["status"] = "INCONCLUSIVE"
                save()
                continue
            old = cohort["trials"][0]["rows"]["baseline"]
            parity = all(row["stdout_sha256"] == old["stdout_sha256"] and
                         row["plan_signature"] == old["plan_signature"] for row in rows)
            cohort["exact_cli_count_and_plan_parity"] = parity
            summary = {}
            for arm in ARMS:
                samples = [t["rows"][arm] for t in cohort["trials"]]
                ns = [s["wall_ns"] for s in samples]
                summary[arm] = {"p50_s": statistics.median(ns) / 1e9,
                                "p90_s": p90(ns) / 1e9,
                                "median_peak_rss_kib": statistics.median(s["peak_rss_kib"] for s in samples)}
            cohort["summary"] = summary
            ratios = {arm: {metric: summary[arm][metric] / summary["baseline"][metric]
                            for metric in ("p50_s", "p90_s", "median_peak_rss_kib")} for arm in ARMS[1:]}
            cohort["ratios_to_v020_same_mode"] = ratios
            passed = parity and all(r[metric] <= 1.05 for r in ratios.values() for metric in ("p50_s", "p90_s"))
            cohort["status"] = "PASS" if passed else "REGRESSION_OR_PARITY_FAIL"
            admitted &= passed
            save()
            print("R6_NATIVE_COHORT", label, mode, cohort["status"], json.dumps(ratios), flush=True)
    report["status"] = "PASS" if admitted else "NOT_ADMITTED"
    save()
    if not admitted:
        raise SystemExit("R6: regression or incomplete evidence")
    print("R6_NATIVE_ADMISSION_PASS 168/168", flush=True)


if __name__ == "__main__":
    main()
