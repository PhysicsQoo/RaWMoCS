#!/usr/bin/env python3
"""Independent original-MQT CNF five-arm exact bounded Factor-VE min-fill holdout.

All inputs are original, pinned QuPRS-encoded weighted CNFs derived from
unmodified MQT Bench QASM. Source encoding is outside native solver timing.
This is not a claim that MQT Bench distributes these CNF files.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import statistics
import subprocess
import sys

THIS = Path(__file__).resolve().parent
RUST_CORE_TOOLS = THIS.parents[1] / "rust_core" / "tools"
sys.path.insert(0, str(THIS))
sys.path.insert(0, str(RUST_CORE_TOOLS))

from quprs_cnf_projected_ab import BITS, GPMC_SHA, QU_PRS_SHA, percentile, sha, source_head  # noqa: E402
from quprs_cnf_factor_minfill_ab import build_command, sample  # noqa: E402
from quprs_mqtbench_original_cnf_ab import (  # noqa: E402
    MAX_SOURCE_BYTES,
    ORIGINAL_MQT,
    complex_relative_witness,
)
from quprs_encoder_validate import parse_cnf  # noqa: E402

ARMS = ("gpmc", "inline_full", "factor_ve", "factor_ve_scope320", "factor_ve_minfill320")
TRIALS = 7
EXPECTED_COHORTS = 2 * len(ORIGINAL_MQT)
ROTATION_WALL_NS = 10_000_000_000


def preflight() -> None:
    if BITS != {30: 100, 100: 333}:
        raise SystemExit("DRIFT_IN_GPMC_TO_MPFR_BITS")
    if len(ORIGINAL_MQT) != 7:
        raise SystemExit("DRIFT_IN_PINNED_MQT_HOLDOUT")
    if ARMS != ("gpmc", "inline_full", "factor_ve", "factor_ve_scope320", "factor_ve_minfill320"):
        raise SystemExit("CHANGED_HOLDOUT_ARMS")
    print("MQT_MINFILL_PREFLIGHT_OK cases=7 precisions=2 arms=5", flush=True)


def parse_manifest(path: Path) -> dict[str, str]:
    statuses: dict[str, str] = {}
    for line in path.read_text(encoding="utf8").splitlines():
        parts = line.split("\t")
        if len(parts) != 2 or parts[0] not in ORIGINAL_MQT or parts[0] in statuses:
            raise SystemExit("INVALID_SOURCE_MANIFEST")
        statuses[parts[0]] = parts[1]
    if set(statuses) != set(ORIGINAL_MQT):
        raise SystemExit("MISSING_ORIGINAL_SOURCE_ATTEMPT")
    return statuses


def group_stats(records: list[dict], arm: str) -> dict:
    selected = [r for r in records if r["arm"] == arm]
    return {
        "p50_wall_ms": statistics.median(r["wall_ns"] for r in selected) / 1e6,
        "p90_wall_ms": percentile([r["wall_ns"] for r in selected], 0.9) / 1e6,
        "p50_peak_rss_kib": statistics.median(r["peak_rss_kib"] for r in selected),
        "p50_nodes": (statistics.median(r["nodes"] for r in selected)
                      if arm != "gpmc" else None),
        "p50_cache_hits": (statistics.median(r["cache_hits"] for r in selected)
                           if arm != "gpmc" else None),
        "p50_decompositions": (statistics.median(r["decompositions"] for r in selected)
                               if arm != "gpmc" else None),
        "factor_ve_completed": (all(r["factor_ve_completed"] for r in selected)
                                if arm != "gpmc" else None),
        "factor_ve_capped": (any(r["factor_ve_capped"] for r in selected)
                             if arm != "gpmc" else None),
        "factor_ve_products": (statistics.median(r["factor_ve_products"] for r in selected)
                               if arm != "gpmc" else None),
        "factor_ve_cells": (statistics.median(r["factor_ve_cells"] for r in selected)
                            if arm != "gpmc" else None),
        "factor_ve_width": (statistics.median(r["factor_ve_width"] for r in selected)
                            if arm != "gpmc" else None),
        "factor_ve_cap_reasons": (sorted(set(r["factor_ve_cap_reason"] for r in selected))
                                 if arm != "gpmc" else None),
        "minfill_predicted_width": (statistics.median(r["minfill_predicted_width"] for r in selected)
                                    if arm != "gpmc" else None),
        "minfill_fill_edges": (statistics.median(r["minfill_fill_edges"] for r in selected)
                              if arm != "gpmc" else None),
        "minfill_plan_ns": (statistics.median(r["minfill_plan_ns"] for r in selected)
                           if arm != "gpmc" else None),

    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, type=Path)
    args = ap.parse_args()
    preflight()
    if args.self_test:
        return
    for name in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            ap.error(f"missing --{name.replace('_', '-')}")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("SOURCE_SHA_DRIFT")
    statuses = parse_manifest(args.corpus / "source-status.tsv")
    report = {
        "study": "Original MQT Bench QASM to exact same QuPRS Weighted CNF five-arm bounded min-fill factor order",
        "not_mqt_published_cnf": True,
        "quprs_sha": QU_PRS_SHA, "gpmc_sha": GPMC_SHA,
        "precision": "GPMC 30/100 decimal digits <-> Rust 100/333 MPFR bits",
        "solver_input": "original unchanged QuPRS emitted GPMC Weighted CNF bytes",
        "arms": list(ARMS),
        "resource": "native Linux 30s and 2 GiB address space per fresh child; GNU time RSS observed",
        "trials": TRIALS,
        "evidence": "Boolean SAT and two signed Complex WMC fields, no independent large WMC oracle",
        "cases": [], "qualified_cohorts": 0,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)

    def save() -> None:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")

    for case, source_name in ORIGINAL_MQT.items():
        source = args.quprs / "benchmarks" / "MQTBench" / source_name
        if not source.is_file():
            raise SystemExit(f"PINNED_QASM_MISSING_{case}")
        case_record = {
            "case": case,
            "source_relpath": str(source.relative_to(args.quprs)),
            "source_qasm_sha256": sha(source),
            "export_status": statuses[case],
            "precisions": [],
        }
        report["cases"].append(case_record)
        if statuses[case] != "EXPORTED":
            case_record["status"] = "CENSORED_EXPORT"
            save()
            print(f"MQT_FACTOR_VE_CENSORED case={case} source={statuses[case]}", flush=True)
            continue
        copied = args.corpus / f"{case}.qasm"
        cnf = args.corpus / f"{case}-gpmc.cnf"
        ganak = args.corpus / f"{case}-ganak.cnf"
        gp = args.corpus / f"{case}-gpmc-phases.tsv"
        kp = args.corpus / f"{case}-ganak-phases.tsv"
        sources = (copied, cnf, ganak, gp, kp)
        if not all(f.is_file() and f.stat().st_size <= MAX_SOURCE_BYTES for f in sources):
            case_record["status"] = "CENSORED_SOURCE_OR_OUTPUT_SIZE"
            save()
            continue
        if source.read_bytes() != copied.read_bytes():
            raise SystemExit(f"MODIFIED_ORIGINAL_MQT_QASM_{case}")
        try:
            a = parse_cnf(cnf, "gpmc")
            b = parse_cnf(ganak, "ganak")
        except ValueError as error:
            case_record.update(status="CENSORED_CNF_DIALECT", error=str(error))
            save()
            continue
        if ((a["variables"], a["clauses"], a["weights"])
                != (b["variables"], b["clauses"], b["weights"])
                or gp.read_bytes() != kp.read_bytes()):
            raise SystemExit(f"DUAL_CNF_OR_PHASE_DISAGREEMENT_{case}")
        case_record.update({
            "cnf_sha256": sha(cnf),
            "ganak_sha256": sha(ganak),
            "phase_sidecar_sha256": sha(gp),
            "variables": a["variables"],
            "clauses": a["clauses_count"],
        })
        for digits, bits in BITS.items():
            cohort = {"digits": digits, "bits": bits, "records": [], "status": "INCOMPLETE"}
            case_record["precisions"].append(cohort)
            save()
            for trial in range(TRIALS):
                rotated = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in rotated:
                    command = build_command(args, arm, digits, cnf, case)
                    if arm != "gpmc":
                        # Same MQT precedent: identical 1M node budget in both Rust arms.
                        command.insert(1, "--node-budget=1000000")
                        # build_command() pins exactly the Feynman five-arm
                        # CLI flags and returns unchanged original-CNF input.

                    item = sample(command, arm, digits)
                    item["trial"] = trial
                    cohort["records"].append(item)
                    save()
                    print(f"MQT_MINFILL case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={item['status']} "
                          f"nodes={item.get('nodes', 'NA')} "
                          f"ve_completed={item.get('factor_ve_completed', 'NA')}", flush=True)
                if trial == 0:
                    first = cohort["records"][:len(ARMS)]
                    if any(r["status"] != "COMPLETED"
                           or r.get("wall_ns", ROTATION_WALL_NS + 1) > ROTATION_WALL_NS
                           for r in first):
                        cohort["status"] = "CENSORED_FIRST_ROTATION"
                        break
            if cohort["status"] == "CENSORED_FIRST_ROTATION":
                save()
                continue
            records = cohort["records"]
            if len(records) != TRIALS * len(ARMS) or any(
                r["status"] != "COMPLETED" for r in records
            ):
                cohort["status"] = "CENSORED_INCOMPLETE_ROTATIONS"
                save()
                continue
            for trial in range(TRIALS):
                selected = {r["arm"]: r for r in records if r["trial"] == trial}
                if set(selected) != set(ARMS):
                    raise SystemExit("UNBALANCED_MQT_PAIRED_TRIAL")
                baseline, rust = (selected[arm] for arm in ARMS[:2])
                if len({selected[arm]["sat"] for arm in ARMS}) != 1:
                    cohort["status"] = "FAILED_BOOLEAN_SAT"
                    save()
                    raise SystemExit("FAILED_BOOLEAN_SAT")
                for arm in ARMS[1:]:
                    result = selected[arm]
                    witness = complex_relative_witness(baseline, result, digits)
                    if not witness["pass"]:
                        cohort["status"] = "FAILED_SIGNED_COMPLEX_WMC"
                        cohort["failing_witness"] = {
                            "trial": trial, "arm": arm, "witness": witness,
                        }
                        save()
                        raise SystemExit("FAILED_SIGNED_COMPLEX_WMC")
                if rust["factor_ve_enabled"]:
                    raise SystemExit("INLINE_CONTROL_CONTAMINATED_FACTOR_VE")
                for arm in ARMS[2:]:
                    candidate = selected[arm]
                    if not candidate["factor_ve_enabled"]:
                        raise SystemExit("MISSING_FACTOR_VE_FLAG")
                    if candidate["factor_ve_completed"]:
                        if (candidate["factor_ve_width"] > 11
                                or candidate["factor_ve_products"] > 600000
                                or candidate["factor_ve_cells"] > 150000):
                            raise SystemExit("COMPLETED_FACTOR_VE_OVER_BUDGET")
                    else:
                        if not candidate["factor_ve_capped"]:
                            raise SystemExit("INCOMPLETE_FACTOR_VE_WITHOUT_CAP")
                        if candidate["factor_ve_width"] > 11 and candidate["factor_ve_cap_reason"] != "bucket_width":
                            raise SystemExit("BUCKET_REJECTION_REASON_DRIFT")
                        if (candidate["nodes"], candidate["cache_hits"], candidate["decompositions"]) != (
                            rust["nodes"], rust["cache_hits"], rust["decompositions"]
                        ):
                            raise SystemExit("FAILED_FACTOR_FALLBACK_TOPOLOGY")
                    if candidate["separator_scans"] != 0 or candidate["separator_chosen"] != 0:
                        raise SystemExit("PIVOT_CONTAMINATION")
                if rust["separator_scans"] != 0 or rust["separator_chosen"] != 0:
                    raise SystemExit("INLINE_PIVOT_CONTAMINATION")
                for arm in ("factor_ve_scope320", "factor_ve_minfill320"):
                    if not selected[arm]["factor_ve_scope_probe_enabled"]:
                        raise SystemExit("SCOPE_320_FLAG_MISSING")
                if selected["factor_ve"]["factor_ve_scope_probe_enabled"]:
                    raise SystemExit("BASELINE_SCOPE_256_DRIFT")
                if not selected["factor_ve_minfill320"]["factor_ve_minfill_enabled"]:
                    raise SystemExit("MINFILL_FLAG_MISSING")
                if selected["factor_ve_scope320"]["factor_ve_minfill_enabled"]:
                    raise SystemExit("MIN_UNION_CONTROL_CONTAMINATION")
                if (selected["factor_ve_minfill320"]["minfill_predicted_width"] > 11
                        and selected["factor_ve_minfill320"]["factor_ve_completed"]):
                    raise SystemExit("UNSAFE_MINFILL_WIDTH")
            cohort["status"] = "MATCHED_SIGNED_COMPLEX_WMC"
            cohort["stats"] = {arm: group_stats(records, arm) for arm in ARMS}
            report["qualified_cohorts"] += 1
            save()
            print(f"MQT_MINFILL_MATCH case={case} bits={bits} "
                  f"gpmc_ms={cohort['stats']['gpmc']['p50_wall_ms']:.3f} "
                  f"inline_ms={cohort['stats']['inline_full']['p50_wall_ms']:.3f} "
                  f"factor_ve_ms={cohort['stats']['factor_ve']['p50_wall_ms']:.3f} "
                  f"nodes_inline={cohort['stats']['inline_full']['p50_nodes']} "
                  f"ve_products={cohort['stats']['factor_ve']['factor_ve_products']} "
                  f"ve_complete={cohort['stats']['factor_ve']['factor_ve_completed']} " +
                  f"ve_width={cohort['stats']['factor_ve']['factor_ve_width']} " +
                  f"wide_ms={cohort['stats']['factor_ve_scope320']['p50_wall_ms']:.3f} " +
                  f"minfill_ms={cohort['stats']['factor_ve_minfill320']['p50_wall_ms']:.3f} " +
                  f"minfill_width={cohort['stats']['factor_ve_minfill320']['factor_ve_width']} " +
                  f"minfill_products={cohort['stats']['factor_ve_minfill320']['factor_ve_products']} " +
                  f"minfill_cap={','.join(cohort['stats']['factor_ve_minfill320']['factor_ve_cap_reasons'])}", flush=True)

    report["status"] = (
        "ALL_ORIGINAL_MQT_HOLDOUT_COHORTS_QUALIFIED"
        if report["qualified_cohorts"] == EXPECTED_COHORTS
        else "PARTIAL_OR_CENSORED_MQT_HOLDOUT"
    )
    save()
    print(f"MQT_MINFILL_FINISHED qualified={report['qualified_cohorts']} "
          f"expected={EXPECTED_COHORTS} status={report['status']}", flush=True)
    if report["qualified_cohorts"] != EXPECTED_COHORTS:
        raise SystemExit("INCOMPLETE_MQT_MINFILL_HOLDOUT_NO_GLOBAL_SPEED_CLAIM")


if __name__ == "__main__":
    main()
