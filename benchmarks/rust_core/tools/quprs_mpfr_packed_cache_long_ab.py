#!/usr/bin/env python3
"""Original Feynman: borrowed exact shared versus flat packed Cache keys.

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
CAPACITIES = (10, 25, 50)
ARMS = ("gpmc", "shared10", "packed10", "shared25", "packed25", "shared50", "packed50")
SOURCE_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scout-report", type=Path, required=True)
    ap.add_argument("--node-report", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--gpmc", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=7)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if args.trials != len(ARMS):
        raise SystemExit("factorial requires one full seven-arm rotation for balanced trial positions")
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
        "numeric_contract": "GPMC/Rust equal 100/333-bit MPFR; exact full Rust stdout; shared and packed exact Cache preserve nodes and hits at fixed capacity; all Rust arms same CSR propagator at 8192 nodes",
        "factors": {"cache_key": ["shared-original", "packed-flat-zero-delimited"], "cache_limits": [10000, 25000, 50000]},
        "sampling": "seven fresh processes per arm, one complete balanced seven-arm rotation within each original source and MPFR precision",
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
                        capacity = int(arm[-2:]) * 1000
                        if capacity != 10_000:
                            cmd.append(f"--cache-limit={capacity}")
                        cmd.append("--indexed-unit-prop-csr-after-8192")
                        cmd.append("--packed-residual-key" if arm.startswith("packed") else "--shared-residual-key")
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
                # Search topology MAY change with more cache entries. It must
                # remain deterministic within each arm; only exact WMC and
                # source/parity conditions are shared across all Rust arms.
                stats_by_arm = {
                    arm: {
                        (s.get("dpll_nodes"), s.get("cache_hits"), s.get("component_splits"))
                        for s in rust if s["arm"] == arm
                    }
                    for arm in ARMS[1:]
                }
                if any(s["mpfr_precision_bits"] != bits for s in rust):
                    row["status"] = "FAIL_RUST_MPFR_PRECISION_DRIFT"
                elif len({s["sat"] for s in trials}) != 1:
                    row["status"] = "FAIL_SAT_DISAGREEMENT"
                elif len(hashes) != 1:
                    row["status"] = "FAIL_RUST_FULL_OUTPUT_PARITY"
                elif any(s.get("engine_drop_ns") is None for s in rust):
                    row["status"] = "FAIL_MISSING_EXPLICIT_ENGINE_TEARDOWN_TIMING"
                elif any(len(stats) != 1 or None in next(iter(stats)) for stats in stats_by_arm.values()):
                    row["status"] = "FAIL_NONDETERMINISTIC_OR_MISSING_SEARCH_STATS"
                elif any(
                    stats_by_arm[f"shared{capacity}"] != stats_by_arm[f"packed{capacity}"]
                    for capacity in CAPACITIES
                ):
                    row["status"] = "FAIL_SHARED_PACKED_SEARCH_PARITY"
                elif not all(
                    numerically_agree(Fraction(s["real"]), Fraction(trials[0]["real"]), digits)
                    and numerically_agree(Fraction(s["imag"]), Fraction(trials[0]["imag"]), digits)
                    for s in trials
                ):
                    row["status"] = "FAIL_MATCHED_MPFR_NUMERIC_AGREEMENT"
                else:
                    row["status"] = "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY"
                    row["search_stats"] = {
                        arm: {
                            "nodes": next(iter(stats_by_arm[arm]))[0],
                            "cache_hits": next(iter(stats_by_arm[arm]))[1],
                            "decompositions": next(iter(stats_by_arm[arm]))[2]
                        }
                        for arm in ARMS[1:]
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
                        for arm in ARMS[1:]
                    }
                    row["rust_engine_drop_ns_median"] = {
                        arm: statistics.median(s["engine_drop_ns"] for s in rust if s["arm"] == arm)
                        for arm in ARMS[1:]
                    }
                    row["baseline_over_arm"] = {
                        arm: row["wall_ns_median"]["shared10"] / row["wall_ns_median"][arm]
                        for arm in ARMS[2:]
                    }
                    row["paired_shared_over_packed_by_capacity"] = {
                        str(capacity): [
                            next(
                                s["wall_ns"] for s in trials
                                if s["arm"] == f"shared{capacity}" and s["trial"] == trial
                            ) / next(
                                s["wall_ns"] for s in trials
                                if s["arm"] == f"packed{capacity}" and s["trial"] == trial
                            )
                            for trial in range(args.trials)
                        ]
                        for capacity in CAPACITIES
                    }
                    row["paired_packed_speedup_median"] = {
                        str(capacity): statistics.median(
                            row["paired_shared_over_packed_by_capacity"][str(capacity)]
                        )
                        for capacity in CAPACITIES
                    }
                    row["wall_ns_mad"] = {
                        arm: statistics.median(
                            abs(s["wall_ns"] - row["wall_ns_median"][arm])
                            for s in trials if s["arm"] == arm
                        )
                        for arm in ARMS
                    }
                    row["packed_interaction_wall_ratio_50k_over_10k"] = (
                        (row["wall_ns_median"]["shared50"] / row["wall_ns_median"]["packed50"])
                        / (row["wall_ns_median"]["shared10"] / row["wall_ns_median"]["packed10"])
                    )
                    row["rss_saved_pct"] = {
                        str(capacity): 100 * (
                            row["peak_rss_kib_median"][f"shared{capacity}"]
                            - row["peak_rss_kib_median"][f"packed{capacity}"]
                        ) / row["peak_rss_kib_median"][f"shared{capacity}"]
                        for capacity in CAPACITIES
                    }
                    row["candidate_pareto_frontier"] = [
                        arm for arm in ARMS[1:]
                        if not any(
                            other != arm
                            and row["wall_ns_median"][other] <= row["wall_ns_median"][arm]
                            and row["peak_rss_kib_median"][other] <= row["peak_rss_kib_median"][arm]
                            and (
                                row["wall_ns_median"][other] < row["wall_ns_median"][arm]
                                or row["peak_rss_kib_median"][other] < row["peak_rss_kib_median"][arm]
                            )
                            for other in ARMS[1:]
                        )
                    ]
                    row["gpmc_over_arm"] = {
                        arm: row["wall_ns_median"][arm] / row["wall_ns_median"]["gpmc"]
                        for arm in ARMS[1:]
                    }
                    print(
                        f"LONG_PACKED_CACHE_AB case={case} bits={bits} "
                        "status=MATCHED_MPFR_SOURCE_AGREEMENT_ONLY "
                        + " ".join(
                            f"{arm}_ms={row['wall_ns_median'][arm]/1e6:.3f}"
                            for arm in ARMS
                        )
                        + " " + " ".join(
                            f"{arm}_nodes={row['search_stats'][arm]['nodes']}"
                            for arm in ARMS[1:]
                        )
                        + " " + " ".join(
                            f"{arm}_rss_kib={row['peak_rss_kib_median'][arm]:.0f}"
                            for arm in ARMS
                        )
                        + " " + " ".join(
                            f"packed{capacity}_vs_shared{capacity}_paired={row['paired_packed_speedup_median'][str(capacity)]:.4f}"
                            for capacity in CAPACITIES
                        )
                        + " " + " ".join(
                            f"{arm}_solve_ms={row['rust_solve_ns_median'][arm]/1e6:.3f}"
                            for arm in ARMS[1:]
                        )
                        + " " + " ".join(
                            f"{arm}_drop_ms={row['rust_engine_drop_ns_median'][arm]/1e6:.3f}"
                            for arm in ARMS[1:]
                        )
                        + " " + " ".join(
                            f"{arm}_wall_mad_ms={row['wall_ns_mad'][arm]/1e6:.3f}"
                            for arm in ARMS
                        )
                        + " " + " ".join(
                            f"packed{capacity}_rss_saved_pct={row['rss_saved_pct'][str(capacity)]:.2f}"
                            for capacity in CAPACITIES
                        )
                        + f" interaction_50k_over_10k={row['packed_interaction_wall_ratio_50k_over_10k']:.4f}"
                        + f" pareto={','.join(row['candidate_pareto_frontier'])}"
                    )
            if row["status"] != "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY":
                error = True
                print(f"LONG_PACKED_CACHE_CENSORED case={case} bits={bits} status={row['status']}")
            result["rows"].append(row)
    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf8")
    if error:
        raise SystemExit("long-Feynman packed exact residual Cache admission failed")


if __name__ == "__main__":
    main()
