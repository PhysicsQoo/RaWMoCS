#!/usr/bin/env python3
"""Controlled original-GPMC C++ feature ablation on identical QuPRS weighted CNF.

Research only. Never treat a feature-disabled GPMC run as a RaWMoCS
speedup; the purpose is to isolate algorithmic sources of the native gap.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "rust_core" / "tools"))
from quprs_paired_native import parse_out
from quprs_long_feynman_pilot import numerically_agree

QU_PRS_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
SOURCES = (
    "feynman_hwb6", "feynman_barenco_tof_5",
    "feynman_mod_mult_55", "feynman_grover_5",
)
BITS = {30: 100, 100: 333}
BUDGET_S = 30
AS_BYTES = 2147483648

# All switches are BoolOptions in pinned GPMC core/Config.cc, with
# -no-{name} supported by its MiniSat option parser. No source changes.
ARMS = {
    "gpmc_default": (),
    "no_equivalence": ("-no-pp_ee",),
    "no_variable_elimination": ("-no-pp_ve",),
    "no_clause_strengthening": ("-no-pp_cs",),
    "no_tree_decomposition": ("-no-td",),
    "no_backjump": ("-no-bj",),
    "no_preprocessing": ("-no-pp_ee", "-no-pp_ve", "-no-pp_cs"),
    "no_preprocess_no_td": ("-no-pp_ee", "-no-pp_ve", "-no-pp_cs", "-no-td"),
}


def self_test() -> None:
    assert len(ARMS) == 8 and list(ARMS)[0] == "gpmc_default"
    assert len(set(ARMS)) == len(ARMS)
    assert not ARMS["gpmc_default"]
    valid = {"-no-pp_ee", "-no-pp_ve", "-no-pp_cs", "-no-td", "-no-bj"}
    assert all(set(flags) <= valid and len(set(flags)) == len(flags)
               for flags in ARMS.values())
    assert set(ARMS["no_preprocessing"]) == {
        "-no-pp_ee", "-no-pp_ve", "-no-pp_cs",
    }
    print("GPMC_FEATURE_ARM_MATRIX_OK", flush=True)


def head(path: Path) -> str:
    return subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True,
    ).strip()


def sha(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def percentile(values: list[int], fraction: float) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = int(position)
    return ordered[lower] + (
        ordered[min(lower + 1, len(ordered) - 1)] - ordered[lower]
    ) * (position - lower)


def sample(gpmc: Path, cnf: Path, precision: int, arm: str) -> dict:
    cmd = [
        str(gpmc.resolve()), "-mode=1", f"-prec={precision}",
        *ARMS[arm], str(cnf.resolve()),
    ]
    with tempfile.TemporaryDirectory() as tmp:
        rss = Path(tmp) / "rss"
        timed_cmd = [
            "/usr/bin/timeout", "-k", "2s", f"{BUDGET_S}s",
            "prlimit", f"--as={AS_BYTES}", "--",
            "/usr/bin/time", "-f", "%M", "-o", str(rss), *cmd,
        ]
        start = time.perf_counter_ns()
        try:
            process = subprocess.run(
                timed_cmd, capture_output=True, text=True,
                timeout=BUDGET_S + 7,
            )
        except subprocess.TimeoutExpired:
            return {"arm": arm, "status": "CENSORED_OUTER_TIMEOUT"}
        wall = time.perf_counter_ns() - start
        row = {
            "arm": arm,
            "status": "COMPLETED" if process.returncode == 0 else (
                "CENSORED_TIME" if process.returncode == 124
                else "CENSORED_EXIT"
            ),
            "wall_ns": wall,
            "exit_code": process.returncode,
        }
        if process.returncode:
            row["stderr_tail"] = process.stderr[-500:]
            return row
        try:
            sat, (real, imag) = parse_out(process.stdout, "gpmc")
            row.update({
                "sat": sat,
                "real": str(real),
                "imag": str(imag),
                "peak_rss_kib": int(rss.read_text().strip()),
                "stdout_sha256": sha256(process.stdout.encode()).hexdigest(),
            })
        except (ValueError, OSError) as exc:
            row["status"] = "CENSORED_INVALID_OUTPUT"
            row["error"] = str(exc)[:200]
        return row


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "report"):
        ap.add_argument("--" + name, type=Path)
    ap.add_argument("--trials", type=int, default=7)
    args = ap.parse_args()
    self_test()
    if args.self_test:
        return
    if args.trials != 7:
        raise SystemExit("Requires exactly seven rotated fresh-process trials")
    if any(getattr(args, name.replace("-", "_")) is None for name in
           ("quprs", "gpmc-source", "corpus", "gpmc", "report")):
        ap.error("missing required pinned original-CNF inputs")
    if head(args.quprs) != QU_PRS_SHA or head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("PINNED_SOURCE_REVISION_DRIFT")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "study": "unchanged original GPMC C++ controlled feature ablation",
        "purpose": "GPMC mechanism attribution; NOT RaWMoCS parity evidence",
        "quprs_revision": QU_PRS_SHA,
        "gpmc_revision": GPMC_SHA,
        "boundary": "identical original QuPRS emitted weighted CNF bytes for all arms",
        "precision": "GPMC 30/100 digits (same precision within each cohort); cross-check Rust 100/333 bits separately",
        "resources": {"wall_s": BUDGET_S, "as_bytes": AS_BYTES,
                      "rss": "GNU time maxRSS observed, not strict cgroup RSS"},
        "arms": {name: list(flags) for name, flags in ARMS.items()},
        "rows": [],
    }
    qualified = 0
    for case in SOURCES:
        cnf = args.corpus / f"{case}-gpmc.cnf"
        if not cnf.is_file() or not cnf.read_bytes().startswith(b"p cnf "):
            report["rows"].append({"case": case, "status": "MISSING_OR_INVALID_CNF"})
            continue
        source = {"feynman_hwb6": "hwb6.qasm",
                  "feynman_barenco_tof_5": "barenco_tof_5.qasm",
                  "feynman_mod_mult_55": "mod_mult_55.qasm",
                  "feynman_grover_5": "grover_5.qasm"}[case]
        qasm = args.quprs / "benchmarks" / "Feynman" / source
        if not qasm.is_file():
            report["rows"].append({"case": case, "status": "MISSING_ORIGINAL_QASM"})
            continue
        result = {
            "case": case, "cnf_sha256": sha(cnf), "cnf_bytes": cnf.stat().st_size,
            "source_qasm_sha256": sha(qasm), "precisions": [],
        }
        report["rows"].append(result)
        for digits, bits in BITS.items():
            trials = 1 if case == "feynman_grover_5" else args.trials
            cohort = {"digits": digits, "bits_reference": bits,
                      "status": "INCOMPLETE", "records": []}
            result["precisions"].append(cohort)
            arms = list(ARMS)
            for trial in range(trials):
                rotated = arms[trial % len(arms):] + arms[:trial % len(arms)]
                for arm in rotated:
                    record = sample(args.gpmc, cnf, digits, arm)
                    record["trial"] = trial
                    cohort["records"].append(record)
                    args.report.write_text(json.dumps(report, indent=2) + "\n")
                    print(f"GPMC_ABLATION case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={record['status']}", flush=True)
                if any(r["status"] != "COMPLETED" for r in cohort["records"]):
                    cohort["status"] = "CENSORED_ARM"
                    break
            if cohort["status"] == "CENSORED_ARM":
                args.report.write_text(json.dumps(report, indent=2) + "\n")
                if digits == 30:
                    break
                continue
            if len(cohort["records"]) != trials * len(arms):
                raise SystemExit("UNBALANCED_ABLATION_TRIALS")
            for trial in range(trials):
                trial_records = {r["arm"]: r for r in cohort["records"]
                                 if r["trial"] == trial}
                reference = trial_records["gpmc_default"]
                for arm, row in trial_records.items():
                    if row["sat"] != reference["sat"] or not (
                        numerically_agree(Fraction(row["real"]),
                                          Fraction(reference["real"]), digits) and
                        numerically_agree(Fraction(row["imag"]),
                                          Fraction(reference["imag"]), digits)
                    ):
                        cohort["status"] = "FAILED_SIGNED_COMPLEX_WMC"
                        cohort["failure"] = {"trial": trial, "arm": arm}
                        args.report.write_text(json.dumps(report, indent=2) + "\n")
                        raise SystemExit("FAILED_SIGNED_COMPLEX_WMC")
            cohort["status"] = "FULL_COMPLEX_WMC_AGREEMENT"
            cohort["stats"] = {
                arm: {
                    "p50_wall_ms": statistics.median(
                        r["wall_ns"] for r in cohort["records"] if r["arm"] == arm
                    ) / 1e6,
                    "p90_wall_ms": percentile(
                        [r["wall_ns"] for r in cohort["records"]
                         if r["arm"] == arm], 0.9
                    ) / 1e6,
                    "p50_peak_rss_kib": statistics.median(
                        r["peak_rss_kib"] for r in cohort["records"]
                        if r["arm"] == arm
                    ),
                } for arm in arms
            }
            qualified += 1
            print(f"GPMC_ABLATION_MATCH case={case} bits={bits} " + " ".join(
                f"{arm}_ms={cohort['stats'][arm]['p50_wall_ms']:.3f}"
                for arm in arms
            ), flush=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    report["qualified_cohorts"] = qualified
    report["status"] = ("FINISHED_WITH_CENSORS" if qualified else
                        "NO_QUALIFIED_COHORTS")
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    if qualified < 4:
        raise SystemExit("INSUFFICIENT_QUALIFIED_GPMC_COHORTS")
    print(f"GPMC_ABLATION_DONE qualified={qualified}", flush=True)


if __name__ == "__main__":
    main()
