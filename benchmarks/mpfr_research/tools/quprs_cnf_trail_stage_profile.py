#!/usr/bin/env python3
"""Source-pinned native original-CNF Trail stage attribution, never a speedup claim.

Three ordinary Feynman cases x 30/100 decimal digits: seven fresh rotated
processes for GPMC, unprofiled/profiler original Trail, unprofiled/profiler
scoped DSU. Grover5 censored records are retained without selective omission.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import statistics

from quprs_cnf_scoped_dsu_ab import (
    BITS, GPMC_SHA, QU_PRS_SHA, SOURCE, build_command, numerically_agree,
    percentile, sample, sha, source_head,
)

ARMS = ("gpmc", "inline_full", "inline_profile", "scoped_dsu", "scoped_profile")
STAGES = ("key", "lookup", "unit", "frequency", "components")
TRIALS = 7


def command(args: argparse.Namespace, arm: str, digits: int, cnf: Path, case: str) -> list[str]:
    native_arm = "gpmc" if arm == "gpmc" else (
        "scoped_dsu" if arm.startswith("scoped") else "inline_full"
    )
    cmd = build_command(args, native_arm, digits, cnf, case)
    if arm.endswith("_profile"):
        # The original parser accepts this flag after --trail-scoped-component-dsu
        # and before --hashbrown-borrowed. Never change CNF or weights.
        assert cmd[-3] == "--hashbrown-borrowed"
        cmd.insert(-3, "--trail-stage-profile")
    return cmd


def statistics_for(rows: list[dict], arm: str) -> dict:
    arm_rows = [x for x in rows if x["arm"] == arm]
    result = {
        "p50_wall_ms": statistics.median(x["wall_ns"] for x in arm_rows) / 1e6,
        "p90_wall_ms": percentile([x["wall_ns"] for x in arm_rows], .9) / 1e6,
        "p50_peak_rss_kib": statistics.median(x["peak_rss_kib"] for x in arm_rows),
    }
    if arm.endswith("_profile"):
        result["p50_solver_ms"] = statistics.median(x["solver_ns"] for x in arm_rows) / 1e6
        result["stage_p50_ms"] = {
            stage: statistics.median(x[f"stage_{stage}_ns"] for x in arm_rows) / 1e6
            for stage in STAGES
        }
        result["profiled_instrumentation_lower_bound_ms"] = sum(
            result["stage_p50_ms"].values()
        )
        result["caveat"] = (
            "Stage clocks are opt-in and exclusive but do not cover all work. "
            "They are observational lower bounds, not uninstrumented solver cost."
        )
    return result


def main() -> None:
    ap = argparse.ArgumentParser()
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, type=Path, required=True)
    args = ap.parse_args()
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("PINNED_SOURCE_REVISION_DRIFT")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "study": "Source-exact weighted Trail stage profile; not a speedup benchmark",
        "source_lock": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "precision": "GPMC -prec=30/100 decimal digits; Rust 100/333 MPFR bits",
        "arms": ARMS,
        "stage_fields": STAGES,
        "method": "7 fresh order-rotated child processes, 30s / 2GiB per arm; profile clocks opt-in",
        "status": "RUNNING",
        "cases": [],
    }
    matched = 0
    for case, relative in SOURCE.items():
        cnf = args.corpus / f"{case}-gpmc.cnf"
        source = args.quprs / relative
        if not cnf.is_file() or not source.is_file() or not cnf.read_bytes().startswith(b"p cnf "):
            raise SystemExit(f"MISSING_OR_INVALID_ORIGINAL_SOURCE: {case}")
        case_row = {
            "case": case, "cnf_sha256": sha(cnf), "qasm_sha256": sha(source),
            "cnf_bytes": cnf.stat().st_size, "precisions": [],
        }
        report["cases"].append(case_row)
        for digits, bits in BITS.items():
            trials = 1 if case == "feynman_grover_5" else TRIALS
            cohort = {"digits": digits, "bits": bits, "status": "RUNNING", "records": []}
            case_row["precisions"].append(cohort)
            for trial in range(trials):
                order = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in order:
                    entry = sample(command(args, arm, digits, cnf, case), arm, digits)
                    entry["trial"] = trial
                    cohort["records"].append(entry)
                    args.report.write_text(json.dumps(report, indent=2) + "\n")
                    print(f"STAGE_PROFILE case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={entry['status']}", flush=True)
            rows = cohort["records"]
            if len(rows) != trials * len(ARMS) or any(
                x["status"] != "COMPLETED" for x in rows
            ):
                cohort["status"] = "CENSORED_ARMS"
                if case != "feynman_grover_5":
                    args.report.write_text(json.dumps(report, indent=2) + "\n")
                    raise SystemExit(f"UNEXPECTED_CENSOR: {case} bits={bits}")
                continue
            for trial in range(trials):
                group = {x["arm"]: x for x in rows if x["trial"] == trial}
                if len(group) != len(ARMS):
                    raise SystemExit("UNBALANCED_TRIALS")
                gpmc, inline = group["gpmc"], group["inline_full"]
                expected_topology = (
                    inline["nodes"], inline["cache_hits"], inline["decompositions"]
                )
                for arm in ARMS[1:]:
                    row = group[arm]
                    if row["sat"] != gpmc["sat"] or not (
                        numerically_agree(Fraction(row["real"]), Fraction(gpmc["real"]), digits)
                        and numerically_agree(Fraction(row["imag"]), Fraction(gpmc["imag"]), digits)
                    ):
                        raise SystemExit(f"SIGNED_COMPLEX_WMC_DRIFT: {case} {bits} {trial} {arm}")
                    if (row["nodes"], row["cache_hits"], row["decompositions"]) != expected_topology:
                        raise SystemExit(f"SEARCH_TOPOLOGY_DRIFT: {case} {bits} {trial} {arm}")
                    if row.get("stage_enabled") != arm.endswith("_profile"):
                        raise SystemExit(f"STAGE_ENABLEMENT_DRIFT: {arm}")
                    if arm.endswith("_profile"):
                        if row["stage_key_ns"] <= 0 or any(
                            row[f"stage_{stage}_ns"] < 0 for stage in STAGES
                        ):
                            raise SystemExit(f"INVALID_STAGE_MEASUREMENT: {arm}")
                    elif any(row.get(f"stage_{stage}_ns", 0) for stage in STAGES):
                        raise SystemExit(f"DISABLED_STAGE_TIMER_HAS_COST: {arm}")
                if group["scoped_profile"]["scoped_dsu_slots"] > group["inline_profile"]["scoped_dsu_slots"]:
                    raise SystemExit("SCOPED_DSU_SLOT_REGRESSION")
            cohort["status"] = "MATCHED"
            cohort["stats"] = {arm: statistics_for(rows, arm) for arm in ARMS}
            matched += 1
            p = cohort["stats"]["inline_profile"]["stage_p50_ms"]
            print(f"STAGE_MATCH case={case} bits={bits} "
                  f"gpmc_ms={cohort['stats']['gpmc']['p50_wall_ms']:.3f} "
                  f"inline_ms={cohort['stats']['inline_full']['p50_wall_ms']:.3f} "
                  f"scoped_ms={cohort['stats']['scoped_dsu']['p50_wall_ms']:.3f} "
                  + " ".join(f"{k}_ms={v:.3f}" for k, v in p.items()), flush=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    report["qualified_cohorts"] = matched
    report["status"] = "MATCHED_WITH_GROVER_CENSORS" if matched == 6 else "INCOMPLETE"
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    if matched != 6:
        raise SystemExit("INSUFFICIENT_COMPLETE_ORDINARY_COHORTS")
    print("STAGE_STUDY_FINISHED qualified=6 (Grover5 censored retained)", flush=True)


if __name__ == "__main__":
    main()
