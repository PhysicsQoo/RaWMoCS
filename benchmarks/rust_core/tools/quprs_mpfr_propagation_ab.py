#!/usr/bin/env python3
"""Long QuPRS source-pinned MPFR unit scan/assignment ablation.

Four search-identical paths execute in the same native binary at the
same MPFR precision and CNF; only optional loop/assignment mechanics differ.
Each case retains regressions and fails closed on any semantic mismatch.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics
from typing import Any

from quprs_representative_native import timed_execution

CASES = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2", "qft3")
PRECISIONS = (30, 100)
BITS = {30: 100, 100: 333}
ARMS = ("baseline", "fused_units", "fast_assign", "combined")
ARGS = {
    "baseline": (),
    "fused_units": ("--fused-units",),
    "fast_assign": ("--fast-assign",),
    "combined": ("--fast-both",),
}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--matched-reference", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--rawmocs-mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=5)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if not 3 <= args.trials <= 7:
        raise SystemExit("opt-in A/B requires between three and seven same-runner trials")
    reference = json.loads(args.matched_reference.read_text())
    if reference["source_sha"] != "0b28736651b3740ed3b201edd2efdef3da541e0d":
        raise SystemExit("original QuPRS source drift")
    qualified = {(r["case"], r["digits"]): r for r in reference["rows"]}
    report: dict[str, Any] = {
        "study": "isolated Rust MPFR fused unit scan and binary-clause assignment",
        "source_sha": reference["source_sha"],
        "cohort_priority": list(CASES),
        "precision_bits": BITS,
        "arms": list(ARMS),
        "numeric_contract": "same MPFR bits and exact stdout across all arms; no GPMC-vs-exact speed ratio",
        "walltime_contract": "same native executable and source CNF, fresh process including startup/read/parse/solve/output",
        "rows": [],
    }
    invalid = False
    for case in CASES:
        for digits in PRECISIONS:
            expected = qualified[(case, digits)]
            if expected["status"] != "MATCHED_PRECISION_QUALIFIED":
                raise SystemExit(f"{case} {digits}: matched MPFR reference is unqualified")
            cnf = args.corpus / f"{case}-gpmc.cnf"
            source_sha = sha256(cnf.read_bytes()).hexdigest()
            if source_sha != expected["source_cnf_sha256"]:
                raise SystemExit(f"{case}: source weighted-CNF bytes drifted")
            samples = []
            for trial in range(args.trials):
                order = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in order:
                    command = [
                        str(args.rawmocs_mpfr.resolve()),
                        *ARGS[arm], str(digits), str(cnf.resolve()),
                    ]
                    sample = timed_execution(command, "rawmocs_mpfr")
                    sample["trial"], sample["arm"] = trial, arm
                    samples.append(sample)
            row: dict[str, Any] = {
                "case": case, "digits": digits, "bits": BITS[digits],
                "source_cnf_sha256": source_sha, "samples": samples,
            }
            if any(s["status"] != "COMPLETED" for s in samples):
                row["status"] = "CENSORED_NATIVE_FAILURE"
            else:
                outputs = {s["native_stdout_sha256"] for s in samples}
                results = {
                    (s["sat"], Fraction(s["real"]), Fraction(s["imag"]))
                    for s in samples
                }
                structures = [s.get("mpfr_stats") for s in samples]
                precisions = [s.get("mpfr_precision_bits") for s in samples]
                phases = [s.get("mpfr_phase_ns") for s in samples]
                if len(outputs) != 1 or len(results) != 1:
                    row["status"] = "CENSORED_NUMERIC_OR_OUTPUT_MISMATCH"
                elif any(x is None or x != structures[0] for x in structures):
                    row["status"] = "CENSORED_STRUCTURE_MISMATCH"
                elif any(x != BITS[digits] for x in precisions) or any(p is None for p in phases):
                    row["status"] = "CENSORED_PRECISION_OR_PHASE_MISMATCH"
                else:
                    row["status"] = "SEMANTIC_PARITY_PASS"
                    row["search_structure"] = structures[0]
                    row["wall_ns_median"] = {
                        arm: statistics.median(
                            s["wall_ns"] for s in samples if s["arm"] == arm
                        ) for arm in ARMS
                    }
                    row["solve_ns_median"] = {
                        arm: statistics.median(
                            s["mpfr_phase_ns"]["solve"] for s in samples if s["arm"] == arm
                        ) for arm in ARMS
                    }
                    row["peak_rss_kib_median"] = {
                        arm: statistics.median(
                            s["peak_rss_kib"] for s in samples if s["arm"] == arm
                        ) for arm in ARMS
                    }
                    row["ratios_baseline_over_arm"] = {
                        arm: row["wall_ns_median"]["baseline"] / row["wall_ns_median"][arm]
                        for arm in ARMS
                    }
                    print(
                        f"MPFR_PROPAGATION_AB case={case} digits={digits} "
                        "status=SEMANTIC_PARITY_PASS "
                        + " ".join(
                            f"{arm}_ms={row['wall_ns_median'][arm] / 1e6:.3f}"
                            for arm in ARMS
                        )
                    )
            if row["status"] != "SEMANTIC_PARITY_PASS":
                invalid = True
                print(f"MPFR_PROPAGATION_CENSORED case={case} digits={digits} status={row['status']}")
            report["rows"].append(row)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if invalid:
        raise SystemExit("propagation/assignment ablation has unqualified cases")


if __name__ == "__main__":
    main()
