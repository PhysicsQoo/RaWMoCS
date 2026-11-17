#!/usr/bin/env python3
"""Independent original QuPRS Feynman CNF factor VE holdout against GPMC C++.

Pinned unmodified QASM -> source-emitted Weighted CNF is generated once,
outside native solver timing. Complete native WallTime, including DP
planning, table multiplication, and fallback, is counted per process.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import statistics
import sys

THIS = Path(__file__).resolve().parent
CORE = THIS.parents[1] / "rust_core" / "tools"
sys.path.insert(0, str(THIS))
sys.path.insert(0, str(CORE))

from quprs_cnf_articulation_ab import (  # noqa: E402
    BITS,
    GPMC_SHA,
    QU_PRS_SHA,
    SOURCE,
    build_command,
    percentile,
    sample,
    sha,
    source_head,
)
from quprs_mqtbench_original_cnf_ab import complex_relative_witness  # noqa: E402

ARMS = ("gpmc", "inline_full", "factor_ve")
TRIALS = 7
REQUIRED_COMPLETED = 6


def self_test() -> None:
    assert BITS == {30: 100, 100: 333}
    assert set(SOURCE) == {
        "feynman_hwb6", "feynman_barenco_tof_5",
        "feynman_mod_mult_55", "feynman_grover_5",
    }
    assert len(ARMS) == 3
    print("FEYNMAN_FACTOR_VE_PREFLIGHT_OK", flush=True)


def command_for(args: argparse.Namespace, arm: str, digits: int,
                cnf: Path, case: str) -> list[str]:
    if arm == "factor_ve":
        cmd = build_command(args, "inline_full", digits, cnf, case)
        position = cmd.index("--hashbrown-borrowed")
        cmd.insert(position, "--factor-variable-elimination")
        return cmd
    return build_command(args, arm, digits, cnf, case)


def summarize(records: list[dict], arm: str) -> dict:
    chosen = [r for r in records if r["arm"] == arm]
    return {
        "p50_wall_ms": statistics.median(r["wall_ns"] for r in chosen) / 1e6,
        "p90_wall_ms": percentile([r["wall_ns"] for r in chosen], .9) / 1e6,
        "p50_peak_rss_kib": statistics.median(r["peak_rss_kib"] for r in chosen),
        "p50_nodes": (statistics.median(r["nodes"] for r in chosen)
                      if arm != "gpmc" else None),
        "p50_cache_hits": (statistics.median(r["cache_hits"] for r in chosen)
                           if arm != "gpmc" else None),
        "p50_components": (statistics.median(r["decompositions"] for r in chosen)
                           if arm != "gpmc" else None),
        "ve_completed": (all(r["factor_ve_completed"] for r in chosen)
                         if arm != "gpmc" else None),
        "ve_capped": (any(r["factor_ve_capped"] for r in chosen)
                      if arm != "gpmc" else None),
        "ve_width": (statistics.median(r["factor_ve_width"] for r in chosen)
                     if arm != "gpmc" else None),
        "ve_cells": (statistics.median(r["factor_ve_cells"] for r in chosen)
                     if arm != "gpmc" else None),
        "ve_products": (statistics.median(r["factor_ve_products"] for r in chosen)
                        if arm != "gpmc" else None),
        "ve_ns": (statistics.median(r["factor_ve_ns"] for r in chosen)
                  if arm != "gpmc" else None),
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    for arg in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + arg, type=Path)
    ap.add_argument("--trials", type=int, default=TRIALS)
    args = ap.parse_args()
    self_test()
    if args.self_test:
        return
    for name in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            ap.error(f"missing required --{name.replace('_','-')}")
    if args.trials != TRIALS:
        raise SystemExit("TRIAL_ROTATION_DRIFT")
    if (source_head(args.quprs) != QU_PRS_SHA
            or source_head(args.gpmc_source) != GPMC_SHA):
        raise SystemExit("PINNED_SOURCE_DRIFT")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    evidence = {
        "research": "Exact factor variable elimination on Feynman original QuPRS Weighted CNF",
        "source_lock": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "precision": "GPMC 30/100 decimals; Rust MPFR 100/333 bits",
        "arms": list(ARMS),
        "native_bounds": "30s, 2GiB address space per independently timed process",
        "fresh_processes_per_completed_cohort": TRIALS,
        "data": [],
        "qualified": 0,
        "warning": "Signed Complex WMC agreement is not an independent large-model exact oracle",
    }

    def save() -> None:
        args.report.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf8")

    for case, path in SOURCE.items():
        cnf = args.corpus / f"{case}-gpmc.cnf"
        qasm = args.quprs / path
        record = {"case": case, "status": "PENDING", "precisions": []}
        evidence["data"].append(record)
        if not (cnf.is_file() and qasm.is_file() and cnf.read_bytes().startswith(b"p cnf ")):
            record["status"] = "CENSORED_SOURCE"
            save()
            continue
        record.update(cnf_sha256=sha(cnf), qasm_sha256=sha(qasm))
        # Non-zero source size and identical original CNF for all three arms.
        record["status"] = "SOURCE_IDENTICAL_CNF"
        save()
        for digits, bits in BITS.items():
            trials = 1 if case == "feynman_grover_5" else TRIALS
            cohort = {"digits": digits, "bits": bits, "status": "INCOMPLETE", "samples": []}
            record["precisions"].append(cohort)
            save()
            for trial in range(trials):
                rotated = ARMS[trial % 3:] + ARMS[:trial % 3]
                for arm in rotated:
                    cmd = command_for(args, arm, digits, cnf, case)
                    measured = sample(cmd, arm, digits)
                    measured["trial"] = trial
                    cohort["samples"].append(measured)
                    save()
                    print(f"FEYNMAN_FACTOR_VE case={case} bits={bits} "
                          f"trial={trial} arm={arm} status={measured['status']} "
                          f"ve_complete={measured.get('factor_ve_completed', 'NA')} "
                          f"ve_width={measured.get('factor_ve_width','NA')} "
                          f"ve_products={measured.get('factor_ve_products','NA')}",
                          flush=True)
            samples = cohort["samples"]
            if any(s["status"] != "COMPLETED" for s in samples):
                cohort["status"] = "CENSORED_NATIVE_ARMS"
                save()
                continue
            if len(samples) != trials * len(ARMS):
                cohort["status"] = "CENSORED_INCOMPLETE_TRIALS"
                save()
                continue
            for trial in range(trials):
                selected = {s["arm"]: s for s in samples if s["trial"] == trial}
                if set(selected) != set(ARMS):
                    raise SystemExit("UNBALANCED_FRESH_PROCESS_TRIAL")
                gpmc = selected["gpmc"]
                inline = selected["inline_full"]
                factor = selected["factor_ve"]
                if not (gpmc["sat"] == inline["sat"] == factor["sat"]):
                    cohort["status"] = "FAILED_BOOLEAN_SAT"
                    save()
                    raise SystemExit("FAILED_BOOLEAN_SAT")
                for other in (inline, factor):
                    witness = complex_relative_witness(gpmc, other, digits)
                    if not witness["pass"]:
                        cohort["status"] = "FAILED_SIGNED_COMPLEX_WMC"
                        cohort["error"] = {"trial": trial, "arm": other["arm"], "witness": witness}
                        save()
                        raise SystemExit("FAILED_SIGNED_COMPLEX_WMC")
                if not factor["factor_ve_enabled"] or inline["factor_ve_enabled"]:
                    cohort["status"] = "FAILED_ISOLATED_ALGORITHM_FLAG"
                    save()
                    raise SystemExit("FAILED_ISOLATED_ALGORITHM_FLAG")
                if factor["factor_ve_products"] > 600_000 or factor["factor_ve_width"] > 11:
                    cohort["status"] = "FAILED_FACTOR_DP_BUDGET"
                    save()
                    raise SystemExit("FAILED_FACTOR_DP_BUDGET")
                if not factor["factor_ve_completed"] and (
                    factor["nodes"], factor["cache_hits"], factor["decompositions"]
                ) != (inline["nodes"], inline["cache_hits"], inline["decompositions"]):
                    cohort["status"] = "FAILED_FAIL_CLOSED_DPLL_TOPOLOGY"
                    save()
                    raise SystemExit("FAILED_FAIL_CLOSED_DPLL_TOPOLOGY")
            cohort["status"] = "MATCHED_SIGNED_COMPLEX_WMC"
            cohort["summary"] = {arm: summarize(samples, arm) for arm in ARMS}
            evidence["qualified"] += 1
            save()
            print(
                f"FEYNMAN_FACTOR_VE_MATCH case={case} bits={bits} "
                f"gpmc_ms={cohort['summary']['gpmc']['p50_wall_ms']:.3f} "
                f"inline_ms={cohort['summary']['inline_full']['p50_wall_ms']:.3f} "
                f"factor_ms={cohort['summary']['factor_ve']['p50_wall_ms']:.3f} "
                f"ve_completed={cohort['summary']['factor_ve']['ve_completed']} "
                f"ve_width={cohort['summary']['factor_ve']['ve_width']} "
                f"ve_products={cohort['summary']['factor_ve']['ve_products']}",
                flush=True,
            )
    evidence["status"] = (
        "SIX_MAIN_COHORTS_MATCHED_WITH_HARD_CASE_CENSORS"
        if evidence["qualified"] >= REQUIRED_COMPLETED
        else "INSUFFICIENT_COMPLETED_NATIVE_COHORTS"
    )
    save()
    if evidence["qualified"] < REQUIRED_COMPLETED:
        raise SystemExit("INSUFFICIENT_FEYNMAN_FACTOR_VE_COHORTS")
    print(f"FEYNMAN_FACTOR_VE_FINISHED qualified={evidence['qualified']}",
          flush=True)


if __name__ == "__main__":
    main()
