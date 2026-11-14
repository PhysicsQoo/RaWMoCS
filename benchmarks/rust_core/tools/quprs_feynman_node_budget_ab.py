#!/usr/bin/env python3
"""Bounded Feynman long-source node-cap scientific follow-up.

This does not remove the original 100k node guard or change any
production solver. The source scout already proved original QASM/CNF
bytes and classified 100k-node failures separately from native timeouts.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics
from typing import Any

from quprs_long_feynman_pilot import native, numerically_agree, sha_file

LONG_CASES = ("feynman_hwb6", "feynman_grover_5")
BUDGETS = (250_000, 1_000_000)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scout-report", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--gpmc", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    source = json.loads(args.scout_report.read_text())
    if source["quprs_source_sha"] != "0b28736651b3740ed3b201edd2efdef3da541e0d":
        raise SystemExit("wrong original QuPRS source")
    if source["gpmc_source_sha"] != "89bbc89d0de918e7a5385740355df55765039230":
        raise SystemExit("wrong unchanged GPMC source")
    original = {r["case"]: r for r in source["rows"]}
    result: dict[str, Any] = {
        "source_quprs_sha": source["quprs_source_sha"],
        "source_gpmc_sha": source["gpmc_source_sha"],
        "scope": "research-only explicit 250k or 1M DPLL node cap at 100-bit MPFR",
        "baseline_node_cap": 100_000,
        "budgets": list(BUDGETS),
        "bounds": "30s wall and 2GiB RLIMIT_AS per native solver process",
        "limits": "no unlimited node budget; exact printed Full WMC is not independently certified on these large cases",
        "rows": [],
    }
    for case in LONG_CASES:
        observed = original[case]
        cnf = args.corpus / f"{case}-gpmc.cnf"
        if observed["source_status"] != "EXPORTED":
            raise SystemExit(f"{case}: original QuPRS exporter incomplete")
        if observed["status"] != "SOURCE_QUALIFIED_WITH_BOUNDED_SOLVER_RESULTS":
            raise SystemExit(f"{case}: original CNF source cannot qualify")
        if observed["source_cnf_sha256"] != sha_file(cnf):
            raise SystemExit(f"{case}: exact source CNF bytes drifted")
        base_rows = [
            s for s in observed["samples"]
            if s["digits"] == 30 and s["tool"] == "rawmocs_mpfr"
        ]
        if len(base_rows) != 2 or any(
            s["status"] != "NATIVE_ERROR_OR_UNKNOWN"
            or "node limit" not in s.get("stderr_tail", "")
            for s in base_rows
        ):
            raise SystemExit(f"{case}: baseline was not definitely a 100k-node cap")
        row: dict[str, Any] = {
            "case": case, "cnf_sha256": sha_file(cnf),
            "variables": observed["variables"], "clauses": observed["clauses"],
            "prior_100k_status": "NODE_BUDGET_REACHED",
            "caps": [], "paired_samples": [],
        }
        qualified_cap = None
        for budget in BUDGETS:
            samples = []
            for trial in range(2):
                command = [
                    str(args.mpfr.resolve()), f"--node-budget={budget}",
                    "--fast-assign", "--hashbrown-borrowed", "30", str(cnf.resolve())
                ]
                sample = native(command, "rawmocs_mpfr")
                sample["trial"] = trial
                samples.append(sample)
            states = [s["status"] for s in samples]
            if all(
                s["status"] == "NATIVE_ERROR_OR_UNKNOWN" and
                "node limit" in s.get("stderr_tail", "")
                for s in samples
            ):
                status = "NODE_BUDGET_REACHED"
            elif all(s["status"] == "COMPLETED" and s.get("mpfr_precision_bits") == 100 for s in samples):
                values = {(s["sat"], s["real"], s["imag"]) for s in samples}
                if len(values) != 1:
                    status = "CENSORED_NONDETERMINISTIC_RUST"
                else:
                    status = "RUST_COMPLETED_AT_BOUNDED_NODE_CAP"
                    qualified_cap = budget
            else:
                status = "CENSORED_TIMEOUT_RESOURCE_OR_NATIVE_ERROR"
            row["caps"].append({"budget": budget, "status": status, "samples": samples})
            print(f"LONG_NODE_CAP case={case} budget={budget} status={status} trial_status={states}")
            if qualified_cap is not None:
                break
        if qualified_cap is not None:
            for trial in range(2):
                order = ("gpmc", "rawmocs_mpfr") if trial == 0 else (
                    "rawmocs_mpfr", "gpmc"
                )
                for tool in order:
                    command = (
                        [str(args.gpmc.resolve()), "-mode=1", "-prec=30", str(cnf.resolve())]
                        if tool == "gpmc" else
                        [
                            str(args.mpfr.resolve()), f"--node-budget={qualified_cap}",
                            "--fast-assign", "--hashbrown-borrowed", "30", str(cnf.resolve())
                        ]
                    )
                    s = native(command, tool)
                    s["trial"] = trial
                    row["paired_samples"].append(s)
            pair = row["paired_samples"]
            if any(s["status"] != "COMPLETED" for s in pair):
                row["status"] = "CENSORED_PAIRED_NATIVE_COMPLETION"
            elif len({s["sat"] for s in pair}) != 1:
                row["status"] = "CENSORED_PAIRED_SAT_DISAGREEMENT"
            elif any(
                s["tool"] == "rawmocs_mpfr" and s.get("mpfr_precision_bits") != 100
                for s in pair
            ):
                row["status"] = "CENSORED_MPFR_WIDTH_MISMATCH"
            elif not all(
                numerically_agree(
                    Fraction(s["real"]), Fraction(pair[0]["real"]), 30
                ) and numerically_agree(
                    Fraction(s["imag"]), Fraction(pair[0]["imag"]), 30
                ) for s in pair
            ):
                row["status"] = "CENSORED_NUMERICAL_DISAGREEMENT"
            else:
                row["status"] = "SOLVER_AGREEMENT_ONLY"
                row["qualified_node_budget"] = qualified_cap
                row["median_wall_ns"] = {
                    tool: statistics.median(
                        s["wall_ns"] for s in pair if s["tool"] == tool
                    ) for tool in ("gpmc", "rawmocs_mpfr")
                }
                row["median_peak_rss_kib"] = {
                    tool: statistics.median(
                        s["peak_rss_kib"] for s in pair if s["tool"] == tool
                    ) for tool in ("gpmc", "rawmocs_mpfr")
                }
                row["ratio_gpmc_over_rust"] = (
                    row["median_wall_ns"]["gpmc"] /
                    row["median_wall_ns"]["rawmocs_mpfr"]
                )
        else:
            row["status"] = "CENSORED_NO_SAFE_COMPLETION_UP_TO_1M_NODES"
        result["rows"].append(row)
        print(
            f"LONG_NODE_BUDGET_RESULT case={case} status={row['status']} "
            f"budget={row.get('qualified_node_budget','CENSORED')} "
            f"wall={row.get('median_wall_ns','NO_RANK')}"
        )
    args.report.write_text(json.dumps(result, indent=2) + "\n")
    print("LONG_NODE_BUDGET_COMPLETE cases=", len(result["rows"]))


if __name__ == "__main__":
    main()
