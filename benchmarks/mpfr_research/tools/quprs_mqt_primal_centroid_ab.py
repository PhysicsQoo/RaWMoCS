#!/usr/bin/env python3
"""Pinned original MQT QASM→QuPRS Weighted CNF: root/full bag-centroid
pivot against unchanged Rust Inline and original C++ GPMC.
This Rust score is a min-degree bag-tree approximation, NOT FlowCutter.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import statistics

from quprs_mqt_failed_literal_ab import (
    BITS, GPMC_SHA, QU_PRS_SHA, ORIGINAL_MQT, MAX_SOURCE_BYTES,
    build_command, complex_relative_witness, parse_cnf, parse_manifest,
    percentile, sample, sha, source_head,
)

ARMS = ("gpmc", "inline_full", "centroid_root", "centroid_full")
TRIALS = 7
EXPECTED_COHORTS = len(ORIGINAL_MQT) * len(BITS)
EARLY_CENSOR_NS = 10_000_000_000


def preflight() -> None:
    assert BITS == {30: 100, 100: 333}
    assert len(ORIGINAL_MQT) == 7
    assert len(ARMS) == 4 and ARMS[0] == "gpmc"
    print("MQT_PRIMAL_CENTROID_MATRIX_OK sources=7 precisions=2 arms=4", flush=True)


def invoke(args: argparse.Namespace, case: str, digits: int, cnf: Path, arm: str) -> dict:
    if arm == "gpmc":
        return sample(build_command(args, "gpmc", digits, cnf, case), "gpmc", digits)
    cmd = build_command(args, "inline_full", digits, cnf, case)
    cmd.insert(1, "--node-budget=1000000")
    if arm.startswith("centroid_"):
        at = cmd.index("--hashbrown-borrowed")
        flags = ["--trail-primal-bag-centroid"]
        if arm == "centroid_root":
            flags.append("--trail-primal-centroid-root-only")
        cmd[at:at] = flags
    out = sample(cmd, "inline_full", digits)
    out["arm"] = arm
    return out


def summary(records: list[dict], arm: str) -> dict:
    selected = [x for x in records if x["arm"] == arm]
    timings = [x["wall_ns"] for x in selected]
    result = {
        "p50_wall_ms": statistics.median(timings) / 1e6,
        "p90_wall_ms": percentile(timings, 0.9) / 1e6,
        "p50_max_rss_kib": statistics.median(x["peak_rss_kib"] for x in selected),
    }
    if arm != "gpmc":
        for key in ("nodes", "cache_hits", "decompositions",
                    "centroid_build_ns", "centroid_scans",
                    "centroid_overrides", "centroid_width", "centroid_edges",
                    "centroid_fill"):
            result["p50_" + key] = statistics.median(x[key] for x in selected)
        result["centroid_eligible"] = all(x["centroid_eligible"] for x in selected)
        result["centroid_capped"] = any(x["centroid_capped"] for x in selected)
    return result


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--self-test", action="store_true")
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        p.add_argument("--" + name, type=Path)
    p.add_argument("--trials", type=int, default=TRIALS)
    args = p.parse_args()
    preflight()
    if args.self_test:
        return
    for name in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            p.error(f"missing {name}")
    if args.trials != TRIALS:
        raise SystemExit("SEVEN_FRESH_ROTATIONS_REQUIRED")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("SOURCE_SHA_MISMATCH")
    manifest = parse_manifest(args.corpus / "source-status.tsv")
    report = {
        "study": "Source-locked MQT original CNF, Rust primal bag-centroid score and GPMC",
        "method": "bounded min-degree triangular bag forest centroid; NOT FlowCutter",
        "quprs_sha": QU_PRS_SHA, "gpmc_sha": GPMC_SHA,
        "original_qasm_not_mqt_published_cnf": True,
        "arms": list(ARMS), "precisions": BITS, "trials": TRIALS,
        "native": "30 seconds and 2 GiB virtual memory per independent process",
        "numeric": "SAT separate; signed Complex WMC both components; large case is cross-solver agreement",
        "cases": [], "qualified_cohorts": 0,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    def persist() -> None:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")

    for case, filename in ORIGINAL_MQT.items():
        qasm = args.quprs / "benchmarks" / "MQTBench" / filename
        if not qasm.is_file():
            raise SystemExit(f"MISSING_ORIGINAL_MQT_QASM_{case}")
        item = {
            "case": case, "source_qasm_sha256": sha(qasm),
            "qasm_relative_path": str(qasm.relative_to(args.quprs)),
            "export_status": manifest[case], "precisions": [],
        }
        report["cases"].append(item)
        if manifest[case] != "EXPORTED":
            item["status"] = "CENSORED_EXPORT"
            persist()
            continue
        files = [
            args.corpus / f"{case}.qasm",
            args.corpus / f"{case}-gpmc.cnf",
            args.corpus / f"{case}-ganak.cnf",
            args.corpus / f"{case}-gpmc-phases.tsv",
            args.corpus / f"{case}-ganak-phases.tsv",
        ]
        copied, cnf, ganak, phases, second_phases = files
        if not all(f.is_file() and f.stat().st_size <= MAX_SOURCE_BYTES for f in files):
            item["status"] = "CENSORED_MISSING_OR_TOO_LARGE"
            persist()
            continue
        if copied.read_bytes() != qasm.read_bytes():
            raise SystemExit(f"MODIFIED_SOURCE_QASM_{case}")
        try:
            a, b = parse_cnf(cnf, "gpmc"), parse_cnf(ganak, "ganak")
        except ValueError as error:
            item.update(status="CENSORED_CNF_DIALECT", error=str(error))
            persist()
            continue
        if ((a["variables"], a["clauses"], a["weights"])
                != (b["variables"], b["clauses"], b["weights"])
                or phases.read_bytes() != second_phases.read_bytes()):
            raise SystemExit(f"DUAL_CNF_OR_PHASE_DRIFT_{case}")
        item.update(
            vars=a["variables"], clauses=a["clauses_count"],
            cnf_sha256=sha(cnf), ganak_sha256=sha(ganak),
            phase_sha256=sha(phases),
        )
        for digits, bits in BITS.items():
            cohort = {"digits": digits, "bits": bits, "status": "INCOMPLETE", "records": []}
            item["precisions"].append(cohort)
            persist()
            for trial in range(TRIALS):
                rotated = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in rotated:
                    row = invoke(args, case, digits, cnf, arm)
                    row["trial"] = trial
                    cohort["records"].append(row)
                    persist()
                    print(f"MQT_CENTROID case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={row['status']} nodes={row.get('nodes','NA')} "
                          f"overrides={row.get('centroid_overrides','NA')}", flush=True)
                if trial == 0 and any(
                    r["status"] != "COMPLETED" or
                    r.get("wall_ns", EARLY_CENSOR_NS + 1) > EARLY_CENSOR_NS
                    for r in cohort["records"]
                ):
                    cohort["status"] = "CENSORED_FIRST_ROTATION"
                    break
            records = cohort["records"]
            if cohort["status"] == "CENSORED_FIRST_ROTATION" or (
                len(records) != TRIALS * len(ARMS) or
                any(r["status"] != "COMPLETED" for r in records)
            ):
                if cohort["status"] != "CENSORED_FIRST_ROTATION":
                    cohort["status"] = "CENSORED_NATIVE_OR_MISSING_TRIALS"
                persist()
                continue

            for trial in range(TRIALS):
                group = {r["arm"]: r for r in records if r["trial"] == trial}
                if set(group) != set(ARMS):
                    raise SystemExit("UNBALANCED_ARM_ROTATION")
                gpmc = group["gpmc"]
                baseline = group["inline_full"]
                if baseline["centroid_enabled"] or baseline["centroid_overrides"] != 0:
                    raise SystemExit("CONTAMINATED_INLINE_CONTROL")
                for arm in ARMS[1:]:
                    out = group[arm]
                    if out["sat"] != gpmc["sat"]:
                        cohort["status"] = "FAILED_BOOLEAN_SAT"
                        persist()
                        raise SystemExit("FAILED_BOOLEAN_SAT")
                    witness = complex_relative_witness(gpmc, out, digits)
                    if not witness["pass"]:
                        cohort.update(status="FAILED_SIGNED_COMPLEX_WMC",
                                      failing_arm=arm, trial=trial, witness=witness)
                        persist()
                        raise SystemExit("FAILED_SIGNED_COMPLEX_WMC")
                    if arm == "inline_full":
                        continue
                    if not out["centroid_enabled"] or (
                        out["centroid_root_only"] != (arm == "centroid_root")
                    ):
                        raise SystemExit("CENTROID_MODE_DIAGNOSTIC_MISMATCH")
                    if arm == "centroid_root" and out["centroid_scans"] > 1:
                        raise SystemExit("UNBOUNDED_ROOT_ONLY_SCORE")
                    if out["centroid_overrides"] == 0 or not out["centroid_eligible"]:
                        # A truly no-op score must preserve *search
                        # topology*, not just the final weighted value.
                        if (out["nodes"], out["cache_hits"], out["decompositions"]) != (
                            baseline["nodes"], baseline["cache_hits"], baseline["decompositions"]
                        ):
                            cohort["status"] = "FAILED_NOOP_SEARCH_TOPOLOGY"
                            persist()
                            raise SystemExit("FAILED_NOOP_SEARCH_TOPOLOGY")
            cohort["status"] = "MATCHED_SIGNED_COMPLEX_WMC_AND_SAT"
            cohort["stats"] = {arm: summary(records, arm) for arm in ARMS}
            report["qualified_cohorts"] += 1
            persist()
            print(f"MQT_CENTROID_MATCH case={case} bits={bits} " +
                  " ".join(f"{arm}_ms={cohort['stats'][arm]['p50_wall_ms']:.3f}"
                           for arm in ARMS) +
                  f" centroid_width={cohort['stats']['centroid_full']['p50_centroid_width']}" +
                  f" full_nodes={cohort['stats']['centroid_full']['p50_nodes']}" +
                  f" root_nodes={cohort['stats']['centroid_root']['p50_nodes']}" +
                  f" inline_nodes={cohort['stats']['inline_full']['p50_nodes']}" +
                  f" full_overrides={cohort['stats']['centroid_full']['p50_centroid_overrides']}",
                  flush=True)

    report["status"] = (
        "ALL_14_ORIGINAL_MQT_COHORTS_QUALIFIED"
        if report["qualified_cohorts"] == EXPECTED_COHORTS
        else "INCOMPLETE_MQT_COHORTS_RETAINED"
    )
    persist()
    print(f"MQT_CENTROID_DONE qualified={report['qualified_cohorts']} "
          f"expected={EXPECTED_COHORTS} status={report['status']}", flush=True)
    if report["qualified_cohorts"] != EXPECTED_COHORTS:
        raise SystemExit("MISSING_MQT_CENTROID_COHORT_NO_SPEED_CLAIM")


if __name__ == "__main__":
    main()
