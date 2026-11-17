#!/usr/bin/env python3
"""Original MQT QASM -> pinned QuPRS Weighted CNF -> GPMC mechanism ablation.

All GPMC switches compare the unchanged pinned C++ binary to ITSELF
at the same decimal precision on byte-identical weighted CNF. One
original Rust Inline control is included only to contextualize gaps.
A failed source, censored toggle or inconsistent signed complex result
cannot be silently excluded from the 14 predeclared cohorts.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import statistics
import subprocess

from quprs_mqt_failed_literal_ab import (
    BITS, GPMC_SHA, QU_PRS_SHA, ORIGINAL_MQT, MAX_SOURCE_BYTES,
    build_command, complex_relative_witness, parse_cnf, parse_manifest,
    percentile, sample, sha, source_head,
)

# BoolOptions from the unchanged pinned GPMC core/Config.cc.
GPMC_ARMS = {
    "gpmc_default": (),
    "no_equivalence": ("-no-pp_ee",),
    "no_variable_elimination": ("-no-pp_ve",),
    "no_clause_strengthening": ("-no-pp_cs",),
    "no_tree_decomposition": ("-no-td",),
    "no_backjump": ("-no-bj",),
    "no_preprocessing": ("-no-pp_ee", "-no-pp_ve", "-no-pp_cs"),
    "no_preprocess_no_td": ("-no-pp_ee", "-no-pp_ve", "-no-pp_cs", "-no-td"),
}
ARMS = tuple(GPMC_ARMS) + ("rust_inline",)
TRIALS = 7
EXPECTED_COHORTS = 2 * len(ORIGINAL_MQT)
ROTATION_MAX_NS = 10_000_000_000


def validate_matrix() -> None:
    assert BITS == {30: 100, 100: 333}
    assert len(ORIGINAL_MQT) == 7 and len(GPMC_ARMS) == 8
    assert ARMS[0] == "gpmc_default" and ARMS[-1] == "rust_inline"
    valid = {"-no-pp_ee", "-no-pp_ve", "-no-pp_cs", "-no-td", "-no-bj"}
    assert all(set(v).issubset(valid) and len(set(v)) == len(v) for v in GPMC_ARMS.values())
    assert GPMC_ARMS["gpmc_default"] == ()
    print("MQT_GPMC_MATRIX_VERIFIED mechanisms=8 rust_control=1 sources=7", flush=True)


def invoke(args: argparse.Namespace, case: str, digits: int, cnf: Path, arm: str) -> dict:
    if arm in GPMC_ARMS:
        command = [
            str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}",
            *GPMC_ARMS[arm], str(cnf.resolve()),
        ]
        record = sample(command, "gpmc", digits)
    else:
        command = build_command(args, "inline_full", digits, cnf, case)
        # The original MQT benchmark admits 1,000,000 research nodes,
        # not an enlarged limit granted only to the treatment arm.
        command.insert(1, "--node-budget=1000000")
        record = sample(command, "inline_full", digits)
    record["arm"] = arm
    return record


def stats(records: list[dict], arm: str) -> dict:
    subset = [r for r in records if r["arm"] == arm]
    wall = [r["wall_ns"] for r in subset]
    output = {
        "p50_wall_ms": statistics.median(wall) / 1e6,
        "p90_wall_ms": percentile(wall, 0.9) / 1e6,
        "p50_peak_rss_kib": statistics.median(r["peak_rss_kib"] for r in subset),
    }
    if arm == "rust_inline":
        for name in ("nodes", "cache_hits", "decompositions"):
            output[f"p50_{name}"] = statistics.median(r[name] for r in subset)
    return output


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--trials", type=int, default=7)
    args = parser.parse_args()
    validate_matrix()
    if args.self_test:
        return
    for name in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            parser.error(f"missing --{name.replace('_','-')}")
    if args.trials != TRIALS:
        raise SystemExit("SEVEN_ROTATED_TRIALS_REQUIRED")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("UNPINNED_MQT_SOURCE_OR_GPMC")
    manifest = parse_manifest(args.corpus / "source-status.tsv")
    report = {
        "study": "Source-identical original MQT QuPRS weighted CNF, unchanged GPMC feature mechanisms",
        "purpose": "causal configuration toggle, NOT proof of RaWMoCS algorithm superiority",
        "source_sha_quprs": QU_PRS_SHA, "source_sha_gpmc": GPMC_SHA,
        "original_qasm_is_not_mqt_published_cnf": True,
        "gpmc_arms": {k: list(v) for k, v in GPMC_ARMS.items()},
        "arms": list(ARMS), "trials": TRIALS,
        "precision": "GPMC feature arms 30/100 decimal; separate Rust Inline 100/333 MPFR bits",
        "process_limits": "30 seconds / 2 GiB virtual address space per fresh child; GNU time RSS observed",
        "cross_solver_oracle": "GPMC↔Rust signed Complex WMC numeric agreement, NOT independent exact full WMC",
        "caveats": [
            "GPMC disabling --pp_ee/--pp_ve/--pp_cs is not equivalent to disabling ALL preprocessing",
            "Removing --td also changes interactions with remaining mechanisms",
            "No backend code, source QASM, phase file, CNF or weights are modified",
        ],
        "cases": [], "qualified_cohorts": 0,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)

    def persist() -> None:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")

    for case, filename in ORIGINAL_MQT.items():
        qasm = args.quprs / "benchmarks" / "MQTBench" / filename
        if not qasm.is_file():
            raise SystemExit(f"MISSING_PINNED_MQT_QASM_{case}")
        item = {
            "case": case, "qasm_relpath": str(qasm.relative_to(args.quprs)),
            "qasm_sha256": sha(qasm), "export_status": manifest[case],
            "precisions": [],
        }
        report["cases"].append(item)
        if manifest[case] != "EXPORTED":
            item["status"] = "CENSORED_SOURCE_EXPORT"
            persist()
            print(f"MQT_GPMC_SOURCE_CENSORED case={case} status={manifest[case]}", flush=True)
            continue
        files = [
            args.corpus / f"{case}.qasm",
            args.corpus / f"{case}-gpmc.cnf",
            args.corpus / f"{case}-ganak.cnf",
            args.corpus / f"{case}-gpmc-phases.tsv",
            args.corpus / f"{case}-ganak-phases.tsv",
        ]
        copied, cnf, ganak, phases, alternate_phases = files
        if not all(f.is_file() and f.stat().st_size <= MAX_SOURCE_BYTES for f in files):
            item["status"] = "CENSORED_MISSING_OR_OVERSIZE_CNF"
            persist()
            continue
        if qasm.read_bytes() != copied.read_bytes():
            raise SystemExit(f"QASM_SOURCE_BYTES_DRIFT_{case}")
        try:
            a, b = parse_cnf(cnf, "gpmc"), parse_cnf(ganak, "ganak")
        except ValueError as error:
            item.update(status="CENSORED_UNSUPPORTED_EMITTED_CNF", error=str(error))
            persist()
            continue
        if ((a["variables"], a["clauses"], a["weights"])
                != (b["variables"], b["clauses"], b["weights"])
                or phases.read_bytes() != alternate_phases.read_bytes()):
            raise SystemExit(f"DUAL_DIALECT_OR_PHASE_DRIFT_{case}")
        item.update(
            variables=a["variables"], clauses=a["clauses_count"],
            cnf_sha256=sha(cnf), ganak_cnf_sha256=sha(ganak),
            phase_sha256=sha(phases),
        )
        for digits, bits in BITS.items():
            cohort = {"digits": digits, "bits": bits, "status": "INCOMPLETE", "records": []}
            item["precisions"].append(cohort)
            persist()
            for trial in range(TRIALS):
                rotated = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in rotated:
                    record = invoke(args, case, digits, cnf, arm)
                    record["trial"] = trial
                    cohort["records"].append(record)
                    persist()
                    print(f"MQT_GPMC_FEATURE case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={record['status']}", flush=True)
                if trial == 0:
                    if any(r["status"] != "COMPLETED" or
                           r.get("wall_ns", ROTATION_MAX_NS + 1) > ROTATION_MAX_NS
                           for r in cohort["records"]):
                        cohort["status"] = "CENSORED_FIRST_ROTATION"
                        break
            records = cohort["records"]
            if cohort["status"] == "CENSORED_FIRST_ROTATION" or (
                len(records) != TRIALS * len(ARMS) or
                any(r["status"] != "COMPLETED" for r in records)
            ):
                if cohort["status"] != "CENSORED_FIRST_ROTATION":
                    cohort["status"] = "CENSORED_INCOMPLETE_ROTATIONS"
                persist()
                continue
            for trial in range(TRIALS):
                selected = {r["arm"]: r for r in records if r["trial"] == trial}
                if set(selected) != set(ARMS):
                    raise SystemExit("UNBALANCED_COHORT_ROTATION")
                reference = selected["gpmc_default"]
                for arm, row in selected.items():
                    if reference["sat"] != row["sat"]:
                        cohort.update(status="FAILED_BOOLEAN_SAT", trial=trial, arm=arm)
                        persist()
                        raise SystemExit("FAILED_BOOLEAN_SAT")
                    evidence = complex_relative_witness(reference, row, digits)
                    if not evidence["pass"]:
                        cohort.update(
                            status="FAILED_SIGNED_COMPLEX_WMC", trial=trial,
                            arm=arm, numeric_failure=evidence,
                        )
                        persist()
                        raise SystemExit("FAILED_SIGNED_COMPLEX_WMC")
            cohort["status"] = "SAME_CNF_SIGNED_COMPLEX_WMC_AGREEMENT"
            cohort["stats"] = {arm: stats(records, arm) for arm in ARMS}
            cohort["paired_default_to_arm_ratio_p50"] = {
                arm: statistics.median(
                    next(r["wall_ns"] for r in records
                         if r["trial"] == t and r["arm"] == "gpmc_default") /
                    next(r["wall_ns"] for r in records
                         if r["trial"] == t and r["arm"] == arm)
                    for t in range(TRIALS)
                ) for arm in ARMS
            }
            report["qualified_cohorts"] += 1
            persist()
            print(f"MQT_GPMC_FEATURE_MATCH case={case} bits={bits} " +
                  " ".join(f"{arm}_ms={cohort['stats'][arm]['p50_wall_ms']:.3f}"
                           for arm in ARMS), flush=True)

    report["status"] = (
        "ALL_14_ORIGINAL_MQT_SOURCE_COHORTS_QUALIFIED"
        if report["qualified_cohorts"] == EXPECTED_COHORTS
        else "INCOMPLETE_OR_CENSORED_MQT_SOURCE_COHORTS"
    )
    persist()
    print(f"MQT_GPMC_FEATURE_DONE qualified={report['qualified_cohorts']} "
          f"expected={EXPECTED_COHORTS} status={report['status']}", flush=True)
    if report["qualified_cohorts"] != EXPECTED_COHORTS:
        raise SystemExit("NO_GLOBAL_CLAIM_MQT_GPMC_FEATURE_COHORT_CENSORED")


if __name__ == "__main__":
    main()
