#!/usr/bin/env python3
"""Source-pinned exact transient key arena A/B versus original GPMC.

Three ordinary Feynman cases x two matched precisions; seven order-rotated
fresh native trials per cohort. Profiled and unprofiled arms are kept separate.
Grover5 censored outcomes must be preserved in the full raw evidence.
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

ARMS = (
    "gpmc", "inline_full", "ordered_key", "arena_reuse",
    "inline_profile", "ordered_profile", "arena_profile",
)
STAGES = ("key", "lookup", "unit", "frequency", "components")
TRIALS = 7


def command(args: argparse.Namespace, arm: str, digits: int, cnf: Path, case: str) -> list[str]:
    native_arm = "gpmc" if arm == "gpmc" else "inline_full"
    cmd = build_command(args, native_arm, digits, cnf, case)
    assert arm in ARMS
    if arm.endswith("_profile"):
        assert cmd[-3] == "--hashbrown-borrowed"
        cmd.insert(-3, "--trail-stage-profile")
    if arm.startswith("ordered"):
        assert cmd[-3] == "--hashbrown-borrowed"
        cmd.insert(-3, "--trail-ordered-key-fastpath")
    if arm.startswith("arena"):
        assert cmd[-3] == "--hashbrown-borrowed"
        cmd.insert(-3, "--trail-reuse-key-arena")
    return cmd


def statistics_for(rows: list[dict], arm: str) -> dict:
    arm_rows = [x for x in rows if x["arm"] == arm]
    result = {
        "p50_wall_ms": statistics.median(x["wall_ns"] for x in arm_rows) / 1e6,
        "p90_wall_ms": percentile([x["wall_ns"] for x in arm_rows], .9) / 1e6,
        "p50_peak_rss_kib": statistics.median(x["peak_rss_kib"] for x in arm_rows),
    }
    if arm.startswith("arena"):
        result["p50_key_arena_calls"] = statistics.median(
            x["key_arena_calls"] for x in arm_rows
        )
        result["p50_key_arena_flat_reuses"] = statistics.median(
            x["key_arena_flat_reuses"] for x in arm_rows
        )
        result["p50_key_arena_view_reuses"] = statistics.median(
            x["key_arena_view_reuses"] for x in arm_rows
        )
    if arm.startswith("ordered"):
        result["p50_ordered_key_probes"] = statistics.median(
            x["ordered_key_probes"] for x in arm_rows
        )
        result["p50_ordered_key_hits"] = statistics.median(
            x["ordered_key_hits"] for x in arm_rows
        )
    if arm.endswith("_profile"):
        result["p50_solver_ms"] = statistics.median(x["solver_ns"] for x in arm_rows) / 1e6
        result["stage_p50_ms"] = {
            stage: statistics.median(x[f"stage_{stage}_ns"] for x in arm_rows) / 1e6
            for stage in STAGES
        }
        result["key_subphase_p50_ms"] = {
            phase: statistics.median(x[f"key_{phase}_ns"] for x in arm_rows) / 1e6
            for phase in ("materialize", "order", "pack")
        }
        result["p50_key_unattributed_ms"] = statistics.median(
            x["key_unattributed_ns"] for x in arm_rows
        ) / 1e6
        result["p50_instrumented_stage_sum_ms"] = statistics.median(
            sum(x[f"stage_{stage}_ns"] for stage in STAGES) for x in arm_rows
        ) / 1e6
        result["p50_unattributed_solver_ms"] = statistics.median(
            x["solver_ns"] - sum(x[f"stage_{stage}_ns"] for stage in STAGES)
            for x in arm_rows
        ) / 1e6
        result["p50_stage_coverage_fraction"] = statistics.median(
            sum(x[f"stage_{stage}_ns"] for stage in STAGES) / x["solver_ns"]
            for x in arm_rows
        )
        result["caveat"] = (
            "Each stage timer is opt-in and exclusive. The per-trial stage sum "
            "does not exhaust solve time, and its p50 need not equal the sum "
            "of the independent stage p50 values."
        )
    return result



def validate_stage(row: dict, arm: str) -> None:
    """Fail closed on disabled clocks, overlapping clocks and invalid solver time."""
    enabled = arm.endswith("_profile")
    if row.get("stage_enabled") != enabled:
        raise ValueError(f"STAGE_ENABLEMENT_DRIFT: {arm}")
    if row.get("key_subphase_enabled") != enabled:
        raise ValueError(f"KEY_SUBPHASE_ENABLEMENT_DRIFT: {arm}")
    subphases = [row[f"key_{part}_ns"] for part in ("materialize", "order", "pack")]
    if enabled:
        stage_values = [row[f"stage_{stage}_ns"] for stage in STAGES]
        solver_ns = row["solver_ns"]
        stage_sum = sum(stage_values)
        if stage_values[0] <= 0 or min(stage_values) < 0 or solver_ns <= 0 or stage_sum > solver_ns:
            raise ValueError(f"INVALID_OR_OVERLAPPING_STAGE_MEASUREMENT: {arm}")
        row["stage_sum_ns"] = stage_sum
        row["unattributed_solver_ns"] = solver_ns - stage_sum
        row["stage_coverage_fraction"] = stage_sum / solver_ns
        key_sum = sum(subphases)
        if min(subphases) < 0 or key_sum <= 0 or key_sum > row["stage_key_ns"]:
            raise ValueError(f"INVALID_OR_OVERLAPPING_KEY_SUBPHASE: {arm}")
        row["key_subphase_sum_ns"] = key_sum
        row["key_unattributed_ns"] = row["stage_key_ns"] - key_sum
        row["key_subphase_coverage_fraction"] = key_sum / row["stage_key_ns"]
    elif any(row.get(f"stage_{stage}_ns", 0) for stage in STAGES) or any(subphases):
        raise ValueError(f"DISABLED_STAGE_OR_KEY_TIMER_HAS_COST: {arm}")


def self_test() -> None:
    good = {
        "arm": "inline_profile", "wall_ns": 140, "peak_rss_kib": 12,
        "solver_ns": 100, "stage_enabled": True, "stage_key_ns": 20,
        "stage_lookup_ns": 10, "stage_unit_ns": 10,
        "stage_frequency_ns": 5, "stage_components_ns": 5,
        "key_subphase_enabled": True, "key_materialize_ns": 8,
        "key_order_ns": 4, "key_pack_ns": 3,
    }
    validate_stage(good, "inline_profile")
    assert good["stage_sum_ns"] == 50
    assert good["unattributed_solver_ns"] == 50
    assert good["stage_coverage_fraction"] == .5
    assert good["key_subphase_sum_ns"] == 15
    assert good["key_unattributed_ns"] == 5
    stats = statistics_for([good], "inline_profile")
    assert stats["p50_instrumented_stage_sum_ms"] == .00005
    assert stats["p50_unattributed_solver_ms"] == .00005
    for invalid in (
        {**good, "stage_enabled": False},
        {**good, "stage_key_ns": 101},
        {**good, "solver_ns": 0},
        {**good, "stage_unit_ns": -1},
        {**good, "key_subphase_enabled": False},
        {**good, "key_materialize_ns": 25},
        {**good, "key_order_ns": -1},
    ):
        try:
            validate_stage(invalid, "inline_profile")
        except ValueError:
            pass
        else:
            raise AssertionError("invalid profiled stage evidence passed")
    try:
        validate_stage({
            **good, "stage_enabled": False, "key_subphase_enabled": False,
            "stage_key_ns": 1,
        }, "inline_full")
    except ValueError:
        pass
    else:
        raise AssertionError("disabled profiler accepted nonzero clock")
    validate_stage({
        "stage_enabled": False, "stage_key_ns": 0,
        "key_subphase_enabled": False,
        "key_materialize_ns": 0, "key_order_ns": 0, "key_pack_ns": 0,
    }, "inline_full")
    print("ARENA_REUSE_PROFILE_ORACLE_SELF_TEST_PASS", flush=True)

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, type=Path)
    args = ap.parse_args()
    if args.self_test:
        self_test()
        return
    for name in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            ap.error(f"missing --{name.replace('_', '-')}")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("PINNED_SOURCE_REVISION_DRIFT")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "study": "Source-exact bounded transient key arena reuse versus Inline, Ordered and original GPMC",
        "source_lock": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "precision": "GPMC -prec=30/100 decimal digits; Rust 100/333 MPFR bits",
        "arms": ARMS,
        "stage_fields": STAGES,
        "method": "7 fresh order-rotated child processes, 30s / 2GiB per arm; bounded exact transient key arena reuse explicitly opt-in",
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
                    print(f"KEY_ARENA_TRIAL case={case} bits={bits} trial={trial} "
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
                    if "key_arena_enabled" not in row or "key_arena_calls" not in row:
                        raise SystemExit(f"MISSING_SOURCE_EXACT_KEY_ARENA_DIAGNOSTICS: {arm}")
                    expected_ordered = arm.startswith("ordered")
                    expected_arena = arm.startswith("arena")
                    if row.get("scope_stamp_enabled") or row.get("scope_stamp_probes", 0):
                        raise SystemExit(f"UNRELATED_SCOPE_STAMP_ARM_CONTAMINATION: {arm}")
                    if row.get("active_bitmap_enabled") or row.get("active_bitmap_key_visits", 0):
                        raise SystemExit(f"UNRELATED_ACTIVE_BITMAP_ARM_CONTAMINATION: {arm}")
                    if row.get("key_arena_enabled") != expected_arena:
                        raise SystemExit(f"KEY_ARENA_ARM_CONTAMINATION: {arm}")
                    if expected_arena:
                        calls = row.get("key_arena_calls", 0)
                        flat = row.get("key_arena_flat_reuses", 0)
                        views = row.get("key_arena_view_reuses", 0)
                        if calls <= 0 or not (0 <= flat <= calls and 0 <= views <= calls):
                            raise SystemExit(f"INVALID_ARENA_REUSE_COUNTERS: {arm}")
                    elif any(row.get(f"key_arena_{field}", 0) for field in ("calls", "flat_reuses", "view_reuses")):
                        raise SystemExit(f"DISABLED_KEY_ARENA_HAS_COUNTERS: {arm}")
                    if row.get("ordered_key_enabled") != expected_ordered:
                        raise SystemExit(f"ORDERED_KEY_ARM_CONTAMINATION: {arm}")
                    if expected_ordered:
                        if row["ordered_key_probes"] <= 0 or not (
                            0 <= row["ordered_key_hits"] <= row["ordered_key_probes"]
                        ):
                            raise SystemExit(f"INVALID_ORDERED_KEY_COUNTERS: {arm}")
                    elif row.get("ordered_key_hits", 0) or row.get("ordered_key_probes", 0):
                        raise SystemExit(f"DISABLED_ORDERED_KEY_COUNTERS: {arm}")
                    if row["sat"] != gpmc["sat"] or not (
                        numerically_agree(Fraction(row["real"]), Fraction(gpmc["real"]), digits)
                        and numerically_agree(Fraction(row["imag"]), Fraction(gpmc["imag"]), digits)
                    ):
                        raise SystemExit(f"SIGNED_COMPLEX_WMC_DRIFT: {case} {bits} {trial} {arm}")
                    if row["precision_bits"] != bits:
                        raise SystemExit(f"RUST_PRECISION_DRIFT: {case} {bits} {trial} {arm}")
                    if row["stdout_sha256"] != inline["stdout_sha256"]:
                        raise SystemExit(f"RUST_SIGNED_COMPLEX_OUTPUT_BYTES_DRIFT: {case} {bits} {trial} {arm}")
                    if (row["nodes"], row["cache_hits"], row["decompositions"]) != expected_topology:
                        raise SystemExit(f"SEARCH_TOPOLOGY_DRIFT: {case} {bits} {trial} {arm}")
                    try:
                        validate_stage(row, arm)
                    except (KeyError, ValueError, TypeError) as exc:
                        raise SystemExit(str(exc)) from exc
                for arm in ARMS[1:]:
                    if group[arm]["scoped_dsu_enabled"]:
                        raise SystemExit("UNEXPECTED_SCOPED_DSU_FLAG")
            cohort["status"] = "MATCHED"
            cohort["stats"] = {arm: statistics_for(rows, arm) for arm in ARMS}
            for target_arm, control_arm in (
                ("ordered_key", "inline_full"),
                ("arena_reuse", "inline_full"),
                ("inline_profile", "inline_full"),
                ("ordered_profile", "ordered_key"),
                ("arena_profile", "arena_reuse"),
            ):
                differences = []
                for trial in range(trials):
                    matched_pairs = {e["arm"]: e for e in rows if e["trial"] == trial}
                    differences.append(
                        matched_pairs[target_arm]["wall_ns"] -
                        matched_pairs[control_arm]["wall_ns"]
                    )
                cohort["stats"][target_arm]["p50_paired_complete_wall_delta_ms"] = (
                    statistics.median(differences) / 1e6
                )
                cohort["stats"][target_arm]["paired_trails_faster"] = sum(
                    difference < 0 for difference in differences
                )
                if target_arm.endswith("_profile"):
                    cohort["stats"][target_arm][
                        "p50_paired_profile_wall_overhead_ms"
                    ] = statistics.median(differences) / 1e6
            matched += 1
            p = cohort["stats"]["inline_profile"]["stage_p50_ms"]
            print(f"KEY_ARENA_MATCH case={case} bits={bits} "
                  f"gpmc_ms={cohort['stats']['gpmc']['p50_wall_ms']:.3f} "
                  f"inline_ms={cohort['stats']['inline_full']['p50_wall_ms']:.3f} "
                  f"ordered_ms={cohort['stats']['ordered_key']['p50_wall_ms']:.3f} "
                  f"arena_ms={cohort['stats']['arena_reuse']['p50_wall_ms']:.3f} "
                  f"arena_flat_reuses={cohort['stats']['arena_reuse']['p50_key_arena_flat_reuses']} "
                  f"ordered_hits={cohort['stats']['ordered_key']['p50_ordered_key_hits']} "
                  f"ordered_probes={cohort['stats']['ordered_key']['p50_ordered_key_probes']} "
                  + " ".join(f"{k}_ms={v:.3f}" for k, v in p.items()), flush=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    report["qualified_cohorts"] = matched
    report["status"] = "MATCHED_WITH_GROVER_CENSORS" if matched == 6 else "INCOMPLETE"
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    if matched != 6:
        raise SystemExit("INSUFFICIENT_COMPLETE_ORDINARY_COHORTS")
    print("KEY_ARENA_STUDY_FINISHED qualified=6 (Grover5 censored retained)", flush=True)


if __name__ == "__main__":
    main()
