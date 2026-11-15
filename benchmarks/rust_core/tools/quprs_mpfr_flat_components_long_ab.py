#!/usr/bin/env python3
"""Source-locked Feynman original CNF flat-component group native A/B.

GPMC C++ and three Rust arms with fresh order-rotated native processes,
identical source CNF, 50k Cache, 100/333-bit MPFR, and bounded resources.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import statistics

from quprs_long_feynman_pilot import native, numerically_agree, sha_file

SOURCE_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
CASES = ("feynman_hwb6", "feynman_barenco_tof_5", "feynman_mod_mult_55")
DIGITS = {30: 100, 100: 333}
CAPACITY = 50_000
ARMS = ("gpmc", "scratch", "flat", "flat_reuse")


def main() -> None:
    p = argparse.ArgumentParser()
    for arg in ("scout-report", "node-report", "corpus", "gpmc", "mpfr", "report"):
        p.add_argument("--" + arg, type=Path, required=True)
    p.add_argument("--trials", type=int, default=8)
    args = p.parse_args()
    if args.trials != 2 * len(ARMS):
        raise SystemExit("requires eight trials, two complete balanced rotations")
    scout = json.loads(args.scout_report.read_text(encoding="utf8"))
    node = json.loads(args.node_report.read_text(encoding="utf8"))
    if scout["quprs_source_sha"] != SOURCE_SHA or node["source_quprs_sha"] != SOURCE_SHA:
        raise SystemExit("original QuPRS source SHA drift")
    if scout["gpmc_source_sha"] != GPMC_SHA or node["source_gpmc_sha"] != GPMC_SHA:
        raise SystemExit("original GPMC C++ source SHA drift")
    source = {r["case"]: r for r in scout["rows"]}
    caps = {r["case"]: r for r in node["rows"]}
    if caps["feynman_hwb6"]["qualified_node_budget"] != 1_000_000 or caps["feynman_hwb6"]["status"] != "SOLVER_AGREEMENT_ONLY":
        raise SystemExit("HWB6 1M-node qualification missing")
    if caps["feynman_grover_5"]["status"] != "CENSORED_NO_SAFE_COMPLETION_UP_TO_1M_NODES":
        raise SystemExit("Grover5 originally censored control drift")
    result = {
        "original_quprs_source_sha": SOURCE_SHA,
        "original_gpmc_source_sha": GPMC_SHA,
        "purpose": "Sorted flat DSU component groups with and without scratch reuse vs unchanged Fast+frequency scratch control"
        "numeric_contract": "100/333-bit MPFR GPMC C++/Rust comparison, full Rust stdout equality, fixed-cap search parity",
        "resource_limit": "per native process 30s, 2GiB, fixed bounded DPLL nodes",
        "cache_limit": CAPACITY,
        "arms": list(ARMS),
        "caveat": "original larger Feynman CNF source/solver agreement only; not independent exact Full WMC certificate",
        "grover5": "CENSORED_1M_NODES_NO_TIME_RANK",
        "rows": [],
    }
    errors = False
    for case in CASES:
        meta = source[case]
        if meta["status"] != "SOURCE_QUALIFIED_WITH_BOUNDED_SOLVER_RESULTS" or meta["source_status"] != "EXPORTED":
            raise SystemExit(f"not source qualified: {case}")
        cnf = args.corpus / f"{case}-gpmc.cnf"
        digest = sha_file(cnf)
        if digest != meta["source_cnf_sha256"]:
            raise SystemExit(f"source CNF bytes changed: {case}")
        budget = 1_000_000 if case == "feynman_hwb6" else 100_000
        for digits, bits in DIGITS.items():
            samples = []
            for trial in range(args.trials):
                offset = trial % len(ARMS)
                ordered = ARMS[offset:] + ARMS[:offset]
                for arm in ordered:
                    if arm == "gpmc":
                        command = [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(cnf.resolve())]
                        tool = "gpmc"
                    else:
                        command = [str(args.mpfr.resolve())]
                        if budget != 100_000:
                            command.append(f"--node-budget={budget}")
                        command.append(f"--cache-limit={CAPACITY}")
                        command += [
                            "--indexed-unit-prop-csr-after-8192",
                            "--packed-residual-key-fast", "--fast-assign",
                            "--reuse-frequency-scratch",
                            *(["--flat-component-groups"] if arm == "flat" else []),
                            *(["--flat-component-groups-reuse"] if arm == "flat_reuse" else []),
                            "--hashbrown-borrowed", str(digits), str(cnf.resolve()),
                        ]
                        tool = "rawmocs_mpfr"
                    observation = native(command, tool)
                    observation["arm"], observation["trial"] = arm, trial
                    samples.append(observation)
            row = {
                "case": case, "digits": digits, "bits": bits, "node_budget": budget,
                "variables": meta["variables"], "clauses": meta["clauses"],
                "source_cnf_sha256": digest, "original_qasm_sha256": meta["original_qasm_sha256"],
                "samples": samples,
            }
            if any(s["status"] != "COMPLETED" for s in samples):
                row["status"] = "CENSORED_NATIVE_OR_RESOURCE_LIMIT"
                row["censored"] = [(s["arm"], s["status"]) for s in samples if s["status"] != "COMPLETED"]
            else:
                rust = [s for s in samples if s["arm"] != "gpmc"]
                by_arm = {
                    arm: {(s.get("dpll_nodes"), s.get("cache_hits"), s.get("component_splits"))
                          for s in rust if s["arm"] == arm}
                    for arm in ARMS[1:]
                }
                if any(s["mpfr_precision_bits"] != bits for s in rust):
                    row["status"] = "FAIL_MPFR_PRECISION"
                elif len({s["sat"] for s in samples}) != 1:
                    row["status"] = "FAIL_SAT"
                elif len({s["stdout_sha256"] for s in rust}) != 1:
                    row["status"] = "FAIL_EXACT_COMPLEX_WMC_STDOUT"
                elif any(len(stats) != 1 or None in next(iter(stats))
                         for stats in by_arm.values()):
                    row["status"] = "FAIL_DETERMINISTIC_SEARCH"
                elif any(by_arm[arm] != by_arm["scratch"] for arm in ("flat", "flat_reuse")):
                    row["status"] = "FAIL_SAME_CAPACITY_SEARCH_PARITY"
                elif any(s.get("engine_drop_ns") is None or s.get("solve_ns") is None for s in rust):
                    row["status"] = "FAIL_MISSING_SOLVE_OR_DROP_PHASE"
                elif not all(
                    numerically_agree(Fraction(s["real"]), Fraction(samples[0]["real"]), digits)
                    and numerically_agree(Fraction(s["imag"]), Fraction(samples[0]["imag"]), digits)
                    for s in samples
                ):
                    row["status"] = "FAIL_EQUAL_MPFR_GPMC_AGREEMENT"
                else:
                    row["status"] = "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY"
                    row["search_stats"] = {
                        arm: {"nodes": next(iter(by_arm[arm]))[0], "cache_hits": next(iter(by_arm[arm]))[1],
                              "decompositions": next(iter(by_arm[arm]))[2]}
                        for arm in ARMS[1:]
                    }
                    for field, key in [
                        ("wall_ns_median", "wall_ns"), ("peak_rss_kib_median", "peak_rss_kib"),
                        ("rust_solve_ns_median", "solve_ns"), ("engine_drop_ns_median", "engine_drop_ns")
                    ]:
                        row[field] = {
                            arm: statistics.median(s[key] for s in samples if s["arm"] == arm)
                            for arm in (ARMS if key in ("wall_ns", "peak_rss_kib") else ARMS[1:])
                        }
                    for metric, key in [
                        ("paired_wall_speedup_vs_scratch", "wall_ns"),
                        ("paired_solve_speedup_vs_scratch", "solve_ns"),
                    ]:
                        row[metric] = {
                            arm: statistics.median(
                                next(s[key] for s in samples if s["arm"] == "scratch" and s["trial"] == t)
                                / next(s[key] for s in samples if s["arm"] == arm and s["trial"] == t)
                                for t in range(args.trials)
                            )
                            for arm in ARMS[1:] if arm != "scratch"
                        }
                    print(
                        f"LONG_FLAT_COMPONENT_AB case={case} bits={bits} status={row['status']} "
                        + " ".join(f"{a}_wall_ms={row['wall_ns_median'][a]/1e6:.3f}" for a in ARMS)
                        + " " + " ".join(f"{a}_solve_ms={row['rust_solve_ns_median'][a]/1e6:.3f}" for a in ARMS[1:])
                        + " " + " ".join(f"{a}_rss_kib={row['peak_rss_kib_median'][a]:.0f}" for a in ARMS)
                        + " " + " ".join(f"{a}_drop_ms={row['engine_drop_ns_median'][a]/1e6:.3f}" for a in ARMS[1:])
                        + " " + " ".join(f"{a}_solve_paired={row['paired_solve_speedup_vs_fast'][a]:.4f}" for a in ARMS[1:] if a != "scratch")
                        + " " + " ".join(f"{a}_nodes={row['search_stats'][a]['nodes']}" for a in ARMS[1:])
                    )
            if row["status"] != "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY":
                errors = True
                print(f"LONG_FLAT_COMPONENT_CENSORED case={case} bits={bits} status={row['status']}")
            result["rows"].append(row)
    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf8")
    if errors:
        raise SystemExit("Flat DSU component grouping native parity failed; no speed claim")


if __name__ == "__main__":
    main()
