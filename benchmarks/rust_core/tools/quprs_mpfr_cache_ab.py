#!/usr/bin/env python3
"""Same MPFR precision and same Rust binary: owned vs borrowed cache-key A/B/C.

Every completed trial must preserve SAT, printed WMC, DPLL search shape and
source provenance. Speeds use five same-runner independent native processes
per arm and do not compare differently configured numerical guarantees.
"""
from __future__ import annotations
import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics

from quprs_representative_native import timed_execution
from quprs_mpfr_matched import near

SOURCE_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
CASES = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2", "qft3")
DIGITS = (30, 100)
BITS = {30: 100, 100: 333}
ARMS = ("std_owned", "hb_owned", "hb_borrowed")
FLAGS = {"std_owned": None, "hb_owned": "--hashbrown-owned",
         "hb_borrowed": "--hashbrown-borrowed"}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-report", required=True, type=Path)
    ap.add_argument("--corpus", required=True, type=Path)
    ap.add_argument("--mpfr", required=True, type=Path)
    ap.add_argument("--trials", type=int, default=5)
    ap.add_argument("--report", required=True, type=Path)
    args = ap.parse_args()
    if args.trials != 5:
        raise SystemExit("predeclared 5-trial policy required")
    original = json.loads(args.source_report.read_text())
    if original["source_commit"] != SOURCE_SHA:
        raise SystemExit("source QuPRS SHA mismatch")
    reference_rows = {r["case"]: r for r in original["rows"]}
    result = {
        "source_commit": SOURCE_SHA,
        "scope": "same binary, source-emitted CNF, matching MPFR bits and rounding; research-only",
        "arms": list(ARMS), "digits": list(DIGITS), "trials": args.trials,
        "walltime": "fresh-process 8s/2GiB limits, launch+read+parse+solve+output",
        "rows": [],
    }
    invalid = False
    for case in CASES:
        prior = reference_rows[case]
        source = args.corpus / f"{case}-gpmc.cnf"
        if sha256(source.read_bytes()).hexdigest() != prior["cnf_sha256"]:
            raise SystemExit(f"{case}: source-pinned input drift")
        witnesses = [
            s for s in prior["samples"]
            if s["tool"] == "rawmocs" and s["status"] == "COMPLETED"
        ]
        if not witnesses or any(
            s["sat"] != witnesses[0]["sat"] or
            (Fraction(s["real"]), Fraction(s["imag"])) !=
            (Fraction(witnesses[0]["real"]), Fraction(witnesses[0]["imag"]))
            or s.get("rawmocs_stats") != witnesses[0].get("rawmocs_stats")
            for s in witnesses
        ):
            raise SystemExit(f"{case}: unstable exact reference")
        exact = (Fraction(witnesses[0]["real"]), Fraction(witnesses[0]["imag"]))
        structure = witnesses[0].get("rawmocs_stats")
        if not structure:
            raise SystemExit(f"{case}: no exact DPLL structure proof")
        for digits in DIGITS:
            samples = []
            for trial in range(args.trials):
                order = ARMS[trial % 3:] + ARMS[:trial % 3]
                for arm in order:
                    cmd = [str(args.mpfr.resolve())]
                    if FLAGS[arm]:
                        cmd.append(FLAGS[arm])
                    cmd.extend([str(digits), str(source.resolve())])
                    outcome = timed_execution(cmd, "rawmocs_mpfr")
                    outcome["arm"] = arm
                    outcome["trial"] = trial
                    samples.append(outcome)
            row = {
                "case": case, "digits": digits, "bits": BITS[digits],
                "input_sha256": prior["cnf_sha256"],
                "qasm_sha256": prior["qasm_sha256"],
                "phase_sha256": prior["phase_sha256"], "samples": samples,
            }
            all_completed = all(s["status"] == "COMPLETED" for s in samples)
            if not all_completed:
                row["status"] = "CENSORED_NATIVE_FAILURE"
            else:
                bytes_equal = len({s["native_stdout_sha256"] for s in samples}) == 1
                science = all(
                    s["sat"] == witnesses[0]["sat"] and
                    near(Fraction(s["real"]), exact[0], digits) and
                    near(Fraction(s["imag"]), exact[1], digits) and
                    s.get("mpfr_stats") == structure and
                    s.get("mpfr_precision_bits") == BITS[digits] and
                    s.get("mpfr_precision_digits") == digits
                    for s in samples
                )
                row["scientific_parity"] = science
                row["stdout_byte_parity"] = bytes_equal
                if science and bytes_equal:
                    row["status"] = "EXACT_SAME_BIT_PRECISION_AND_STRUCTURE"
                    row["median_wall_ns"] = {
                        arm: statistics.median(s["wall_ns"] for s in samples if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["median_peak_rss_kib"] = {
                        arm: statistics.median(s["peak_rss_kib"] for s in samples if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["median_solve_ns"] = {
                        arm: statistics.median(
                            s["mpfr_phase_ns"]["solve"] for s in samples if s["arm"] == arm
                        ) for arm in ARMS
                    }
                    row["ratio_std_over_hashbrown_owned"] = (
                        row["median_wall_ns"]["std_owned"] /
                        row["median_wall_ns"]["hb_owned"]
                    )
                    row["ratio_hashbrown_owned_over_borrowed"] = (
                        row["median_wall_ns"]["hb_owned"] /
                        row["median_wall_ns"]["hb_borrowed"]
                    )
                    row["ratio_std_over_borrowed"] = (
                        row["median_wall_ns"]["std_owned"] /
                        row["median_wall_ns"]["hb_borrowed"]
                    )
                else:
                    row["status"] = "CENSORED_NUMERIC_STRUCTURE_OR_STDOUT_MISMATCH"
            if row["status"] != "EXACT_SAME_BIT_PRECISION_AND_STRUCTURE":
                invalid = True
            result["rows"].append(row)
            print(
                f"MPFR_CACHE_AB case={case} digits={digits} status={row['status']} "
                f"speedup={row.get('ratio_std_over_borrowed', 'CENSORED')}"
            )
    args.report.write_text(json.dumps(result, indent=2) + "\n")
    if invalid:
        raise SystemExit("cache-key A/B scientific admission failed")


if __name__ == "__main__":
    main()
