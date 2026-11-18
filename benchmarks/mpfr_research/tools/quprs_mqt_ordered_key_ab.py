#!/usr/bin/env python3
"""Independent original MQT QASM -> QuPRS Weighted CNF ordered-key holdout.

The inputs are original, source-locked QuPRS-emitted CNFs, not CNFs
published by MQT Bench. The same original CNF bytes and signed
Complex weights are used for GPMC and every Rust arm.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics
import sys

THIS = Path(__file__).resolve().parent
RUST_CORE = THIS.parents[1] / "rust_core" / "tools"
sys.path.insert(0, str(THIS))
sys.path.insert(0, str(RUST_CORE))

from quprs_cnf_ordered_key_ab import (  # noqa: E402
    ARMS, BITS, GPMC_SHA, QU_PRS_SHA, STAGES, TRIALS,
    command, sample, sha, source_head, statistics_for, validate_stage,
)
from quprs_mqtbench_original_cnf_ab import (  # noqa: E402
    ORIGINAL_MQT, MAX_SOURCE_BYTES, complex_relative_witness,
)
from quprs_encoder_validate import parse_cnf  # noqa: E402

MAX_FIRST_TRIAL_WALL_NS = 10_000_000_000
EXPECTED_COHORTS = 2 * len(ORIGINAL_MQT)


def preflight() -> None:
    if BITS != {30: 100, 100: 333}:
        raise SystemExit("PINNED_PRECISION_DRIFT")
    if (len(ORIGINAL_MQT), len(ARMS), TRIALS) != (7, 5, 7):
        raise SystemExit("CHANGED_INDEPENDENT_HOLDOUT_DESIGN")
    if ARMS != (
        "gpmc", "inline_full", "ordered_key", "inline_profile",
        "ordered_profile",
    ):
        raise SystemExit("CHANGED_ORDERED_KEY_ARMS")


def parse_manifest(path: Path) -> dict[str, str]:
    statuses: dict[str, str] = {}
    for line in path.read_text(encoding="utf8").splitlines():
        fields = line.split("\t")
        if (
            len(fields) != 2 or fields[0] not in ORIGINAL_MQT
            or fields[0] in statuses
        ):
            raise SystemExit("INVALID_PINNED_SOURCE_MANIFEST")
        statuses[fields[0]] = fields[1]
    if set(statuses) != set(ORIGINAL_MQT):
        raise SystemExit("MISSING_ORIGINAL_MQT_SOURCE_ATTEMPTS")
    return statuses


def certify_trial(records: list[dict], trial: int, digits: int) -> None:
    group = {r["arm"]: r for r in records if r["trial"] == trial}
    if set(group) != set(ARMS) or len([r for r in records if r["trial"] == trial]) != len(ARMS):
        raise ValueError("UNBALANCED_OR_DUPLICATE_NATIVE_TRIAL")
    reference = group["gpmc"]
    control = group["inline_full"]
    baseline_topology = (
        control["nodes"], control["cache_hits"], control["decompositions"],
    )
    if len({r["sat"] for r in group.values()}) != 1:
        raise ValueError("ORIGINAL_MQT_SAT_DISAGREEMENT")
    for arm in ARMS[1:]:
        row = group[arm]
        witness = complex_relative_witness(reference, row, digits)
        if not witness["pass"]:
            raise ValueError(f"ORIGINAL_MQT_SIGNED_COMPLEX_DRIFT arm={arm} witness={witness}")
        if row["precision_bits"] != BITS[digits]:
            raise ValueError(f"RUST_PRECISION_DRIFT arm={arm}")
        topology = (
            row["nodes"], row["cache_hits"], row["decompositions"],
        )
        if topology != baseline_topology:
            raise ValueError(f"ORDERED_KEY_SEARCH_TOPOLOGY_DRIFT arm={arm}")
        if row["stdout_sha256"] != control["stdout_sha256"]:
            raise ValueError(f"RUST_SIGNED_COMPLEX_OUTPUT_BYTES_DRIFT arm={arm}")
        if row.get("ordered_key_enabled") != arm.startswith("ordered"):
            raise ValueError(f"ORDERED_KEY_FLAG_MISMATCH arm={arm}")
        probes = row.get("ordered_key_probes", 0)
        hits = row.get("ordered_key_hits", 0)
        if not (0 <= hits <= probes):
            raise ValueError(f"ORDERED_KEY_HITS_OUTSIDE_PROBES arm={arm}")
        if not arm.startswith("ordered") and (hits != 0 or probes != 0):
            raise ValueError(f"DISABLED_ORDERED_KEY_HAS_PROBES arm={arm}")
        if row.get("scoped_dsu_enabled") or row.get("factor_ve_enabled"):
            raise ValueError(f"UNRELATED_SEARCH_ARM_ENABLED arm={arm}")
        validate_stage(row, arm)


def self_test() -> None:
    preflight()

    def fake(arm: str) -> dict:
        profiled = arm.endswith("_profile")
        ordered = arm.startswith("ordered")
        return {
            "arm": arm, "trial": 0,
            "sat": True, "real": "1/2", "imag": "-1/8",
            "nodes": 10, "cache_hits": 2, "decompositions": 1,
            "stdout_sha256": "unchanged-signed-complex-output",
            "precision_bits": 100, "scoped_dsu_enabled": False,
            "factor_ve_enabled": False,
            "ordered_key_enabled": ordered,
            "ordered_key_probes": 2 if ordered else 0,
            "ordered_key_hits": 0,
            "stage_enabled": profiled,
            "stage_key_ns": 5 if profiled else 0,
            "stage_lookup_ns": 2 if profiled else 0,
            "stage_unit_ns": 2 if profiled else 0,
            "stage_frequency_ns": 2 if profiled else 0,
            "stage_components_ns": 2 if profiled else 0,
            "solver_ns": 100,
        }

    baseline = [fake(arm) for arm in ARMS]
    certify_trial(baseline, 0, 30)
    for arm, field, bad in (
        ("ordered_key", "imag", "1/8"),
        ("ordered_key", "nodes", 11),
        ("ordered_key", "ordered_key_enabled", False),
        ("ordered_key", "ordered_key_hits", 3),
        ("ordered_key", "precision_bits", 333),
        ("ordered_profile", "stage_enabled", False),
        ("ordered_profile", "stage_key_ns", 200),
    ):
        records = [dict(row) for row in baseline]
        candidate = next(r for r in records if r["arm"] == arm)
        candidate[field] = bad
        try:
            certify_trial(records, 0, 30)
        except (KeyError, ValueError, TypeError):
            pass
        else:
            raise AssertionError(f"invalid trial passed {arm} {field}")
    print("ORIGINAL_MQT_ORDERED_KEY_PREFLIGHT_PASS", flush=True)


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--self-test", action="store_true")
    for field in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        p.add_argument("--" + field, type=Path)
    args = p.parse_args()
    preflight()
    if args.self_test:
        self_test()
        return
    for name in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            p.error(f"missing --{name.replace('_', '-')}")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("ORIGINAL_MQT_QU_PRS_OR_GPMC_SHA_DRIFT")
    statuses = parse_manifest(args.corpus / "source-status.tsv")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "dataset": "PINNED_ORIGINAL_MQT_QASM_TO_ORIGINAL_QU_PRS_WEIGHTED_CNF",
        "not_mqt_published_cnf": True,
        "quprs_sha": QU_PRS_SHA,
        "gpmc_sha": GPMC_SHA,
        "precision": "GPMC 30/100 decimal digits; Rust 100/333 MPFR bits",
        "arms": list(ARMS),
        "trials_per_complete_cohort": TRIALS,
        "resource": "30s and 2GiB address-space per fresh process; RSS observed",
        "go_no_go": "Numeric exactness required; repeatable unprofiled full-process WallTime and independent out-of-family improvement required for promotion",
        "oracle": "Pinned source and exact signed Complex cross-check, NOT independent large-CNF full WMC proof",
        "expected_cohorts": EXPECTED_COHORTS,
        "qualified_cohorts": 0,
        "cases": [],
        "status": "RUNNING",
    }

    def save() -> None:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")

    for case, filename in ORIGINAL_MQT.items():
        original = args.quprs / "benchmarks" / "MQTBench" / filename
        if not original.is_file():
            raise SystemExit(f"ORIGINAL_QASM_NOT_FOUND: {case}")
        item = {
            "case": case,
            "source_relpath": str(original.relative_to(args.quprs)),
            "source_sha256": sha(original),
            "export_status": statuses[case],
            "precisions": [],
        }
        report["cases"].append(item)
        save()
        if statuses[case] != "EXPORTED":
            item["status"] = "CENSORED_SOURCE_EXPORT"
            save()
            continue
        copied = args.corpus / f"{case}.qasm"
        cnf = args.corpus / f"{case}-gpmc.cnf"
        ganak = args.corpus / f"{case}-ganak.cnf"
        gp = args.corpus / f"{case}-gpmc-phases.tsv"
        kp = args.corpus / f"{case}-ganak-phases.tsv"
        files = (copied, cnf, ganak, gp, kp)
        if not all(
            x.is_file() and 0 < x.stat().st_size <= MAX_SOURCE_BYTES
            for x in files
        ):
            item["status"] = "CENSORED_SOURCE_OR_SIZE_LIMIT"
            save()
            continue
        if original.read_bytes() != copied.read_bytes():
            raise SystemExit(f"ORIGINAL_QASM_BYTE_DRIFT: {case}")
        try:
            g = parse_cnf(cnf, "gpmc")
            k = parse_cnf(ganak, "ganak")
        except ValueError as error:
            item.update(status="CENSORED_CNF_DIALECT", error=str(error))
            save()
            continue
        if (
            (g["variables"], g["clauses"], g["weights"])
            != (k["variables"], k["clauses"], k["weights"])
            or gp.read_bytes() != kp.read_bytes()
        ):
            raise SystemExit(f"EXACT_WEIGHTED_CNF_OR_PHASE_DRIFT: {case}")
        item.update(
            cnf_sha256=sha(cnf),
            ganak_sha256=sha(ganak),
            phase_sidecar_sha256=sha(gp),
            variables=g["variables"],
            clauses=g["clauses_count"],
        )
        save()
        for digits, bits in BITS.items():
            cohort = {
                "digits": digits,
                "bits": bits,
                "records": [],
                "status": "RUNNING",
            }
            item["precisions"].append(cohort)
            save()
            for trial in range(TRIALS):
                rotated = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in rotated:
                    cmd = command(args, arm, digits, cnf, case)
                    if arm != "gpmc":
                        # The independent source-control MQT benchmark uses
                        # the same 1M-node budget for all Rust variants.
                        cmd.insert(1, "--node-budget=1000000")
                    result = sample(cmd, arm, digits)
                    result["trial"] = trial
                    cohort["records"].append(result)
                    save()
                    print(
                        f"MQT_ORDERED case={case} bits={bits} trial={trial} arm={arm} "
                        f"status={result['status']} wall_ms={result.get('wall_ns', 0)/1e6:.3f}",
                        flush=True,
                    )
                if trial == 0 and any(
                    r["status"] != "COMPLETED"
                    or r.get("wall_ns", MAX_FIRST_TRIAL_WALL_NS + 1) > MAX_FIRST_TRIAL_WALL_NS
                    for r in cohort["records"]
                ):
                    cohort["status"] = "CENSORED_FIRST_ROTATION"
                    save()
                    break
            records = cohort["records"]
            if cohort["status"] == "CENSORED_FIRST_ROTATION":
                continue
            if len(records) != TRIALS * len(ARMS) or any(
                r["status"] != "COMPLETED" for r in records
            ):
                cohort["status"] = "CENSORED_NATIVE"
                save()
                continue
            try:
                for trial in range(TRIALS):
                    certify_trial(records, trial, digits)
            except (KeyError, ValueError, TypeError) as error:
                cohort.update(status="FAILED_NUMERIC_OR_TOPOLOGY", error=str(error))
                save()
                raise SystemExit(str(error)) from error
            cohort["status"] = "MATCHED_SOURCE_EXACT_CROSS_SOLVER"
            cohort["stats"] = {arm: statistics_for(records, arm) for arm in ARMS}
            for target, control_arm in (
                ("ordered_key", "inline_full"),
                ("inline_profile", "inline_full"),
                ("ordered_profile", "ordered_key"),
            ):
                paired_deltas = [
                    next(
                        r["wall_ns"] for r in records
                        if r["trial"] == trial and r["arm"] == target
                    ) - next(
                        r["wall_ns"] for r in records
                        if r["trial"] == trial and r["arm"] == control_arm
                    )
                    for trial in range(TRIALS)
                ]
                cohort["stats"][target][
                    "p50_paired_complete_wall_delta_ms"
                ] = statistics.median(paired_deltas) / 1e6
            report["qualified_cohorts"] += 1
            print(
                f"MQT_ORDERED_MATCH case={case} bits={bits} "
                f"gpmc_ms={cohort['stats']['gpmc']['p50_wall_ms']:.3f} "
                f"inline_ms={cohort['stats']['inline_full']['p50_wall_ms']:.3f} "
                f"ordered_ms={cohort['stats']['ordered_key']['p50_wall_ms']:.3f} "
                f"hits={cohort['stats']['ordered_key']['p50_ordered_key_hits']}",
                flush=True,
            )
            save()
    report["status"] = (
        "COMPLETE_14_OF_14_MATCHED_SOURCE_EXACT"
        if report["qualified_cohorts"] == EXPECTED_COHORTS
        else "PARTIAL_OR_CENSORED_INDEPENDENT_MQT_NO_ROUTER_CLAIM"
    )
    save()
    print(
        f"MQT_ORDERED_FINAL qualified={report['qualified_cohorts']} "
        f"expected={EXPECTED_COHORTS} status={report['status']}",
        flush=True,
    )
    if report["qualified_cohorts"] != EXPECTED_COHORTS:
        raise SystemExit("INCOMPLETE_ORIGINAL_MQT_ORDERED_KEY_HOLDOUT")


if __name__ == "__main__":
    main()
