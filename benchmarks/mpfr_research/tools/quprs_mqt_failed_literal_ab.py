#!/usr/bin/env python3
"""Independent MQT Bench holdout for exact entailed-root failed-literal units.

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

from quprs_cnf_articulation_ab import (  # noqa: E402
    BITS,
    GPMC_SHA,
    QU_PRS_SHA,
    build_command,
    percentile,
    sample,
    sha,
    source_head,
)
from quprs_mqtbench_original_cnf_ab import (  # noqa: E402
    MAX_SOURCE_BYTES,
    ORIGINAL_MQT,
    complex_relative_witness,
)
from quprs_encoder_validate import parse_cnf  # noqa: E402

ARMS = ("gpmc", "inline_full", "failed_literal")
TRIALS = 7
EXPECTED_COHORTS = 2 * len(ORIGINAL_MQT)
ROTATION_WALL_NS = 10_000_000_000


def preflight() -> None:
    if BITS != {30: 100, 100: 333}:
        raise SystemExit("DRIFT_IN_GPMC_TO_MPFR_BITS")
    if len(ORIGINAL_MQT) != 7:
        raise SystemExit("DRIFT_IN_PINNED_MQT_HOLDOUT")
    if ARMS != ("gpmc", "inline_full", "failed_literal"):
        raise SystemExit("CHANGED_HOLDOUT_ARMS")
    print("MQT_FAILED_LITERAL_PREFLIGHT_OK cases=7 precisions=2 arms=3", flush=True)


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
        "implied_units": (statistics.median(r["implied_units"] for r in selected)
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
        "study": "Original MQT Bench QASM to QuPRS Weighted CNF, bounded entailed failed-literal root units",
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
            print(f"MQT_FAILED_LITERAL_CENSORED case={case} source={statuses[case]}", flush=True)
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
                        if arm == "failed_literal":
                            command.insert(
                                command.index("--weighted-equivalence") + 1,
                                "--root-failed-literal",
                            )
                    item = sample(command, arm, digits)
                    item["trial"] = trial
                    cohort["records"].append(item)
                    save()
                    print(f"MQT_FAILED_LITERAL case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={item['status']} "
                          f"nodes={item.get('nodes', 'NA')} "
                          f"implied={item.get('implied_units', 'NA')}", flush=True)
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
                baseline, rust, root = (selected[arm] for arm in ARMS)
                if not (baseline["sat"] == rust["sat"] == root["sat"]):
                    cohort["status"] = "FAILED_BOOLEAN_SAT"
                    save()
                    raise SystemExit("FAILED_BOOLEAN_SAT")
                for arm, result in (("inline_full", rust), ("failed_literal", root)):
                    witness = complex_relative_witness(baseline, result, digits)
                    if not witness["pass"]:
                        cohort["status"] = "FAILED_SIGNED_COMPLEX_WMC"
                        cohort["failing_witness"] = {
                            "trial": trial, "arm": arm, "witness": witness,
                        }
                        save()
                        raise SystemExit("FAILED_SIGNED_COMPLEX_WMC")
                if root["failed_candidates"] > 32 or root["failed_probes"] > 64:
                    cohort["status"] = "FAILED_BOUNDED_ROOT_INFERENCE"
                    save()
                    raise SystemExit("FAILED_BOUNDED_ROOT_INFERENCE")
                if root["separator_scans"] != 0 or root["separator_chosen"] != 0:
                    cohort["status"] = "FAILED_PIVOT_CONTROL_CONTAMINATION"
                    save()
                    raise SystemExit("FAILED_PIVOT_CONTROL_CONTAMINATION")
                if rust["separator_scans"] != 0 or rust["separator_chosen"] != 0:
                    cohort["status"] = "FAILED_INLINE_CONTROL_CONTAMINATION"
                    save()
                    raise SystemExit("FAILED_INLINE_CONTROL_CONTAMINATION")
                if root["implied_units"] == 0 and not root["failed_contradicted"] and (
                    root["nodes"], root["cache_hits"], root["decompositions"]
                ) != (rust["nodes"], rust["cache_hits"], rust["decompositions"]):
                    cohort["status"] = "FAILED_NOOP_SEARCH_TOPOLOGY"
                    save()
                    raise SystemExit("FAILED_NOOP_SEARCH_TOPOLOGY")
            cohort["status"] = "MATCHED_SIGNED_COMPLEX_WMC"
            cohort["stats"] = {arm: group_stats(records, arm) for arm in ARMS}
            report["qualified_cohorts"] += 1
            save()
            print(f"MQT_FAILED_LITERAL_MATCH case={case} bits={bits} "
                  f"gpmc_ms={cohort['stats']['gpmc']['p50_wall_ms']:.3f} "
                  f"inline_ms={cohort['stats']['inline_full']['p50_wall_ms']:.3f} "
                  f"failed_literal_ms={cohort['stats']['failed_literal']['p50_wall_ms']:.3f} "
                  f"nodes_inline={cohort['stats']['inline_full']['p50_nodes']} "
                  f"nodes_failed={cohort['stats']['failed_literal']['p50_nodes']} "
                  f"implied_units={cohort['stats']['failed_literal']['implied_units']}", flush=True)

    report["status"] = (
        "ALL_ORIGINAL_MQT_HOLDOUT_COHORTS_QUALIFIED"
        if report["qualified_cohorts"] == EXPECTED_COHORTS
        else "PARTIAL_OR_CENSORED_MQT_HOLDOUT"
    )
    save()
    print(f"MQT_FAILED_LITERAL_FINISHED qualified={report['qualified_cohorts']} "
          f"expected={EXPECTED_COHORTS} status={report['status']}", flush=True)
    if report["qualified_cohorts"] != EXPECTED_COHORTS:
        raise SystemExit("INCOMPLETE_MQT_FAILED_LITERAL_HOLDOUT_NO_GLOBAL_SPEED_CLAIM")


if __name__ == "__main__":
    main()
