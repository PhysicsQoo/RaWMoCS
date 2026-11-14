#!/usr/bin/env python3
"""Four-arm source-pinned matched-MPFR cache/assignment composition.

Same native binary, same 100/333-bit MPFR, source SHA and native worker.
Never multiply speedups from separate runs; retain all process samples.
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
BITS = {30: 100, 100: 333}
ARMS = ("std_original", "std_assign", "borrowed_original", "borrowed_assign")
FLAGS = {
    "std_original": (),
    "std_assign": ("--fast-assign",),
    "borrowed_original": ("--hashbrown-borrowed",),
    "borrowed_assign": ("--fast-assign", "--hashbrown-borrowed"),
}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--matched-reference", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=5)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if not 3 <= args.trials <= 7:
        raise SystemExit("three to seven native trials per arm required")
    original = json.loads(args.matched_reference.read_text())
    if original["source_sha"] != "0b28736651b3740ed3b201edd2efdef3da541e0d":
        raise SystemExit("original QuPRS exporter not pinned")
    matched = {(r["case"], r["digits"]): r for r in original["rows"]}
    report: dict[str, Any] = {
        "source_commit": original["source_sha"],
        "precision_digits_to_bits": BITS,
        "study": "baseline vs binary assignment vs borrowed-cache key vs both",
        "same_solver_executable": str(args.mpfr),
        "arms": list(ARMS),
        "test_contract": "same bit precision, fresh process, byte-identical stdout and structural parity, all trials included",
        "rows": [],
    }
    censored = False
    for case in CASES:
        for digits, bits in BITS.items():
            reference = matched[(case, digits)]
            if reference["status"] != "MATCHED_PRECISION_QUALIFIED":
                raise SystemExit(f"unqualified original GPMC/Rust MPFR reference: {case} {digits}")
            cnf = args.corpus / f"{case}-gpmc.cnf"
            digest = sha256(cnf.read_bytes()).hexdigest()
            if digest != reference["source_cnf_sha256"]:
                raise SystemExit(f"source CNF drift: {case}")
            samples = []
            for trial in range(args.trials):
                order = ARMS[trial % 4:] + ARMS[:trial % 4]
                for arm in order:
                    command = [
                        str(args.mpfr.resolve()),
                        *FLAGS[arm],
                        str(digits), str(cnf.resolve()),
                    ]
                    row = timed_execution(command, "rawmocs_mpfr")
                    row["arm"], row["trial"] = arm, trial
                    samples.append(row)
            observed: dict[str, Any] = {
                "case": case, "digits": digits, "bits": bits,
                "source_cnf_sha256": digest, "samples": samples,
            }
            if any(s["status"] != "COMPLETED" for s in samples):
                observed["status"] = "CENSORED_NATIVE_FAILURE"
            else:
                hashes = {s["native_stdout_sha256"] for s in samples}
                values = {
                    (s["sat"], Fraction(s["real"]), Fraction(s["imag"]))
                    for s in samples
                }
                counts = [s.get("mpfr_stats") for s in samples]
                precision = [s.get("mpfr_precision_bits") for s in samples]
                phases = [s.get("mpfr_phase_ns") for s in samples]
                if len(hashes) != 1 or len(values) != 1:
                    observed["status"] = "CENSORED_WMC_OR_OUTPUT_DIVERGENCE"
                elif any(c is None or c != counts[0] for c in counts):
                    observed["status"] = "CENSORED_SEARCH_STRUCTURE_DIVERGENCE"
                elif any(p != bits for p in precision) or any(t is None for t in phases):
                    observed["status"] = "CENSORED_MPFR_PRECISION_OR_PHASE"
                else:
                    observed["status"] = "SEMANTIC_PARITY_PASS"
                    observed["search_structure"] = counts[0]
                    observed["wall_ns_median"] = {
                        arm: statistics.median(
                            x["wall_ns"] for x in samples if x["arm"] == arm
                        ) for arm in ARMS
                    }
                    observed["solve_ns_median"] = {
                        arm: statistics.median(
                            x["mpfr_phase_ns"]["solve"] for x in samples if x["arm"] == arm
                        ) for arm in ARMS
                    }
                    observed["peak_rss_kib_median"] = {
                        arm: statistics.median(
                            x["peak_rss_kib"] for x in samples if x["arm"] == arm
                        ) for arm in ARMS
                    }
                    observed["baseline_over_arm"] = {
                        arm: observed["wall_ns_median"]["std_original"] / observed["wall_ns_median"][arm]
                        for arm in ARMS
                    }
                    print(
                        f"MPFR_COMPOSED_AB case={case} bits={bits} status=SEMANTIC_PARITY_PASS "
                        + " ".join(
                            f"{arm}_ms={observed['wall_ns_median'][arm]/1e6:.3f}"
                            for arm in ARMS
                        )
                    )
            if observed["status"] != "SEMANTIC_PARITY_PASS":
                censored = True
                print(f"MPFR_COMPOSED_CENSORED case={case} bits={bits} status={observed['status']}")
            report["rows"].append(observed)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if censored:
        raise SystemExit("combined cache/assign A/B has unqualified source cases")


if __name__ == "__main__":
    main()
