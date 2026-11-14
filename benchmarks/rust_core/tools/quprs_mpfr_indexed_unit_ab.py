#!/usr/bin/env python3
"""Original QuPRS CNF: shared Cache + occurrence-indexed unit propagation A/B.

Same source CNF, fresh native Rust binary/MPFR and GPMC-qualified numeric
reference; all records retained, no production implementation changes.
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
ARMS = ("shared", "indexed")
FLAGS = {
    "shared": ("--shared-residual-key",),
    "indexed": ("--indexed-unit-prop", "--shared-residual-key"),
}
SOURCE = "0b28736651b3740ed3b201edd2efdef3da541e0d"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-report", type=Path, required=True)
    ap.add_argument("--matched-reference", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=5)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if not 3 <= args.trials <= 7:
        raise SystemExit("require 3-7 independent alternating process trials")
    source = json.loads(args.source_report.read_text())
    matched = json.loads(args.matched_reference.read_text())
    if source["source_commit"] != SOURCE or matched["source_sha"] != SOURCE:
        raise SystemExit("original QuPRS PathSum/WmcManager exporter not pinned")
    expected = {r["case"]: r for r in source["rows"]}
    qualified = {(r["case"], r["digits"]): r for r in matched["rows"]}
    report = {
        "study": "source-identical shared-residual Cache vs occurrence-indexed unit propagation",
        "source_quprs_commit": SOURCE,
        "source_qualification": "unchanged original GPMC C++ and 100/333-bit MPFR reference",
        "search": "same original frequency pivot, component split, exact equality and 10000 Cache entry cap",
        "walltime": "fresh native process, startup/read/parse/solve/serializing included",
        "trials_per_arm": args.trials,
        "rows": [],
    }
    any_bad = False
    for case in CASES:
        source_row = expected[case]
        cnf = args.corpus / f"{case}-gpmc.cnf"
        digest = sha256(cnf.read_bytes()).hexdigest()
        if source_row["cnf_sha256"] != digest:
            raise SystemExit(f"original source CNF mismatch: {case}")
        for digits, bits in PRECISIONS.items():
            prior = qualified[(case, digits)]
            if prior["status"] != "MATCHED_PRECISION_QUALIFIED" or prior["source_cnf_sha256"] != digest:
                raise SystemExit(f"original MPFR precision reference invalid: {case} {digits}")
            samples = []
            for trial in range(args.trials):
                order = ARMS if trial % 2 == 0 else ARMS[::-1]
                for arm in order:
                    command = [
                        str(args.mpfr.resolve()),
                        *FLAGS[arm], "--fast-assign", "--hashbrown-borrowed",
                        str(digits), str(cnf.resolve()),
                    ]
                    record = timed_execution(command, "rawmocs_mpfr")
                    record["arm"], record["trial"] = arm, trial
                    samples.append(record)
            row = {
                "case": case, "digits": digits, "bits": bits,
                "source_cnf_sha256": digest, "source_qasm_sha256": source_row["qasm_sha256"],
                "samples": samples,
            }
            if any(s["status"] != "COMPLETED" for s in samples):
                row["status"] = "CENSORED_NATIVE_TIMEOUT_OR_ERROR"
            else:
                outputs = {s["native_stdout_sha256"] for s in samples}
                values = {(s["sat"], Fraction(s["real"]), Fraction(s["imag"])) for s in samples}
                profiles = [s.get("mpfr_phase_ns") for s in samples]
                counters = [s.get("mpfr_stats") for s in samples]
                precisions = [s.get("mpfr_precision_bits") for s in samples]
                if len(outputs) != 1 or len(values) != 1:
                    row["status"] = "CENSORED_WMC_OR_OUTPUT_DIFFERENCE"
                elif any(c is None or c != counters[0] for c in counters):
                    row["status"] = "CENSORED_SEARCH_STRUCTURE_DIFFERENCE"
                elif any(v != bits for v in precisions) or any(p is None for p in profiles):
                    row["status"] = "CENSORED_PRECISION_OR_TELEMETRY"
                else:
                    row["status"] = "SEMANTIC_PARITY_PASS"
                    row["search_counters"] = counters[0]
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
                    row["speed_ratio"] = (
                        row["wall_ns_median"][ARMS[0]] / row["wall_ns_median"][ARMS[1]]
                    )
                    print(
                        f"MPFR_INDEXED_UNIT_AB case={case} bits={bits} status=SEMANTIC_PARITY_PASS "
                        f"shared_ms={row['wall_ns_median'][ARMS[0]]/1e6:.3f} "
                        f"indexed_ms={row['wall_ns_median'][ARMS[1]]/1e6:.3f} "
                        f"ratio={row['speed_ratio']:.3f}"
                    )
            if row["status"] != "SEMANTIC_PARITY_PASS":
                any_bad = True
                print(f"MPFR_INDEXED_UNIT_CENSORED case={case} bits={bits} status={row['status']}")
            report["rows"].append(row)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if any_bad:
        raise SystemExit("indexed unit propagation A/B correctness or resources not qualified")


if __name__ == "__main__":
    main()
