#!/usr/bin/env python3
"""Exact source/protocol matched native MPFR component-layout A/B.

Research-only dense component-index hypothesis. Same Rust binary, same
precision, same weighted CNF, fresh processes and rotating arm order.
Regressions remain visible; any semantic or source mismatch is censored.
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

CASES = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2")
DIGITS = (30, 100)
BITS = {30: 100, 100: 333}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--matched-reference", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--rawmocs-mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=5)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if not 3 <= args.trials <= 7:
        raise SystemExit("A/B requires between three and seven native trials")
    source = json.loads(args.matched_reference.read_text())
    if source["source_sha"] != "0b28736651b3740ed3b201edd2efdef3da541e0d":
        raise SystemExit("mismatched QuPRS source exporter")
    known = {(r["case"], r["digits"]): r for r in source["rows"]}
    report: dict[str, Any] = {
        "study": "dense MPFR component indexing vs unchanged BTreeMap on identical source CNF",
        "qualification": "same MPFR bits, exact stdout SHA, SAT and MPFR result, solver structural parity",
        "long_cases_first": list(CASES),
        "source_commit": source["source_sha"],
        "rows": [],
    }
    mismatch = False
    for case in CASES:
        for digits in DIGITS:
            qualified = known[(case, digits)]
            if qualified["status"] != "MATCHED_PRECISION_QUALIFIED":
                raise SystemExit(f"unqualified precision pair {case} {digits}")
            filename = args.corpus / f"{case}-gpmc.cnf"
            input_sha = sha256(filename.read_bytes()).hexdigest()
            if input_sha != qualified["source_cnf_sha256"]:
                raise SystemExit("source CNF bytes changed")
            samples = []
            for trial in range(args.trials):
                order = ("baseline", "dense") if trial % 2 == 0 else ("dense", "baseline")
                for arm in order:
                    cmd = [str(args.rawmocs_mpfr.resolve())]
                    if arm == "dense":
                        cmd.append("--dense-components")
                    cmd.extend((str(digits), str(filename.resolve())))
                    result = timed_execution(cmd, "rawmocs_mpfr")
                    result["arm"], result["trial"] = arm, trial
                    samples.append(result)
            row: dict[str, Any] = {
                "case": case, "digits": digits, "bits": BITS[digits],
                "source_cnf_sha256": input_sha,
                "samples": samples,
            }
            if any(x["status"] != "COMPLETED" for x in samples):
                row["status"] = "CENSORED_NATIVE_FAILURE"
            else:
                result_set = {
                    (x["sat"], Fraction(x["real"]), Fraction(x["imag"]))
                    for x in samples
                }
                outputs = {x["native_stdout_sha256"] for x in samples}
                profile = [x.get("mpfr_stats") for x in samples]
                bits = [x.get("mpfr_precision_bits") for x in samples]
                if len(result_set) != 1 or len(outputs) != 1:
                    row["status"] = "CENSORED_RESULT_OR_STDOUT_MISMATCH"
                elif any(x is None or x != profile[0] for x in profile) or any(b != BITS[digits] for b in bits):
                    row["status"] = "CENSORED_STRUCTURAL_MISMATCH"
                else:
                    row["status"] = "SEMANTIC_PARITY_PASS"
                    row["search_structure"] = profile[0]
                    row["wall_ns_median"] = {
                        arm: statistics.median(
                            x["wall_ns"] for x in samples if x["arm"] == arm
                        ) for arm in ("baseline", "dense")
                    }
                    row["peak_rss_kib_median"] = {
                        arm: statistics.median(
                            x["peak_rss_kib"] for x in samples if x["arm"] == arm
                        ) for arm in ("baseline", "dense")
                    }
                    row["internal_solve_ns_median"] = {
                        arm: statistics.median(
                            x["mpfr_phase_ns"]["solve"]
                            for x in samples if x["arm"] == arm
                        ) for arm in ("baseline", "dense")
                    }
                    row["speed_ratio_baseline_over_dense"] = (
                        row["wall_ns_median"]["baseline"] / row["wall_ns_median"]["dense"]
                    )
                    print(
                        f"MPFR_DENSE_AB case={case} digits={digits} status=SEMANTIC_PARITY_PASS "
                        f"baseline_ms={row['wall_ns_median']['baseline']/1e6:.3f} "
                        f"dense_ms={row['wall_ns_median']['dense']/1e6:.3f} "
                        f"speedup={row['speed_ratio_baseline_over_dense']:.4f}"
                    )
            if row["status"] != "SEMANTIC_PARITY_PASS":
                mismatch = True
                print(f"MPFR_DENSE_AB_CENSORED case={case} digits={digits} status={row['status']}")
            report["rows"].append(row)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if mismatch:
        raise SystemExit("component-layout A/B semantic parity failed")


if __name__ == "__main__":
    main()
