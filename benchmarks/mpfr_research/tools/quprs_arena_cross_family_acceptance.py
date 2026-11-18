#!/usr/bin/env python3
"""Recompute independent native Feynman + original MQT arena admission.

Research Go is not production approval. Input is the two unchanged JSON
reports archived by source-locked native CI on the same Rust revision.
Never infer speed from profiler or arena allocation counters.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics
import sys

# The original MQT numerical oracle scales both complex components
# by the same full-complex magnitude, including cancellation cases.
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "rust_core" / "tools"))
from quprs_mqtbench_original_cnf_ab import complex_relative_witness
from quprs_cnf_scoped_dsu_ab import numerically_agree, percentile

ARMS = (
    "gpmc", "inline_full", "ordered_key", "arena_reuse",
    "inline_profile", "ordered_profile", "arena_profile",
)
PRECISION = {30: 100, 100: 333}
QU_PRS_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
FEYNMAN = ("feynman_hwb6", "feynman_barenco_tof_5", "feynman_mod_mult_55")
MQT = (
    "mqt_wstate_4", "mqt_vqe_4", "mqt_qnn_4", "mqt_grover_4",
    "mqt_qwalk_3", "mqt_qaoa_7", "mqt_qpeexact_8",
)
HARD = (
    ("feynman_hwb6", 100), ("feynman_hwb6", 333),
    ("mqt_qnn_4", 100), ("mqt_qnn_4", 333),
)


def check(condition: bool, reason: str) -> None:
    if not condition:
        raise ValueError(reason)


def is_sha(value: object) -> bool:
    return (isinstance(value, str) and len(value) == 64
            and all(letter in "0123456789abcdef" for letter in value))


def audit_trials(cohort: dict, case: str, bits: int, kind: str) -> dict:
    digits = {v: k for k, v in PRECISION.items()}[bits]
    rows = cohort["records"]
    check(len(rows) == 7 * len(ARMS), f"INCOMPLETE_NATIVE_TRIALS:{case}:{bits}")
    by_trial: dict[tuple[int, str], dict] = {}
    for r in rows:
        key = (r["trial"], r["arm"])
        check(
            type(r["trial"]) is int and 0 <= r["trial"] < 7
            and r["arm"] in ARMS and key not in by_trial,
            f"INVALID_NATIVE_TRIAL_IDENTITY:{case}:{bits}",
        )
        check(
            r.get("status") == "COMPLETED" and r.get("exit_code") == 0
            and type(r.get("wall_ns")) is int and 0 < r["wall_ns"] <= 30_000_000_000
            and type(r.get("peak_rss_kib")) is int and r["peak_rss_kib"] > 0,
            f"INVALID_NATIVE_RESOURCE_OR_CENSORED_TRIAL:{case}:{bits}:{key}",
        )
        by_trial[key] = r
    check(
        len(by_trial) == 7 * len(ARMS),
        f"MISSING_NATIVE_TRIALS:{case}:{bits}",
    )

    for trial in range(7):
        original = by_trial[(trial, "gpmc")]
        inline = by_trial[(trial, "inline_full")]
        topology = (inline["nodes"], inline["cache_hits"], inline["decompositions"])
        for arm in ARMS[1:]:
            r = by_trial[(trial, arm)]
            check(r["precision_bits"] == bits, f"PRECISION_DRIFT:{case}:{bits}:{arm}")
            check(r["sat"] == original["sat"], f"SAT_DRIFT:{case}:{bits}:{arm}")
            numeric_agrees = (
                complex_relative_witness(original, r, digits)["pass"]
                if kind == "mqt" else (
                    numerically_agree(
                        Fraction(r["real"]), Fraction(original["real"]), digits,
                    ) and numerically_agree(
                        Fraction(r["imag"]), Fraction(original["imag"]), digits,
                    )
                )
            )
            check(numeric_agrees, f"SIGNED_COMPLEX_DRIFT:{case}:{bits}:{arm}")
            check(
                r["stdout_sha256"] == inline["stdout_sha256"]
                and (r["nodes"], r["cache_hits"], r["decompositions"]) == topology,
                f"EXACT_RUST_OUTPUT_OR_TOPOLOGY_DRIFT:{case}:{bits}:{arm}",
            )
            check(r.get("key_arena_enabled") is arm.startswith("arena"),
                  f"ARENA_FLAG_DRIFT:{case}:{bits}:{arm}")
            calls = r.get("key_arena_calls", -1)
            flats = r.get("key_arena_flat_reuses", -1)
            views = r.get("key_arena_view_reuses", -1)
            check(
                (calls > 0 and 0 <= flats <= calls and 0 <= views <= calls)
                if arm.startswith("arena") else (calls, flats, views) == (0, 0, 0),
                f"INVALID_ARENA_REUSE:{case}:{bits}:{arm}",
            )
            check(
                r.get("ordered_key_enabled") is arm.startswith("ordered"),
                f"ORDERED_FLAG_DRIFT:{case}:{bits}:{arm}",
            )
    def raw(arm: str, key: str) -> list[int]:
        return [by_trial[(trial, arm)][key] for trial in range(7)]
    inline = raw("inline_full", "wall_ns")
    arena = raw("arena_reuse", "wall_ns")
    original = raw("gpmc", "wall_ns")
    inline_p50 = statistics.median(inline)
    arena_p50 = statistics.median(arena)
    inline_p90 = percentile(inline, .9)
    arena_p90 = percentile(arena, .9)
    inline_rss = statistics.median(raw("inline_full", "peak_rss_kib"))
    arena_rss = statistics.median(raw("arena_reuse", "peak_rss_kib"))
    wins = sum(x < y for x, y in zip(arena, inline))
    gates = {
        "p50_reduction_at_least_five_percent": arena_p50 * 100 <= inline_p50 * 95,
        "at_least_five_of_seven_paired_wins": wins >= 5,
        "p90_not_worse": arena_p90 <= inline_p90,
        "median_peak_rss_within_five_percent": arena_rss * 100 <= inline_rss * 105,
    }
    return {
        "case": case,
        "bits": bits,
        "digits": digits,
        "trials": 7,
        "original_gpmc_p50_wall_ms": statistics.median(original) / 1e6,
        "original_gpmc_p90_wall_ms": percentile(original, .9) / 1e6,
        "inline_p50_wall_ms": inline_p50 / 1e6,
        "arena_p50_wall_ms": arena_p50 / 1e6,
        "inline_p90_wall_ms": inline_p90 / 1e6,
        "arena_p90_wall_ms": arena_p90 / 1e6,
        "paired_wall_delta_p50_ms": statistics.median(
            x - y for x, y in zip(arena, inline)
        ) / 1e6,
        "paired_wins": wins,
        "p50_wall_improvement_percent": 100 * (1 - arena_p50 / inline_p50),
        "arena_to_original_gpmc_p50_ratio": arena_p50 / statistics.median(original),
        "inline_peak_rss_p50_kib": inline_rss,
        "arena_peak_rss_p50_kib": arena_rss,
        "checks": gates,
        "meets_performance_gate": all(gates.values()),
    }


def audit_family(report: dict, kind: str) -> tuple[list[dict], list[dict]]:
    check(report.get("arms") == list(ARMS), f"WRONG_SEVEN_ARMS:{kind}")
    if kind == "feynman":
        check(report.get("source_lock") == {
            "quprs": QU_PRS_SHA, "gpmc": GPMC_SHA,
        }, "ORIGINAL_FEYNMAN_SOURCE_LOCK_DRIFT")
        check(report.get("status") == "MATCHED_WITH_GROVER_CENSORS"
              and report.get("qualified_cohorts") == 6,
              "FEYNMAN_COHORT_ACCEPTANCE_MISSING")
        expected = set(FEYNMAN) | {"feynman_grover_5"}
    else:
        check(report.get("dataset") ==
              "PINNED_ORIGINAL_MQT_QASM_TO_ORIGINAL_QU_PRS_WEIGHTED_CNF"
              and report.get("not_mqt_published_cnf") is True
              and report.get("quprs_sha") == QU_PRS_SHA
              and report.get("gpmc_sha") == GPMC_SHA
              and report.get("status") == "COMPLETE_14_OF_14_MATCHED_SOURCE_EXACT"
              and report.get("qualified_cohorts") == 14
              and report.get("expected_cohorts") == 14
              and report.get("trials_per_complete_cohort") == 7,
              "ORIGINAL_MQT_SOURCE_LOCK_OR_COHORT_DRIFT")
        expected = set(MQT)
    cases = report["cases"]
    check(len(cases) == len(expected) and
          {c["case"] for c in cases} == expected,
          f"CASE_FAMILY_DRIFT:{kind}")
    accepted, negatives = [], []
    for case in cases:
        name = case["case"]
        check(is_sha(case.get("cnf_sha256")), f"INVALID_CNF_SHA:{name}")
        check(is_sha(case.get("qasm_sha256") if kind == "feynman"
                     else case.get("source_sha256")),
              f"INVALID_ORIGINAL_QASM_SHA:{name}")
        if kind == "mqt":
            check(case.get("export_status") == "EXPORTED"
                  and is_sha(case.get("phase_sidecar_sha256"))
                  and is_sha(case.get("ganak_sha256")),
                  f"ORIGINAL_MQT_EXPORT_OR_PHASE_DRIFT:{name}")
        cohorts = case["precisions"]
        check(len(cohorts) == 2 and
              {(x["digits"], x["bits"]) for x in cohorts} ==
              set(PRECISION.items()), f"PRECISION_COHORT_DRIFT:{name}")
        for c in cohorts:
            bits = c["bits"]
            if name == "feynman_grover_5":
                check(c.get("status") == "CENSORED_ARMS"
                      and len(c["records"]) == len(ARMS),
                      f"ORIGINAL_GROVER_CENSOR_LOST:{bits}")
                status = {x["arm"]: x["status"] for x in c["records"]}
                check(set(status) == set(ARMS)
                      and status["gpmc"] == "COMPLETED"
                      and all(status[arm].startswith("CENSORED_") for arm in ARMS[1:]),
                      f"ORIGINAL_GROVER_NEGATIVE_CONTROL_DRIFT:{bits}")
                negatives.append({"case": name, "bits": bits, "status": status})
                continue
            check(c["status"] == (
                "MATCHED" if kind == "feynman"
                else "MATCHED_SOURCE_EXACT_CROSS_SOLVER"
            ), f"UNQUALIFIED_NATIVE_COHORT:{name}:{bits}")
            accepted.append(audit_trials(c, name, bits, kind))
    return accepted, negatives


def audit(feynman: dict, mqt: dict) -> dict:
    source, negative = audit_family(feynman, "feynman")
    out_of_family, extra = audit_family(mqt, "mqt")
    check(len(source) == 6 and len(out_of_family) == 14
          and len(negative) == 2 and not extra,
          "SOURCE_FAMILY_OR_CENSOR_INCOMPLETE")
    all_rows = source + out_of_family
    hard = [next(x for x in all_rows
                 if (x["case"], x["bits"]) == pair) for pair in HARD]
    winner = all(x["meets_performance_gate"] for x in hard)
    return {
        "status": ("RESEARCH_GO_EXACT_SOURCE_NUMERIC_AND_HARD_CASE_PERFORMANCE"
                   if winner else "HARD_CASE_PERFORMANCE_NO_GO"),
        "source_sha": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "independent_original_mqt_cnf_emitted_by_quprs": True,
        "all_ordinary_source_cohorts_matched": 6,
        "all_independent_mqt_cohorts_matched": 14,
        "negative_original_grover5_controls_retained": negative,
        "required_hard_cases": [{"case": c["case"], "bits": c["bits"]}
                                for c in hard],
        "all_four_hard_cases_pass": winner,
        "hard_case_results": hard,
        "other_cohort_results": [
            x for x in all_rows if (x["case"], x["bits"]) not in HARD
        ],
        "production_router_approval": False,
        "limitations": [
            "Research Go only; the original GPMC gap and DPLL node-limit negatives remain",
            "Each input is a completed source-locked independent native CI JSON; "
            "the script recomputes performance directly from seven raw trials",
            "The scientific gate is not an independent mathematical proof of large CNF WMC",
            "A future implementation commit requires independent final-HEAD native revalidation",
        ],
    }


def self_test() -> None:
    def fake_run(arm: str, trial: int) -> dict:
        arena = arm.startswith("arena")
        ordered = arm.startswith("ordered")
        return {
            "trial": trial, "arm": arm, "status": "COMPLETED", "exit_code": 0,
            "wall_ns": (90 if arena else 100) * 1_000_000,
            "peak_rss_kib": 1000,
            "sat": True, "real": "1/2", "imag": "-1/8",
            "stdout_sha256": "same-signed-complex-output",
            "precision_bits": 100, "nodes": 17, "cache_hits": 3,
            "decompositions": 2,
            "key_arena_enabled": arena, "key_arena_calls": 10 if arena else 0,
            "key_arena_flat_reuses": 8 if arena else 0,
            "key_arena_view_reuses": 8 if arena else 0,
            "ordered_key_enabled": ordered,
        }
    rows = [fake_run(arm, trial) for trial in range(7) for arm in ARMS]
    good = audit_trials({"records": rows}, "synthetic", 100, "feynman")
    assert good["meets_performance_gate"] and good["paired_wins"] == 7
    damaged = [dict(row) for row in rows]
    damaged[3]["imag"] = "1/8"
    try:
        audit_trials({"records": damaged}, "synthetic", 100, "feynman")
    except ValueError as ex:
        assert "SIGNED_COMPLEX_DRIFT" in str(ex)
    else:
        raise AssertionError("signed complex drift passed the gate")
    damaged = [dict(row) for row in rows]
    for row in damaged:
        if row["arm"] == "arena_reuse":
            row["wall_ns"] = 120_000_000
    rejected = audit_trials({"records": damaged}, "synthetic", 100, "feynman")
    assert not rejected["meets_performance_gate"]
    print("EXACT_ARENA_CROSS_FAMILY_ACCEPTANCE_SELF_TEST_PASS")


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--feynman", type=Path)
    p.add_argument("--mqt", type=Path)
    p.add_argument("--out", type=Path)
    p.add_argument("--self-test", action="store_true")
    a = p.parse_args()
    if a.self_test:
        self_test()
        return
    if a.feynman is None or a.mqt is None or a.out is None:
        p.error("source-locked --feynman, --mqt and --out are all mandatory")
    raw_f = a.feynman.read_bytes()
    raw_m = a.mqt.read_bytes()
    report = audit(json.loads(raw_f), json.loads(raw_m))
    report["input_report_sha256"] = {
        "feynman": sha256(raw_f).hexdigest(),
        "mqt": sha256(raw_m).hexdigest(),
    }
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    print(report["status"])
    for row in report["hard_case_results"]:
        print(
            f"{row['case']} bits={row['bits']} "
            f"inline_ms={row['inline_p50_wall_ms']:.3f} "
            f"arena_ms={row['arena_p50_wall_ms']:.3f} "
            f"improvement_pct={row['p50_wall_improvement_percent']:.2f} "
            f"wins={row['paired_wins']}/7 "
            f"gpmc_ms={row['original_gpmc_p50_wall_ms']:.3f} "
            f"gate={'GO' if row['meets_performance_gate'] else 'NO_GO'}"
        )
    if not report["all_four_hard_cases_pass"]:
        raise SystemExit("EXACT_ARENA_HARD_CASE_PERFORMANCE_NO_GO")


if __name__ == "__main__":
    main()
