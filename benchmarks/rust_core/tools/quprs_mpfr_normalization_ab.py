#!/usr/bin/env python3
"""Source-pinned native MPFR residual canonicalization A/B.

All arms use the same binary, 100/333-bit precision, source CNF bytes,
Borrowed Cache and fast binary assignment. Output/provenance parity is a
hard admission gate; any slower cases and failures remain archived.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics

from quprs_representative_native import timed_execution

CASES = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2", "qft3")
PRECISIONS = {30: 100, 100: 333}
ARMS = ("legacy", "unstable", "check_sorted")
FLAGS = {
    "legacy": (),
    "unstable": ("--norm-unstable",),
    "check_sorted": ("--norm-check-sorted",),
}
SOURCE = "0b28736651b3740ed3b201edd2efdef3da541e0d"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-report", type=Path, required=True)
    parser.add_argument("--matched-reference", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--mpfr", type=Path, required=True)
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not 3 <= args.trials <= 7:
        raise SystemExit("must use 3-7 alternating independent native trials")
    source = json.loads(args.source_report.read_text())
    matched = json.loads(args.matched_reference.read_text())
    if source["source_commit"] != SOURCE or matched["source_sha"] != SOURCE:
        raise SystemExit("original QuPRS exporter source changed")
    source_rows = {row["case"]: row for row in source["rows"]}
    matched_rows = {(row["case"], row["digits"]): row for row in matched["rows"]}
    report = {
        "study": "MPFR residual clause normalization source-locked A/B",
        "source_sha": SOURCE,
        "contract": "source-identical same-bit MPFR, exact byte-equal output, SAT and search-structure parity",
        "controls": "same native binary and GPMC-qualified source, Borrowed Cache and binary fast assignment",
        "predeclared_long_cases": list(CASES),
        "precision_digits_to_bits": PRECISIONS,
        "trials": args.trials,
        "rows": [],
    }
    censored = False
    for case in CASES:
        original = source_rows[case]
        cnf = args.corpus / f"{case}-gpmc.cnf"
        digest = sha256(cnf.read_bytes()).hexdigest()
        if digest != original["cnf_sha256"]:
            raise SystemExit(f"{case}: source CNF byte SHA mismatch")
        for digits, bits in PRECISIONS.items():
            reference = matched_rows[(case, digits)]
            if reference["status"] != "MATCHED_PRECISION_QUALIFIED":
                raise SystemExit(f"{case}: original GPMC/Rust precision contract unqualified")
            if reference["source_cnf_sha256"] != digest:
                raise SystemExit(f"{case}: matched-precision source SHA mismatch")
            samples = []
            for trial in range(args.trials):
                order = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in order:
                    command = [
                        str(args.mpfr.resolve()), *FLAGS[arm],
                        "--fast-assign", "--hashbrown-borrowed",
                        str(digits), str(cnf.resolve()),
                    ]
                    record = timed_execution(command, "rawmocs_mpfr")
                    record["arm"], record["trial"] = arm, trial
                    samples.append(record)
            row = {
                "case": case, "digits": digits, "bits": bits,
                "source_cnf_sha256": digest, "source_qasm_sha256": original["qasm_sha256"],
                "samples": samples,
            }
            statuses = {sample["status"] for sample in samples}
            if statuses != {"COMPLETED"}:
                row["status"] = "CENSORED_NATIVE_FAILURE"
            else:
                outputs = {sample["native_stdout_sha256"] for sample in samples}
                values = {
                    (sample["sat"], Fraction(sample["real"]), Fraction(sample["imag"]))
                    for sample in samples
                }
                search = [sample.get("mpfr_stats") for sample in samples]
                profiles = [sample.get("mpfr_phase_ns") for sample in samples]
                precision = [sample.get("mpfr_precision_bits") for sample in samples]
                metrics = [sample.get("mpfr_normalization") for sample in samples]
                if len(outputs) != 1 or len(values) != 1:
                    row["status"] = "CENSORED_OUTPUT_OR_WMC_DIVERGENCE"
                elif any(s is None or s != search[0] for s in search):
                    row["status"] = "CENSORED_SEARCH_STRUCTURE_DIVERGENCE"
                elif any(p != bits for p in precision) or any(p is None for p in profiles):
                    row["status"] = "CENSORED_PRECISION_OR_PROFILE_MISSING"
                elif any(m is None for m in metrics):
                    row["status"] = "CENSORED_NORMALIZATION_METRIC_MISSING"
                else:
                    row["status"] = "SEMANTIC_PARITY_PASS"
                    row["search_stats"] = search[0]
                    row["wall_ns_median"] = {
                        arm: statistics.median(s["wall_ns"] for s in samples if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["solve_ns_median"] = {
                        arm: statistics.median(s["mpfr_phase_ns"]["solve"] for s in samples if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["peak_rss_kib_median"] = {
                        arm: statistics.median(s["peak_rss_kib"] for s in samples if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["normalization_calls"] = {
                        arm: [s["mpfr_normalization"]["calls"] for s in samples if s["arm"] == arm]
                        for arm in ARMS
                    }
                    row["normalization_skipped"] = {
                        arm: [s["mpfr_normalization"]["skipped_resorts"] for s in samples if s["arm"] == arm]
                        for arm in ARMS
                    }
                    row["baseline_over_arm"] = {
                        arm: row["wall_ns_median"]["legacy"] / row["wall_ns_median"][arm]
                        for arm in ARMS
                    }
                    print(
                        f"MPFR_NORMALIZE_AB case={case} bits={bits} status=SEMANTIC_PARITY_PASS "
                        + " ".join(
                            f"{arm}_ms={row['wall_ns_median'][arm] / 1e6:.3f}"
                            for arm in ARMS
                        )
                        + f" conditional_skip={row['normalization_skipped']['check_sorted'][0]}"
                    )
            if row["status"] != "SEMANTIC_PARITY_PASS":
                censored = True
                print(f"MPFR_NORMALIZE_CENSORED case={case} bits={bits} status={row['status']}")
            report["rows"].append(row)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if censored:
        raise SystemExit("normalization ablation failed scientific admission")


if __name__ == "__main__":
    main()
