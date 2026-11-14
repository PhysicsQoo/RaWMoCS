#!/usr/bin/env python3
"""Real long Feynman: GPMC / work-admitted Rust / dense component Rust.

Original immutable source CNFs and 100/333-bit precision, bounded HWB6
1M DPLL nodes. Grover5 is censored at that bound, with no invented rank.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics

from quprs_long_feynman_pilot import native, numerically_agree, sha_file

CASES = ("feynman_hwb6", "feynman_barenco_tof_5", "feynman_mod_mult_55")
DIGITS = {30: 100, 100: 333}
ARMS = ("gpmc", "admitted", "dense")
SOURCE_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scout-report", type=Path, required=True)
    ap.add_argument("--node-report", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--gpmc", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=3)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if args.trials != 3:
        raise SystemExit("research cohort requires three alternating independent trials")
    scout = json.loads(args.scout_report.read_text(encoding="utf8"))
    node = json.loads(args.node_report.read_text(encoding="utf8"))
    if scout["quprs_source_sha"] != SOURCE_SHA or node["source_quprs_sha"] != SOURCE_SHA:
        raise SystemExit("original immutable QuPRS exporter SHA mismatch")
    if scout["gpmc_source_sha"] != GPMC_SHA or node["source_gpmc_sha"] != GPMC_SHA:
        raise SystemExit("original immutable GPMC C++ SHA mismatch")
    source = {r["case"]: r for r in scout["rows"]}
    caps = {r["case"]: r for r in node["rows"]}
    hwb = caps["feynman_hwb6"]
    if hwb["status"] != "SOLVER_AGREEMENT_ONLY" or hwb["qualified_node_budget"] != 1_000_000:
        raise SystemExit("HWB6 did not qualify the exact bounded 1M-node mode")
    grover = caps["feynman_grover_5"]
    if grover["status"] != "CENSORED_NO_SAFE_COMPLETION_UP_TO_1M_NODES":
        raise SystemExit("Grover5 previous node-bound outcome changed")
    result = {
        "source_quprs_sha": SOURCE_SHA,
        "source_gpmc_sha": GPMC_SHA,
        "numeric_contract": "GPMC/Rust equal 100/333-bit MPFR; byte-equal Rust stdout/search; dense component research after 8192-node indexed-unit activation",
        "bounds": "30s/2GiB per native solver process, finite node caps only",
        "precision_caveat": "no independent exact Full WMC certificate for these >17-variable source CNFs; cross-solver agreement only",
        "Grover5": {"status": "CENSORED_NODE_BUDGET_AT_1000000", "variables": 607, "no_rank": True},
        "rows": [],
    }
    error = False
    for case in CASES:
        source_row = source[case]
        if source_row["source_status"] != "EXPORTED" or source_row["status"] != "SOURCE_QUALIFIED_WITH_BOUNDED_SOLVER_RESULTS":
            raise SystemExit(f"{case}: unqualified original QuPRS weighted CNF")
        cnf = args.corpus / f"{case}-gpmc.cnf"
        checksum = sha_file(cnf)
        if source_row["source_cnf_sha256"] != checksum:
            raise SystemExit(f"{case}: original source CNF SHA drift")
        budget = 1_000_000 if case == "feynman_hwb6" else 100_000
        for digits, bits in DIGITS.items():
            trials = []
            for trial in range(args.trials):
                order = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in order:
                    if arm == "gpmc":
                        cmd = [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(cnf.resolve())]
                        tool = "gpmc"
                    else:
                        cmd = [str(args.mpfr.resolve())]
                        if budget != 100_000:
                            cmd.append(f"--node-budget={budget}")
                        if arm == "dense":
                            cmd.append("--dense-component-groups")
                        cmd.append("--indexed-unit-prop-after-8192")
                        cmd.append("--shared-residual-key")
                        cmd.extend(("--fast-assign", "--hashbrown-borrowed", str(digits), str(cnf.resolve())))
                        tool = "rawmocs_mpfr"
                    sample = native(cmd, tool)
                    sample["arm"], sample["trial"] = arm, trial
                    trials.append(sample)
            row = {
                "case": case, "digits": digits, "bits": bits,
                "node_budget": budget, "variables": source_row["variables"],
                "clauses": source_row["clauses"], "source_cnf_sha256": checksum,
                "original_qasm_sha256": source_row["original_qasm_sha256"],
                "samples": trials,
            }
            if any(s["status"] != "COMPLETED" for s in trials):
                row["status"] = "CENSORED_NATIVE_LIMIT_OR_ERROR"
                row["censored_arm_status"] = [(s["arm"], s["status"]) for s in trials]
            else:
                rust = [s for s in trials if s["arm"] != "gpmc"]
                hashes = {s["stdout_sha256"] for s in rust}
                counts = {(s.get("dpll_nodes"), s.get("cache_hits"), s.get("component_splits")) for s in rust}
                if any(s["mpfr_precision_bits"] != bits for s in rust):
                    row["status"] = "FAIL_RUST_MPFR_PRECISION_DRIFT"
                elif len({s["sat"] for s in trials}) != 1:
                    row["status"] = "FAIL_SAT_DISAGREEMENT"
                elif len(hashes) != 1 or len(counts) != 1 or None in next(iter(counts)):
                    row["status"] = "FAIL_OUTPUT_OR_SEARCH_PARITY"
                elif not all(
                    numerically_agree(Fraction(s["real"]), Fraction(trials[0]["real"]), digits)
                    and numerically_agree(Fraction(s["imag"]), Fraction(trials[0]["imag"]), digits)
                    for s in trials
                ):
                    row["status"] = "FAIL_MATCHED_MPFR_NUMERIC_AGREEMENT"
                else:
                    row["status"] = "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY"
                    row["search_stats"] = {
                        "nodes": rust[0]["dpll_nodes"], "cache_hits": rust[0]["cache_hits"],
                        "decompositions": rust[0]["component_splits"],
                    }
                    row["wall_ns_median"] = {
                        arm: statistics.median(s["wall_ns"] for s in trials if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["peak_rss_kib_median"] = {
                        arm: statistics.median(s["peak_rss_kib"] for s in trials if s["arm"] == arm)
                        for arm in ARMS
                    }
                    row["rust_solve_ns_median"] = {
                        arm: statistics.median(s["solve_ns"] for s in rust if s["arm"] == arm)
                        for arm in ("admitted", "dense")
                    }
                    row["baseline_over_arm"] = {
                        arm: row["wall_ns_median"]["admitted"] / row["wall_ns_median"][arm]
                        for arm in ("dense",)
                    }
                    row["gpmc_over_arm"] = {
                        arm: row["wall_ns_median"][arm] / row["wall_ns_median"]["gpmc"]
                        for arm in ("admitted", "dense")
                    }
                    print(
                        f"LONG_DENSE_COMPONENT_AB case={case} bits={bits} "
                        "status=MATCHED_MPFR_SOURCE_AGREEMENT_ONLY "
                        + " ".join(
                            f"{arm}_ms={row['wall_ns_median'][arm]/1e6:.3f}"
                            for arm in ARMS
                        )
                        + f" nodes={row['search_stats']['nodes']}"
                    )
            if row["status"] != "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY":
                error = True
                print(f"LONG_DENSE_COMPONENT_CENSORED case={case} bits={bits} status={row['status']}")
            result["rows"].append(row)
    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf8")
    if error:
        raise SystemExit("long-Feynman dense component science admission failed")


if __name__ == "__main__":
    main()
