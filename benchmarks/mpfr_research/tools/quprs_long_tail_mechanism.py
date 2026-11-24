#!/usr/bin/env python3
"""Source-locked GPMC-completes/Rust-censored original Weighted CNF holdout.

Seven order-rotated independent-process GPMC/Rust controls at 100/333-bit
and precommitted single-trial native GPMC option ablations. No CNF rewrite,
fake completion times, SAT-only shortcuts, or production default changes.
"""
from __future__ import annotations

import argparse
from hashlib import sha256
import json
from pathlib import Path
import statistics
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from quprs_long_tail_paired import (
    ARMS, BITS, GPMC_SHA, QU_PRS_SHA, LONG_TAIL_NODE_BUDGET, SCREEN_S,
    AS_BYTES, classify_native_failure, command, git_sha, native, sha,
    source_index, complex_relative_witness, EXPORTED_CASES, persist,
)

CASES = ("feynman_gf2_6_mult", "mqt_qnn_8", "mqt_wstate_16", "mqt_qwalk_5")
INDEXED_ARM = "indexed_fused"

def indexed_command(args, name, cnf, digits):
    cmd = command(args, name, cnf, digits, "inline_full")
    position = cmd.index("--hashbrown-borrowed")
    cmd[position:position] = ["--factor-variable-elimination", "--factor-ve-indexed-fused"]
    return cmd

TRIALS = 7
# Validated against pinned GPMC BoolOptions in original Config.cc. A
# toggle changes multiple downstream states: exploratory, NOT causality.
GPMC_ABLATIONS = {
    "no_preprocessing": ("-no-pp_ee", "-no-pp_ve", "-no-pp_cs"),
    "no_tree_decomposition": ("-no-td",),
    "no_backjump": ("-no-bj",),
}
MAX_WALL_SECONDS = 30
SOURCE_REPORT = "LONGTAIL_SCREEN_COMPLETE"
ORACLE_LIMIT = "cross-solver numeric agreement is not independent Full WMC proof"


def percentile(values: list[int], quantile: float) -> float:
    if not values:
        raise ValueError("MISSING_COMPLETED_SAMPLES")
    sorted_values = sorted(values)
    rank = (len(sorted_values) - 1) * quantile
    low, high = int(rank), min(int(rank) + 1, len(sorted_values) - 1)
    frac = rank - low
    return sorted_values[low] * (1 - frac) + sorted_values[high] * frac


def gpmc_ablation_cmd(args: argparse.Namespace, cnf: Path, digits: int, name: str) -> list[str]:
    if name not in GPMC_ABLATIONS or digits not in BITS:
        raise ValueError("UNLISTED_GPMC_TOGGLE")
    return [
        str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}",
        *GPMC_ABLATIONS[name], str(cnf.resolve()),
    ]


