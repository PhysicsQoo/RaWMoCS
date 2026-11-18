#!/usr/bin/env python3
"""Independent source-exact GPMC / Arena / Arena+active-bitmap native A/B.

A research-only incremental experiment, not an extension or replacement of
the previously accepted seven-arm Arena baseline. Always retain censored
trials, full process WallTime and observed RSS. Only source-pinned original
QuPRS-emitted weighted CNFs are permitted.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
import os
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent
CORE = HERE.parents[1] / "rust_core" / "tools"
sys.path.insert(0, str(CORE))
from quprs_cnf_key_arena_ab import command  # noqa: E402
from quprs_cnf_scoped_dsu_ab import (  # noqa: E402
    BITS, GPMC_SHA, QU_PRS_SHA, SOURCE, numerically_agree, percentile,
    sample, sha, source_head,
)
from quprs_mqtbench_original_cnf_ab import (  # noqa: E402
    ORIGINAL_MQT, MAX_SOURCE_BYTES, complex_relative_witness,
)
from quprs_encoder_validate import parse_cnf  # noqa: E402

ARMS = ("gpmc", "arena_control", "arena_active_bitmap")
TRIALS = 7
HARD = {"feynman_hwb6", "mqt_qnn_4"}


def checked(condition: bool, label: str) -> None:
    if not condition:
        raise ValueError(label)


def make_command(args: argparse.Namespace, arm: str, digits: int,
                 cnf: Path, case: str) -> list[str]:
    if arm == "gpmc":
        return command(args, "gpmc", digits, cnf, case)
    checked(arm in ARMS, "UNKNOWN_STAMP_ARM")
    cmd = command(args, "arena_reuse", digits, cnf, case)
    if args.family == "mqt":
        cmd.insert(1, "--node-budget=1000000")
    if arm == "arena_active_bitmap":
        checked(cmd.count("--trail-reuse-key-arena") == 1,
                "ACTIVE_BITMAP_REQUIRES_ARENA_CONTROL")
        cmd.insert(cmd.index("--hashbrown-borrowed"),
                   "--trail-active-clause-bitmap")
    return cmd


def check_trial(group: dict[str, dict], bits: int, digits: int,
                family: str) -> None:
    checked(set(group) == set(ARMS), "UNBALANCED_ACTIVE_BITMAP_ARMS")
    g, a, b = (group[arm] for arm in ARMS)
    checked(all(r["status"] == "COMPLETED" and r["exit_code"] == 0
                for r in group.values()), "ACTIVE_BITMAP_CENSORED_TRIAL")
    checked(g["sat"] == a["sat"] == b["sat"],
            "ACTIVE_BITMAP_BOOLEAN_SAT_DRIFT")
    for rust in (a, b):
        witness = (complex_relative_witness(g, rust, digits)["pass"]
                   if family == "mqt" else (
                       numerically_agree(Fraction(rust["real"]),
                                         Fraction(g["real"]), digits)
                       and numerically_agree(Fraction(rust["imag"]),
                                             Fraction(g["imag"]), digits)
                   ))
        checked(witness, "ACTIVE_BITMAP_SIGNED_COMPLEX_WMC_DRIFT")
        checked(rust["precision_bits"] == bits,
                "ACTIVE_BITMAP_MPFR_PRECISION_DRIFT")
        checked(rust["key_arena_enabled"] is True
                and rust["key_arena_calls"] > 0
                and 0 <= rust["key_arena_flat_reuses"] <= rust["key_arena_calls"]
                and 0 <= rust["key_arena_view_reuses"] <= rust["key_arena_calls"],
                "ACTIVE_BITMAP_ARENA_REUSE_DRIFT")
        checked(not rust.get("ordered_key_enabled")
                and not rust.get("scoped_dsu_enabled")
                and not rust.get("factor_ve_enabled"),
                "UNRELATED_SEARCH_ARM")
        checked(not rust.get("stage_enabled"),
                "INSTRUMENTED_NATIVE_WALLTIME_CONTAMINATION")
    checked(a.get("active_bitmap_enabled") is False
            and a.get("active_bitmap_key_visits") == 0
            and b.get("active_bitmap_enabled") is True
            and b.get("active_bitmap_key_visits", 0) > 0,
            "ACTIVE_BITMAP_FLAG_OR_ADMISSION_DRIFT")
    checked(
        a["stdout_sha256"] == b["stdout_sha256"]
        and tuple(a[k] for k in ("nodes", "cache_hits", "decompositions"))
        == tuple(b[k] for k in ("nodes", "cache_hits", "decompositions")),
        "EXACT_CANONICAL_OUTPUT_OR_SEARCH_TOPOLOGY_DRIFT",
    )
    checkout = {r.get("source_checkout_sha") for r in group.values()}
    checked(len(checkout) == 1 and isinstance(next(iter(checkout)), str)
            and len(next(iter(checkout))) == 40,
            "SOURCE_CHECKOUT_MIXED_OR_MISSING")


def check_grover_negative(rows: list[dict], checkout: str) -> None:
    checked(len(rows) == 3 and {r["arm"] for r in rows} == set(ARMS),
            "GROVER_NEGATIVE_CONTROL_ARM_MISSING")
    trials = {r.get("trial") for r in rows}
    checked(trials == {0}, "GROVER_NEGATIVE_TRIAL_DRIFT")
    ref = next(r for r in rows if r["arm"] == "gpmc")
    # Grover is a deliberately censored negative control, not a qualified
    # performance cohort. Original GPMC may itself exceed the unchanged 30s
    # budget at the higher precision; do not erase its negative outcome.
    gpmc_completed = ref["status"] == "COMPLETED" and ref["exit_code"] == 0
    gpmc_timed_out = (
        ref["status"] == "CENSORED_TIME" and ref.get("exit_code") == 124
    ) or ref["status"] == "CENSORED_OUTER_TIMEOUT"
    checked(gpmc_completed or gpmc_timed_out,
            "ORIGINAL_GPMC_GROVER_CONTROL_INVALID_STATUS")
    for row in rows:
        checked(row.get("source_checkout_sha") == checkout,
                "GROVER_NEGATIVE_CHECKOUT_DRIFT")
        if row["arm"] != "gpmc":
            checked(row["status"] == "CENSORED_EXIT"
                    and "node limit" in row.get("stderr_tail", ""),
                    "GROVER_NODE_LIMIT_NEGATIVE_WITNESS_MISSING")


def stats(rows: list[dict]) -> dict:
    series = {arm: [next(r for r in rows
                         if r["trial"] == trial and r["arm"] == arm)
                    for trial in range(TRIALS)] for arm in ARMS}
    summary = {}
    for arm in ARMS:
        z = series[arm]
        summary[arm] = {
            "p50_full_wall_ms": statistics.median(r["wall_ns"] for r in z) / 1e6,
            "p90_full_wall_ms": percentile([r["wall_ns"] for r in z], .9) / 1e6,
            "p50_peak_rss_kib": statistics.median(r["peak_rss_kib"] for r in z),
        }
    old = series["arena_control"]
    new = series["arena_active_bitmap"]
    median_before = summary["arena_control"]["p50_full_wall_ms"]
    median_after = summary["arena_active_bitmap"]["p50_full_wall_ms"]
    tail_before = summary["arena_control"]["p90_full_wall_ms"]
    tail_after = summary["arena_active_bitmap"]["p90_full_wall_ms"]
    rss_before = summary["arena_control"]["p50_peak_rss_kib"]
    rss_after = summary["arena_active_bitmap"]["p50_peak_rss_kib"]
    wins = sum(x["wall_ns"] < y["wall_ns"] for x, y in zip(new, old))
    gates = {
        "p50_incremental_gain_at_least_five_percent":
            median_after * 100 <= median_before * 95,
        "p90_incremental_gain_at_least_five_percent":
            tail_after * 100 <= tail_before * 95,
        "at_least_five_of_seven_paired_wins": wins >= 5,
        "rss_p50_no_more_than_five_percent_higher":
            rss_after * 100 <= rss_before * 105,
    }
    return {
        "arms": summary,
        "paired_wins": wins,
        "paired_p50_wall_delta_ms":
            statistics.median(x["wall_ns"] - y["wall_ns"]
                              for x, y in zip(new, old)) / 1e6,
        "incremental_p50_improvement_percent":
            100 * (1 - median_after / median_before),
        "incremental_p90_improvement_percent":
            100 * (1 - tail_after / tail_before),
        "median_active_bitmap_key_visits":
            statistics.median(r["active_bitmap_key_visits"] for r in new),
        "checks": gates,
        "incremental_research_go": all(gates.values()),
    }


def self_test() -> None:
    def trial(arm: str, idx: int) -> dict:
        is_rust = arm != "gpmc"
        stamp = arm == "arena_active_bitmap"
        return {
            "arm": arm, "trial": idx, "status": "COMPLETED",
            "exit_code": 0, "sat": True, "real": "1/2", "imag": "-1/8",
            "precision_bits": 100, "key_arena_enabled": is_rust,
            "key_arena_calls": 20 if is_rust else 0,
            "key_arena_flat_reuses": 18 if is_rust else 0,
            "key_arena_view_reuses": 18 if is_rust else 0,
            "active_bitmap_enabled": stamp, "active_bitmap_key_visits": 20 if stamp else 0,
            "stdout_sha256": "same-output", "nodes": 12, "cache_hits": 2,
            "decompositions": 1, "ordered_key_enabled": False,
            "scoped_dsu_enabled": False, "factor_ve_enabled": False,
            "stage_enabled": False,
            "source_checkout_sha": "a" * 40,
            "wall_ns": (90 if stamp else 100) * 1_000_000,
            "peak_rss_kib": 6000,
        }
    rows = [trial(arm, i) for i in range(7) for arm in ARMS]
    check_trial({r["arm"]: r for r in rows[:3]}, 100, 30, "feynman")
    assert stats(rows)["incremental_research_go"]
    for arm, field, bad in (
        ("arena_active_bitmap", "imag", "1/8"),
        ("arena_active_bitmap", "nodes", 99),
        ("arena_active_bitmap", "active_bitmap_enabled", False),
        ("arena_active_bitmap", "active_bitmap_key_visits", 0),
        ("arena_active_bitmap", "source_checkout_sha", "b" * 40),
        ("arena_control", "active_bitmap_key_visits", 1),
        ("arena_control", "key_arena_enabled", False),
    ):
        broken = [dict(row) for row in rows[:3]]
        next(r for r in broken if r["arm"] == arm)[field] = bad
        try:
            check_trial({r["arm"]: r for r in broken}, 100, 30, "feynman")
        except ValueError:
            pass
        else:
            raise AssertionError(f"CORRUPTED_ACTIVE_BITMAP_ORACLE: {arm}:{field}")
    slow = [dict(row) for row in rows]
    for row in slow:
        if row["arm"] == "arena_active_bitmap":
            row["wall_ns"] = 99_000_000
    assert not stats(slow)["incremental_research_go"]
    tail = [dict(row) for row in rows]
    for row in tail:
        if row["arm"] == "arena_active_bitmap" and row["trial"] in (5, 6):
            row["wall_ns"] = 111_000_000
    assert not stats(tail)["incremental_research_go"]
    negative = [
        {**trial(arm, 0), "status": "COMPLETED" if arm == "gpmc"
         else "CENSORED_EXIT",
         "stderr_tail": "" if arm == "gpmc" else
         "RESEARCH_MPFR_FAIL_CLOSED: node limit"}
        for arm in ARMS
    ]
    check_grover_negative(negative, "a" * 40)
    timed_gpmc = [dict(row) for row in negative]
    timed_gpmc[0]["status"] = "CENSORED_TIME"
    timed_gpmc[0]["exit_code"] = 124
    check_grover_negative(timed_gpmc, "a" * 40)
    broken_gpmc = [dict(row) for row in timed_gpmc]
    broken_gpmc[0]["status"] = "CENSORED_EXIT"
    broken_gpmc[0]["exit_code"] = 2
    try:
        check_grover_negative(broken_gpmc, "a" * 40)
    except ValueError as ex:
        assert "ORIGINAL_GPMC" in str(ex)
    else:
        raise AssertionError("undocumented original GPMC Grover failure accepted")
    negative[1]["stderr_tail"] = "resource exhausted"
    try:
        check_grover_negative(negative, "a" * 40)
    except ValueError as ex:
        assert "GROVER_NODE_LIMIT" in str(ex)
    else:
        raise AssertionError("censored Grover without node witness passed")
    print("ACTIVE_BITMAP_EXACT_PAIRED_ORACLE_SELF_TEST_PASS", flush=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--family", choices=("feynman", "mqt"))
    for opt in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + opt, type=Path)
    args = ap.parse_args()
    if args.self_test:
        self_test()
        return
    for key in ("family", "quprs", "gpmc_source", "corpus",
                "gpmc", "mpfr", "report"):
        if getattr(args, key) is None:
            ap.error(f"missing --{key.replace('_', '-')}")
    checked(source_head(args.quprs) == QU_PRS_SHA
            and source_head(args.gpmc_source) == GPMC_SHA,
            "PINNED_QU_PRS_ORIGINAL_GPMC_SOURCE_DRIFT")
    checkout = os.environ.get("GITHUB_SHA", "")
    checked(len(checkout) == 40
            and all(c in "0123456789abcdef" for c in checkout),
            "GITHUB_SOURCE_CHECKOUT_SHA_MISSING")
    cases = SOURCE if args.family == "feynman" else ORIGINAL_MQT
    if args.family == "mqt":
        manifest = {}
        for line in (args.corpus / "source-status.tsv").read_text().splitlines():
            fields = line.split("\t")
            checked(len(fields) == 2 and fields[0] in ORIGINAL_MQT
                    and fields[0] not in manifest,
                    "MQT_SOURCE_MANIFEST_DUPLICATE_OR_UNKNOWN")
            manifest[fields[0]] = fields[1]
        checked(set(manifest) == set(ORIGINAL_MQT)
                and all(status == "EXPORTED" for status in manifest.values()),
                "ORIGINAL_MQT_SEVEN_SOURCE_EXPORTS_INCOMPLETE")
    report = {
        "study": "Exact reversible source-clause bitmap admission vs bounded Arena",
        "family": args.family, "arms": list(ARMS),
        "source_sha": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA,
                       "checkout": checkout},
        "precision_map": BITS,
        "no_original_quprs_or_gpmc_modification": True,
        "complete_native_process_trials": TRIALS,
        "resource": "30s per child, 2GiB virtual address-space, peak RSS observed",
        "incremental_gate": "at least 5% p50 AND p90, 5/7 paired wins, <=5% RSS increase",
        "cases": [], "status": "RUNNING",
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    def save() -> None:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    save()
    complete = 0
    negative = 0
    for case, rel in cases.items():
        cnf = args.corpus / f"{case}-gpmc.cnf"
        original = (args.quprs / rel if args.family == "feynman"
                    else args.quprs / "benchmarks" / "MQTBench" / rel)
        checked(cnf.is_file() and original.is_file()
                and cnf.read_bytes().startswith(b"p cnf "),
                f"INVALID_ORIGINAL_EMITTED_CNF:{case}")
        item = {
            "case": case, "qasm_sha256": sha(original),
            "gpmc_cnf_sha256": sha(cnf), "precisions": [],
        }
        if args.family == "mqt":
            copied = args.corpus / f"{case}.qasm"
            ganak = args.corpus / f"{case}-ganak.cnf"
            gp = args.corpus / f"{case}-gpmc-phases.tsv"
            kp = args.corpus / f"{case}-ganak-phases.tsv"
            checked(all(p.exists() and 0 < p.stat().st_size <= MAX_SOURCE_BYTES
                        for p in (copied, cnf, ganak, gp, kp)),
                    f"INVALID_MQT_SOURCE_OR_SIZE:{case}")
            checked(copied.read_bytes() == original.read_bytes(),
                    f"ORIGINAL_MQT_QASM_BYTE_DRIFT:{case}")
            g = parse_cnf(cnf, "gpmc")
            k = parse_cnf(ganak, "ganak")
            checked((g["variables"], g["clauses"], g["weights"])
                    == (k["variables"], k["clauses"], k["weights"])
                    and gp.read_bytes() == kp.read_bytes(),
                    f"ORIGINAL_MQT_SIGNED_WEIGHTS_OR_PHASE_DRIFT:{case}")
            item.update(ganak_cnf_sha256=sha(ganak),
                        phase_sha256=sha(gp))
        report["cases"].append(item)
        save()
        for digits, bits in BITS.items():
            attempts = 1 if case == "feynman_grover_5" else TRIALS
            cohort = {"digits": digits, "bits": bits, "records": [],
                      "status": "RUNNING"}
            item["precisions"].append(cohort)
            save()
            for trial_no in range(attempts):
                order = ARMS[trial_no % len(ARMS):] + ARMS[:trial_no % len(ARMS)]
                for arm in order:
                    row = sample(make_command(args, arm, digits, cnf, case),
                                 arm, digits)
                    row["trial"] = trial_no
                    cohort["records"].append(row)
                    save()
                    print(f"ACTIVE_BITMAP_TRIAL case={case} bits={bits} "
                          f"trial={trial_no} arm={arm} "
                          f"status={row['status']}", flush=True)
                if trial_no == 0 and case != "feynman_grover_5" and any(
                    r["status"] != "COMPLETED" or r.get("wall_ns", 11e9) > 10e9
                    for r in cohort["records"]
                ):
                    cohort["status"] = "CENSORED_FIRST_ROTATION"
                    break
            if case == "feynman_grover_5":
                check_grover_negative(cohort["records"], checkout)
                # Original GPMC completion may vary with precision and
                # runner load. Preserve the actual independent status.
                cohort["gpmc_native_status"] = next(
                    row["status"] for row in cohort["records"]
                    if row["arm"] == "gpmc"
                )
                cohort["status"] = "CENSORED_NODE_BUDGET_NEGATIVE"
                negative += 1
                save()
                continue
            if len(cohort["records"]) != TRIALS * len(ARMS) or any(
                r["status"] != "COMPLETED" for r in cohort["records"]
            ):
                cohort["status"] = "CENSORED_NATIVE_NO_CLAIM"
                save()
                raise SystemExit(f"INCOMPLETE_ACTIVE_BITMAP_NATIVE:{case}:{bits}")
            for trial_no in range(TRIALS):
                check_trial({r["arm"]: r for r in cohort["records"]
                             if r["trial"] == trial_no}, bits,
                            digits, args.family)
            cohort["status"] = "EXACT_NUMERIC_AND_TOPOLOGY_MATCH"
            cohort["stats"] = stats(cohort["records"])
            complete += 1
            print(f"ACTIVE_BITMAP_MATCH case={case} bits={bits} "
                  f"arena_ms={cohort['stats']['arms']['arena_control']['p50_full_wall_ms']:.3f} "
                  f"stamp_ms={cohort['stats']['arms']['arena_active_bitmap']['p50_full_wall_ms']:.3f} "
                  f"gpmc_ms={cohort['stats']['arms']['gpmc']['p50_full_wall_ms']:.3f} "
                  f"wins={cohort['stats']['paired_wins']}/7 "
                  f"gate={'GO' if cohort['stats']['incremental_research_go'] else 'NO_GO'}",
                  flush=True)
            save()
    expected = 6 if args.family == "feynman" else 14
    report["qualified_cohorts"] = complete
    report["negative_grover_cohorts"] = negative
    report["status"] = (
        "NUMERIC_SOURCE_MATCH_PERFORMANCE_RESEARCH_PENDING_REVIEW"
        if complete == expected and negative == (2 if args.family == "feynman" else 0)
        else "INCOMPLETE_SOURCE_COHORT_NO_GO"
    )
    hard = [c for item in report["cases"] if item["case"] in HARD
            for c in item["precisions"] if c.get("status") ==
            "EXACT_NUMERIC_AND_TOPOLOGY_MATCH"]
    report["hard_case_incremental_research_go"] = (
        len(hard) == 2 and all(c["stats"]["incremental_research_go"] for c in hard)
    )
    save()
    checked(report["status"] == "NUMERIC_SOURCE_MATCH_PERFORMANCE_RESEARCH_PENDING_REVIEW",
            "INCOMPLETE_ACTIVE_BITMAP_SOURCE_FAMILY")
    print(f"ACTIVE_BITMAP_FINAL family={args.family} matched={complete}/{expected} "
          f"hard_research_go={report['hard_case_incremental_research_go']}",
          flush=True)


if __name__ == "__main__":
    main()
