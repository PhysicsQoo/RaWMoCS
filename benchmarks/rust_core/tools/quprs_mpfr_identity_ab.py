#!/usr/bin/env python3
"""Matched MPFR, same Rust binary: borrowed-cache baseline vs exact-one shortcut.

Five independent fresh-process trials per arm; preserve source SHA, SAT,
complete native stdout, numeric and DPLL structural parity. Censor failed
or inconsistent cases before displaying WallTime speedups.
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

SOURCE = "0b28736651b3740ed3b201edd2efdef3da541e0d"
CASES = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2", "qft3")
DIGITS = {30: 100, 100: 333}
ARMS = ("baseline", "identity")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-report", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=5)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if args.trials != 5:
        raise SystemExit("exactly five predeclared fresh trials per mode")
    original = json.loads(args.source_report.read_text())
    if original["source_commit"] != SOURCE:
        raise SystemExit("QuPRS original PathSum source SHA mismatch")
    original_rows = {r["case"]: r for r in original["rows"]}
    result = {
        "source_quprs_sha": SOURCE,
        "numerical": "same MPFR 100 and 333 bits, separate four-product arithmetic except exact 1+0i identity",
        "cases": list(CASES), "trials_per_arm": args.trials,
        "bound": "8 seconds wall, 2GiB virtual memory per native child",
        "rows": [],
    }
    censored = False
    for case in CASES:
        prior = original_rows[case]
        cnf = args.corpus / f"{case}-gpmc.cnf"
        if sha256(cnf.read_bytes()).hexdigest() != prior["cnf_sha256"]:
            raise SystemExit(f"{case}: source CNF drift")
        references = [
            s for s in prior["samples"]
            if s["tool"] == "rawmocs" and s["status"] == "COMPLETED"
        ]
        if len(references) < 2 or any(
            (s["sat"], Fraction(s["real"]), Fraction(s["imag"]), s.get("rawmocs_stats")) !=
            (references[0]["sat"], Fraction(references[0]["real"]),
             Fraction(references[0]["imag"]), references[0].get("rawmocs_stats"))
            for s in references
        ):
            raise SystemExit(f"{case}: source-pinned exact rational reference failed")
        exact = (Fraction(references[0]["real"]), Fraction(references[0]["imag"]))
        structure = references[0].get("rawmocs_stats")
        if not structure:
            raise SystemExit(f"{case}: missing exact search witness")
        for digits, bits in DIGITS.items():
            samples = []
            for trial in range(args.trials):
                order = ARMS if trial % 2 == 0 else ARMS[::-1]
                for arm in order:
                    command = [str(args.mpfr.resolve()), "--hashbrown-borrowed"]
                    if arm == "identity":
                        command.append("--identity-mul")
                    command.extend([str(digits), str(cnf.resolve())])
                    trial_record = timed_execution(command, "rawmocs_mpfr")
                    trial_record.update({"trial": trial, "arm": arm})
                    samples.append(trial_record)
            row = {
                "case": case, "digits": digits, "bits": bits,
                "cnf_sha256": prior["cnf_sha256"],
                "qasm_sha256": prior["qasm_sha256"],
                "phase_sha256": prior["phase_sha256"],
                "samples": samples,
            }
            if any(s["status"] != "COMPLETED" for s in samples):
                row["status"] = "CENSORED_NATIVE_FAILURE"
            else:
                semantic = all(
                    s["sat"] == references[0]["sat"] and
                    near(Fraction(s["real"]), exact[0], digits) and
                    near(Fraction(s["imag"]), exact[1], digits) and
                    s.get("mpfr_stats") == structure and
                    s.get("mpfr_precision_bits") == bits and
                    s.get("mpfr_identity_mul_skips") is not None
                    for s in samples
                )
                identical_stdout = len({s["native_stdout_sha256"] for s in samples}) == 1
                exercised = all(
                    s["mpfr_identity_mul_skips"] > 0
                    for s in samples if s["arm"] == "identity"
                )
                row["semantic_admitted"] = semantic
                row["stdout_byte_parity"] = identical_stdout
                row["fastpath_exercised"] = exercised
                if semantic and identical_stdout and exercised:
                    row["status"] = "QUALIFIED_IDENTICAL_MPFR_WMC"
                    row["wall_ns_median"] = {
                        arm: statistics.median(s["wall_ns"] for s in samples if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["solve_ns_median"] = {
                        arm: statistics.median(
                            s["mpfr_phase_ns"]["solve"] for s in samples if s["arm"] == arm
                        ) for arm in ARMS
                    }
                    row["peak_rss_kib_median"] = {
                        arm: statistics.median(s["peak_rss_kib"] for s in samples if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["identity_skip_median"] = statistics.median(
                        s["mpfr_identity_mul_skips"] for s in samples if s["arm"] == "identity"
                    )
                    row["speedup"] = row["wall_ns_median"]["baseline"] / row["wall_ns_median"]["identity"]
                else:
                    row["status"] = "CENSORED_NUMERIC_STRUCTURE_OR_OUTPUT_MISMATCH"
            if row["status"] != "QUALIFIED_IDENTICAL_MPFR_WMC":
                censored = True
            result["rows"].append(row)
            print(
                f"MPFR_IDENTITY_AB case={case} bits={bits} status={row['status']} "
                f"speedup={row.get('speedup', 'CENSORED')} "
                f"skips={row.get('identity_skip_median', 'UNKNOWN')}"
            )
    args.report.write_text(json.dumps(result, indent=2) + "\n")
    if censored:
        raise SystemExit("identity-multiply A/B parity gate refused one or more cases")


if __name__ == "__main__":
    main()