def verify_source(args: argparse.Namespace, screen: dict) -> tuple[dict, Path]:
    if git_sha(args.quprs) != QU_PRS_SHA or git_sha(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("ORIGINAL_SOURCE_REVISION_DRIFT")
    if (screen.get("status") != SOURCE_REPORT
            or screen.get("source_revisions") !=
            {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA}
            or screen.get("precisions") !=
            {"GPMC_decimal_30": 100, "GPMC_decimal_100": 333}
            or screen.get("limits", {}).get("AS_bytes_each_process") != AS_BYTES
            or screen.get("limits", {}).get("rust_node_budget_research") != 1_000_000_000):
        raise SystemExit("UNQUALIFIED_SCREEN_ARTIFACT")
    index = source_index((args.corpus / "source-index.tsv").read_text(encoding="utf-8"))
    records = screen.get("cases", [])
    if (len(records) != EXPORTED_CASES or
            {r.get("case") for r in records} != set(index)):
        raise SystemExit("INCOMPLETE_SOURCE_COHORT")
    chosen = None
    for row in records:
        case = row["case"]
        qasm = args.quprs / index[case]
        if (row.get("original_qasm") != index[case]
                or not qasm.is_file()
                or sha(qasm) != row.get("source_qasm_sha256")):
            raise SystemExit(f"SOURCE_QASM_DRIFT {case}")
        if row.get("source_status") == "SOURCE_EXACT":
            cnf = args.corpus / f"{case}-gpmc.cnf"
            copied = args.corpus / f"{case}.qasm"
            if (not cnf.is_file() or not copied.is_file()
                    or sha(cnf) != row.get("cnf_sha256")
                    or copied.read_bytes() != qasm.read_bytes()
                    or set(row.get("screen", {})) != set(ARMS)):
                raise SystemExit(f"SOURCE_WEIGHTED_CNF_DRIFT {case}")
        if case == args.case:
            chosen = row
    if chosen is None or chosen.get("source_status") != "SOURCE_EXACT":
        raise SystemExit("NO_ORIGINAL_WEIGHTED_CNF")
    prev = chosen["screen"]
    if (prev["gpmc"]["status"] != "COMPLETED"
            or prev["inline_full"]["status"] not in ("CENSORED_TIME", "CENSORED_OUTER_TIMEOUT")):
        raise SystemExit("NOT_A_PREDECLARED_GPMC_COMPLETE_RUST_TIMEOUT_COHORT")
    return chosen, args.corpus / f"{args.case}-gpmc.cnf"


def same_complex(reference: dict, trial: dict, digits: int) -> dict:
    if reference["sat"] != trial["sat"]:
        return {"pass": False, "cause": "SAT_MISMATCH"}
    return complex_relative_witness(reference, trial, digits)


def summarize(cohort: dict, digits: int) -> bool:
    baseline = cohort["native_trials"]
    gp = [r for r in baseline if r["arm"] == "gpmc"]
    rust = [r for r in baseline if r["arm"] == "inline_full"]
    if len(gp) != TRIALS or len(rust) != TRIALS:
        raise ValueError("INCOMPLETE_SEVEN_ROTATED_TRIALS")
    cohort["gpmc_completions"] = sum(r["status"] == "COMPLETED" for r in gp)
    cohort["rust_completions"] = sum(r["status"] == "COMPLETED" for r in rust)
    cohort["gpmc_statuses"] = [r["status"] for r in gp]
    cohort["rust_statuses"] = [r["status"] for r in rust]
    cohort["full_numeric_oracle_available"] = False
    cohort["limitations"] = ORACLE_LIMIT
    fail = False
    valid_gp = [r for r in gp if r["status"] == "COMPLETED"]
    if len(valid_gp) != TRIALS:
        cohort["status"] = "CENSORED_GPMC_BASELINE"
        return False
    ref = valid_gp[0]
    gp_parity = [same_complex(ref, record, digits) for record in valid_gp]
    cohort["gpmc_repeat_numeric_witness"] = gp_parity
    if not all(v["pass"] for v in gp_parity):
        cohort["status"] = "FAIL_GPMC_REPEAT_NUMERIC"
        return True
    gp_times = [r["wall_ns"] for r in gp]
    cohort["gpmc_p50_wall_ms"] = statistics.median(gp_times) / 1e6
    cohort["gpmc_p90_wall_ms"] = percentile(gp_times, 0.9) / 1e6
    cohort["gpmc_p50_rss_kib"] = statistics.median(
        r["peak_rss_kib"] for r in gp if r.get("peak_rss_kib") is not None
    ) if any(r.get("peak_rss_kib") is not None for r in gp) else None
    for record in rust:
        if record["status"] == "COMPLETED":
            witness = same_complex(ref, record, digits)
            record["source_locked_complex_numeric_witness"] = witness
            if not witness["pass"]:
                fail = True
    for name, record in cohort["gpmc_single_trial_ablations"].items():
        if record["status"] == "COMPLETED":
            witness = same_complex(ref, record, digits)
            record["numeric_witness_to_gpmc_default"] = witness
            if not witness["pass"]:
                fail = True
    if fail:
        cohort["status"] = "FAIL_SIGNED_COMPLEX_NUMERIC"
        return True
    if cohort["rust_completions"] == TRIALS:
        rtimes = [r["wall_ns"] for r in rust]
        cohort["rust_p50_wall_ms"] = statistics.median(rtimes) / 1e6
        cohort["rust_p90_wall_ms"] = percentile(rtimes, 0.9) / 1e6
        cohort["status"] = "EXPLORATORY_COMPLETED_AND_NUMERIC_MATCHED"
    elif all(r["status"] == "CENSORED_TIME" for r in rust):
        cohort["status"] = "RUST_ALL_SEVEN_TIMEOUT_GPMC_SEVEN_COMPLETIONS"
        # This is a mathematically valid LOWER BOUND, not a speed ratio or
        # an estimated Rust completion time.
        cohort["rust_over_gpmc_p50_strict_lower_bound"] = (
            MAX_WALL_SECONDS * 1e9 / statistics.median(gp_times)
        )
        cohort["rust_p50_wall_ms"] = None
        cohort["rust_p90_wall_ms"] = None
    else:
        cohort["status"] = "MIXED_CENSORED_RUST"
        cohort["rust_p50_wall_ms"] = None
        cohort["rust_p90_wall_ms"] = None
    return False


def self_test() -> None:
    assert TRIALS == 7 and set(BITS.items()) == {(30, 100), (100, 333)}
    assert LONG_TAIL_NODE_BUDGET == "--node-budget=1000000000"
    assert set(CASES) == {"feynman_gf2_6_mult", "mqt_qnn_8", "mqt_wstate_16", "mqt_qwalk_5"}
    fake = argparse.Namespace(gpmc=Path("/pinned/gpmc"), mpfr=Path("/pinned/mpfr"))
    cnf = Path("/pinned/source-gpmc.cnf")
    for digits in BITS:
        for arm in ARMS:
            cmd = command(fake, CASES[0], cnf, digits, arm)
            assert (LONG_TAIL_NODE_BUDGET in cmd) == (arm == "inline_full")
        indexed = indexed_command(fake, CASES[0], cnf, digits)
        assert indexed.index("--factor-ve-indexed-fused") + 1 == indexed.index("--hashbrown-borrowed")
        for name in GPMC_ABLATIONS:
            cmd = gpmc_ablation_cmd(fake, cnf, digits, name)
            assert cmd[1:3] == ["-mode=1", f"-prec={digits}"]
            assert not any(x.startswith("--node-budget=") for x in cmd)
    assert percentile([1, 2, 3, 4, 5, 6, 7], 0.9) == 6.4
    assert classify_native_failure(2, "RESEARCH_MPFR_FAIL_CLOSED: node limit") == "CENSORED_NODE_BUDGET"
    print("LONGTAIL_GPMC_COMPLETE_SEVEN_TRIAL_PREFLIGHT_PASS", flush=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--indexed-fused", action="store_true")
    ap.add_argument("--case", choices=CASES)
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, type=Path)
    args = ap.parse_args()
    self_test()
    if args.self_test:
        return
    for name in ("case", "quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            ap.error("missing --" + name.replace("_", "-"))
    src_report = args.corpus / "long-tail-source-locked-evidence.json"
    if not src_report.is_file():
        raise SystemExit("MISSING_ORIGINAL_SOURCE_SCREEN")
    evidence = json.loads(src_report.read_text(encoding="utf-8"))
    prior, cnf = verify_source(args, evidence)
    result = {
        "study": "Same-original-Weighted-CNF seven-trial GPMC-complete Rust-timeout contrast",
        "status": "IN_PROGRESS", "case": args.case,
        "source_qasm_sha256": prior["source_qasm_sha256"],
        "source_cnf_sha256": prior["cnf_sha256"],
        "source_lock": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "research_bound": {"seconds_per_solver": MAX_WALL_SECONDS, "AS_bytes": AS_BYTES,
                           "rust_node_budget": 1_000_000_000},
        "design": "seven fresh rotated native processes per arm and precision; one exploratory native GPMC feature-toggle sample each",
        "indexed_fused_enabled": args.indexed_fused,
        "not_a_production_go": True,
        "full_oracle_caveat": ORACLE_LIMIT, "cohorts": [],
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    persist(args, result)
    failures = False
    for digits, bits in BITS.items():
        cohort = {"digits": digits, "bits": bits, "status": "IN_PROGRESS",
                  "native_trials": [], "gpmc_single_trial_ablations": {}}
        result["cohorts"].append(cohort)
        persist(args, result)
        arms = (*ARMS, INDEXED_ARM) if args.indexed_fused else ARMS
        for trial in range(TRIALS):
            shift = trial % len(arms)
            rotated = arms[shift:] + arms[:shift]
            for arm in rotated:
                cmd = (indexed_command(args, args.case, cnf, digits) if arm == INDEXED_ARM
                       else command(args, args.case, cnf, digits, arm))
                record = native(cmd, arm, MAX_WALL_SECONDS, digits)
                record["trial"] = trial
                cohort["native_trials"].append(record)
                persist(args, result)
                print(f"LONGTAIL_7TRIAL case={args.case} bits={bits} "
                      f"trial={trial} arm={arm} status={record['status']}", flush=True)
        for name in GPMC_ABLATIONS:
            record = native(gpmc_ablation_cmd(args, cnf, digits, name),
                            "gpmc", MAX_WALL_SECONDS, digits)
            record["toggle"] = name
            cohort["gpmc_single_trial_ablations"][name] = record
            persist(args, result)
            print(f"LONGTAIL_GPMC_TOGGLE case={args.case} bits={bits} "
                  f"arm={name} status={record['status']}", flush=True)
        failures = summarize(cohort, digits) or failures
        if args.indexed_fused:
            candidates = [r for r in cohort["native_trials"] if r["arm"] == INDEXED_ARM]
            assert len(candidates) == TRIALS
            refs = [r for r in cohort["native_trials"] if r["arm"] == "gpmc" and r["status"] == "COMPLETED"]
            completed = [r for r in candidates if r["status"] == "COMPLETED"]
            for row in completed:
                if refs:
                    witness = same_complex(refs[0], row, digits)
                    row["source_locked_complex_numeric_witness"] = witness
                    failures = not witness["pass"] or failures
            cohort["indexed_fused_completions"] = len(completed)
            cohort["indexed_fused_fastpath_completions"] = sum(r.get("factor_ve_completed") is True for r in completed)
            cohort["indexed_fused_p50_wall_ms"] = statistics.median(r["wall_ns"] for r in completed) / 1e6 if len(completed) == TRIALS else None
            cohort["indexed_fused_p90_wall_ms"] = percentile([r["wall_ns"] for r in completed], .9) / 1e6 if len(completed) == TRIALS else None
            rss = [r["peak_rss_kib"] for r in completed if r.get("peak_rss_kib") is not None]
            cohort["indexed_fused_p50_rss_kib"] = statistics.median(rss) if len(rss) == TRIALS else None
            cohort["indexed_fused_promotion"] = "NOT_AUTHORIZED_RESEARCH_ONLY"
        persist(args, result)
        print(f"LONGTAIL_7TRIAL_RESULT case={args.case} bits={bits} "
              f"status={cohort['status']} "
              f"gpmc_p50_ms={cohort.get('gpmc_p50_wall_ms', 'CENSORED')} "
              f"rust_over_gpmc_lower_bound="
              f"{cohort.get('rust_over_gpmc_p50_strict_lower_bound', 'NOT_DEFINED')}",
              flush=True)
    result["status"] = ("NUMERIC_DISAGREEMENT_FAIL" if failures else
                        "COMPLETE_EXPLORATORY_TIMEOUT_BOUND_EVIDENCE")
    persist(args, result)
    print(f"LONGTAIL_7TRIAL_FINAL case={args.case} status={result['status']}", flush=True)
    if failures:
        raise SystemExit("FAIL_SOURCE_LOCKED_SAT_OR_COMPLEX_WMC")

if __name__ == "__main__":
    main()
