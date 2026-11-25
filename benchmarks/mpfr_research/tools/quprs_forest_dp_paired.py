#!/usr/bin/env python3
"""Same-original-Weighted-CNF native Forest DP research (not a production GO)."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics

from quprs_full_live_key_paired import (
    ADDRESS_SPACE, BITS, CASES, DEADLINE_S, GPMC_SHA, QU_PRS_SHA,
    agrees, command_for, run_one, sha, source_file, source_head, summary,
)

ARMS = ("gpmc", "inline_full", "forest_dp")
TRIALS = 7


def command(args: argparse.Namespace, case: str, cnf: Path,
            digits: int, arm: str) -> list[str]:
    if arm not in ARMS:
        raise ValueError("UNKNOWN_FOREST_ARM")
    base = "gpmc" if arm == "gpmc" else "inline_full"
    out = command_for(args, case, cnf, digits, base)
    if arm == "forest_dp":
        out.insert(out.index("--hashbrown-borrowed"), "--trail-forest-dp")
    return out


def self_test() -> None:
    for family, cases in CASES.items():
        args = argparse.Namespace(
            family=family, gpmc=Path("/original-gpmc"), mpfr=Path("/original-rust"))
        for arm in ARMS:
            out = command(args, cases[0], Path("/same-original.cnf"), 30, arm)
            assert ("--trail-forest-dp" in out) == (arm == "forest_dp")
            assert "--trail-full-live-clause-fastpath" not in out
            if arm != "gpmc":
                assert "--trail-inline-weight-relabel" in out
                assert "--weighted-equivalence" in out
                assert "--trail-count" in out
    print("FOREST_DP_SOURCE_ARM_PREFLIGHT_PASS", flush=True)


def persist(args: argparse.Namespace, report: dict) -> None:
    args.report.write_text(json.dumps(report, indent=2) + "\n")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--family", choices=CASES, default="feynman")
    for arg in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + arg, type=Path)
    args = ap.parse_args()
    self_test()
    if args.self_test:
        return
    for field in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, field) is None:
            ap.error("missing --" + field.replace("_", "-"))
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("ORIGINAL_SOURCE_LOCK_DRIFT")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "study": "Exact opt-in width-one signed complex forest message passing",
        "family": args.family, "arms": list(ARMS), "trials": TRIALS,
        "sources": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "precision": "GPMC 30/100 decimal digits, Rust MPFR 100/333 bits",
        "resources": {"wall_s": DEADLINE_S, "address_space": ADDRESS_SPACE},
        "cases": [], "status": "INCOMPLETE",
    }
    for case in CASES[args.family]:
        cnf = args.corpus / f"{case}-gpmc.cnf"
        if not cnf.is_file():
            raise SystemExit("MISSING_ORIGINAL_WEIGHTED_CNF")
        item = {"case": case, "qasm_sha256": sha(source_file(args, case)),
                "cnf_sha256": sha(cnf), "precisions": []}
        report["cases"].append(item)
        for digits, bits in BITS.items():
            cohort = {"digits": digits, "bits": bits, "status": "INCOMPLETE",
                      "records": []}
            item["precisions"].append(cohort)
            persist(args, report)
            for trial in range(TRIALS):
                ordered = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in ordered:
                    cmd = command(args, case, cnf, digits, arm)
                    # The original common native harness parses full complex
                    # outputs and Trail diagnostics with a no-forest control.
                    parsed_arm = "inline_full" if arm == "forest_dp" else arm
                    row = run_one(cmd, parsed_arm, bits, digits)
                    row["arm"] = arm
                    row["trial"] = trial
                    cohort["records"].append(row)
                    persist(args, report)
                    print(f"FOREST_DP_TRIAL case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={row['status']}", flush=True)
            rows = cohort["records"]
            if len(rows) != TRIALS * len(ARMS) or any(
                    r["status"] != "COMPLETED" for r in rows):
                cohort["status"] = "CENSORED_OR_INCOMPLETE"
                persist(args, report)
                continue
            wins = 0
            for trial in range(TRIALS):
                arms = {r["arm"]: r for r in rows if r["trial"] == trial}
                if set(arms) != set(ARMS):
                    raise SystemExit("FOREST_DP_UNBALANCED_ARMS")
                ref, control, candidate = (arms[a] for a in ARMS)
                if not (agrees(ref, control, args.family, digits)
                        and agrees(ref, candidate, args.family, digits)):
                    cohort["status"] = "SIGNED_COMPLEX_WMC_MISMATCH"
                    persist(args, report)
                    raise SystemExit("FOREST_DP_SIGNED_COMPLEX_WMC_MISMATCH")
                if candidate["forest_completed"] > candidate["forest_attempts"]:
                    raise SystemExit("FOREST_DP_IMPOSSIBLE_ACCOUNTING")
                wins += candidate["wall_ns"] < control["wall_ns"]
            stats = {arm: summary(rows, arm) for arm in ARMS}
            forest = [r for r in rows if r["arm"] == "forest_dp"]
            completions = sum(r["forest_completed"] for r in forest)
            attempts = sum(r["forest_attempts"] for r in forest)
            control, candidate = stats["inline_full"], stats["forest_dp"]
            signal = (completions > 0 and wins >= 5
                      and candidate["p50_wall_ms"] <= 0.95 * control["p50_wall_ms"]
                      and candidate["p90_wall_ms"] <= 0.95 * control["p90_wall_ms"]
                      and candidate["p50_peak_rss_kib"] <= 1.05 * control["p50_peak_rss_kib"])
            cohort.update(
                status="MATCHED_SAT_AND_SIGNED_COMPLEX_WMC",
                stats=stats, paired_wins=wins,
                forest_attempts=attempts, forest_completions=completions,
                performance="FAMILY_SIGNAL_ONLY" if signal else "PERFORMANCE_NO_GO")
            persist(args, report)
            print(f"FOREST_DP_MATCH case={case} bits={bits} "
                  f"gpmc_ms={stats['gpmc']['p50_wall_ms']:.3f} "
                  f"inline_ms={control['p50_wall_ms']:.3f} "
                  f"forest_ms={candidate['p50_wall_ms']:.3f} "
                  f"p90_inline_ms={control['p90_wall_ms']:.3f} "
                  f"p90_forest_ms={candidate['p90_wall_ms']:.3f} "
                  f"wins={wins}/{TRIALS} attempts={attempts} completions={completions} "
                  f"performance={cohort['performance']}", flush=True)
    report["status"] = "SOURCE_LOCKED_NUMERIC_EVIDENCE_NOT_PRODUCTION_GO"
    persist(args, report)
    print("FOREST_DP_STUDY_FINISHED status=" + report["status"], flush=True)


if __name__ == "__main__":
    main()
