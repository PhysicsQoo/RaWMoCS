#!/usr/bin/env python3
"""Original source-locked exact full-live-clause residual-key research.

No source regeneration during a solver run. Compare independent native
processes using identical Weighted CNF and GPMC/Rust effective precision.
Do not promote a fastpath from correctness or instrumentation counters alone.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "rust_core" / "tools"))
from quprs_cnf_articulation_ab import BITS, GPMC_SHA, QU_PRS_SHA, sha, source_head, percentile
from quprs_cnf_factor_minfill_ab import build_command as feynman_command
from quprs_mqt_factor_ve_ab import build_trial_command as mqt_command
from quprs_factor_preflight_paired import source_file
from quprs_mqtbench_original_cnf_ab import complex_relative_witness
from quprs_long_feynman_pilot import numerically_agree
from quprs_paired_native import parse_out

ARMS = ("gpmc", "inline_full", "full_live_clause")
CASES = {
    "feynman": ("feynman_hwb6", "feynman_barenco_tof_5", "feynman_mod_mult_55"),
    "mqt": ("mqt_qnn_4", "mqt_vqe_4", "mqt_qpeexact_8"),
}
TRIALS = 7
DEADLINE_S = 30
ADDRESS_SPACE = 2_147_483_648
HARD_CASES = {"feynman": "feynman_hwb6", "mqt": "mqt_qnn_4"}
# Prospective research-only thresholds, never production acceptance.
MIN_WALL_GAIN = 0.05
MIN_PAIRED_WINS = 5
MAX_RSS_RATIO = 1.05


def command_for(args: argparse.Namespace, case: str, cnf: Path,
                digits: int, arm: str) -> list[str]:
    if arm not in ARMS:
        raise ValueError("UNKNOWN_FULL_LIVE_ARM")
    control = "gpmc" if arm == "gpmc" else "inline_full"
    cmd = (feynman_command(args, control, digits, cnf, case)
           if args.family == "feynman"
           else mqt_command(args, control, digits, cnf, case))
    if arm == "full_live_clause":
        insertion = cmd.index("--hashbrown-borrowed")
        cmd.insert(insertion, "--trail-full-live-clause-fastpath")
    return cmd


def command_preflight() -> None:
    if BITS != {30: 100, 100: 333}:
        raise SystemExit("PRECISION_MAP_CHANGED")
    for family, cases in CASES.items():
        args = argparse.Namespace(
            family=family, gpmc=Path("/source-gpmc"), mpfr=Path("/source-rust")
        )
        for arm in ARMS:
            cmd = command_for(args, cases[0], Path("/original-weighted.cnf"), 30, arm)
            enabled = "--trail-full-live-clause-fastpath" in cmd
            if enabled != (arm == "full_live_clause"):
                raise SystemExit("FULL_LIVE_FLAG_CONTROL_CONTAMINATION")
            if arm != "gpmc":
                required = ("--weighted-equivalence", "--trail-count",
                            "--trail-compact-residual", "--trail-fused-compact",
                            "--trail-weight-class-cache", "--trail-inline-weight-relabel")
                if not all(flag in cmd for flag in required):
                    raise SystemExit("WEIGHTED_TRAIL_CONTROL_CHANGED")
                if cmd.index("--trail-inline-weight-relabel") > cmd.index(
                        "--trail-full-live-clause-fastpath" if enabled
                        else "--hashbrown-borrowed"):
                    raise SystemExit("INVALID_FULL_LIVE_FLAG_ORDER")
    print("FULL_LIVE_KEY_SOURCE_ARM_PREFLIGHT_PASS", flush=True)


def run_one(command: list[str], arm: str, bits: int, digits: int) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        rss = Path(tmp) / "rss"
        timed = [
            "/usr/bin/timeout", "-k", "2s", f"{DEADLINE_S}s",
            "prlimit", f"--as={ADDRESS_SPACE}", "--",
            "/usr/bin/time", "-f", "%M", "-o", str(rss), *command,
        ]
        started = time.perf_counter_ns()
        try:
            proc = subprocess.run(timed, capture_output=True, text=True,
                                  timeout=DEADLINE_S + 7)
        except subprocess.TimeoutExpired:
            return {"arm": arm, "status": "CENSORED_OUTER_TIMEOUT"}
        elapsed = time.perf_counter_ns() - started
        row = {
            "arm": arm, "wall_ns": elapsed,
            "exit_code": proc.returncode,
            "status": ("COMPLETED" if proc.returncode == 0 else
                       "CENSORED_TIME" if proc.returncode == 124 else "CENSORED_EXIT"),
        }
        if proc.returncode:
            row["stderr_tail"] = proc.stderr[-900:]
            return row
        try:
            sat, (real, imag) = parse_out(
                proc.stdout, "gpmc" if arm == "gpmc" else "rawmocs_mpfr")
            row.update(
                sat=sat, real=str(real), imag=str(imag),
                peak_rss_kib=int(rss.read_text().strip()),
                stdout_sha256=sha256(proc.stdout.encode()).hexdigest(),
            )
            if arm != "gpmc":
                phase = re.search(
                    r"^RESEARCH_MPFR_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) "
                    r"output=(\d+) nodes=(\d+) cache_hits=(\d+) decompositions=(\d+) "
                    r"digits=(\d+) bits=(\d+)$", proc.stderr, re.M)
                full_live = re.search(
                    r"^RESEARCH_MPFR_FULL_LIVE_KEY enabled=(true|false) hits=(\d+)$",
                    proc.stderr, re.M)
                if not phase or not full_live:
                    raise ValueError("MISSING_EXACT_TRAIL_OR_FULL_LIVE_DIAGNOSTIC")
                phase_vals = [int(v) for v in phase.groups()]
                if phase_vals[7] != digits or phase_vals[8] != bits:
                    raise ValueError("SIGNED_COMPLEX_PRECISION_MISMATCH")
                if (full_live.group(1) == "true") != (arm == "full_live_clause"):
                    raise ValueError("FULL_LIVE_RUNTIME_FLAG_MISMATCH")
                hits = int(full_live.group(2))
                # A source-exact negative control may have no fully-live
                # clauses after equivalence/units. Zero candidate hits is
                # therefore a measured observation, not a failure.
                if arm == "inline_full" and hits != 0:
                    raise ValueError("DISABLED_FULL_LIVE_HIT_COUNTER_NONZERO")
                row.update(
                    nodes=phase_vals[4],
                    cache_hits=phase_vals[5],
                    decompositions=phase_vals[6],
                    solver_ns=phase_vals[2],
                    precision_bits=phase_vals[8],
                    fully_live_clauses=hits,
                )
        except (ValueError, OSError) as exc:
            row["status"] = "CENSORED_INVALID_OUTPUT"
            row["error"] = str(exc)[:200]
        return row


def agrees(a: dict, b: dict, family: str, digits: int) -> bool:
    if a["sat"] != b["sat"]:
        return False
    if family == "mqt":
        return bool(complex_relative_witness(a, b, digits)["pass"])
    return (
        numerically_agree(Fraction(a["real"]), Fraction(b["real"]), digits)
        and numerically_agree(Fraction(a["imag"]), Fraction(b["imag"]), digits)
    )


def summary(records: list[dict], arm: str) -> dict:
    a = [r for r in records if r["arm"] == arm]
    return {
        "p50_wall_ms": statistics.median(r["wall_ns"] for r in a) / 1e6,
        "p90_wall_ms": percentile([r["wall_ns"] for r in a], 0.9) / 1e6,
        "p50_peak_rss_kib": statistics.median(r["peak_rss_kib"] for r in a),
        "p50_source_full_live_hits": (
            statistics.median(r["fully_live_clauses"] for r in a)
            if arm == "full_live_clause" else None
        ),
    }



def hard_case_performance_gate(cohort: dict) -> dict:
    """Fail-closed native performance signal; numeric correctness is separate.

    At least 5% p50 and p90 speedup, >=5/7 paired wins and no more than
    5% median RSS growth must all hold. Only compare arms in this cohort.
    A passing family signal is never a global or production GO.
    """
    if cohort.get("status") != "MATCHED_SAT_AND_SIGNED_COMPLEX_WMC":
        return {"status": "PERFORMANCE_NO_GO", "reason": "NUMERIC_NOT_QUALIFIED"}
    try:
        stats = cohort["stats"]
        gpmc, inline, fast = (stats[arm] for arm in ARMS)
        p50 = (gpmc["p50_wall_ms"], inline["p50_wall_ms"],
               fast["p50_wall_ms"])
        p90 = (inline["p90_wall_ms"], fast["p90_wall_ms"])
        rss = (inline["p50_peak_rss_kib"], fast["p50_peak_rss_kib"])
        wins = cohort["paired_wins_vs_original_inline"]
        if (any(not isinstance(value, (float, int)) or value <= 0
                for value in (*p50, *p90, *rss))
                or not isinstance(wins, int) or not 0 <= wins <= TRIALS):
            raise ValueError("invalid native performance fields")
    except (KeyError, TypeError, ValueError):
        return {"status": "PERFORMANCE_NO_GO", "reason": "MISSING_OR_INVALID_METRIC"}
    checks = {
        "p50_gain_at_least_5pct": p50[2] <= (1 - MIN_WALL_GAIN) * p50[1],
        "p90_gain_at_least_5pct": p90[1] <= (1 - MIN_WALL_GAIN) * p90[0],
        "paired_wins_at_least_5_of_7": wins >= MIN_PAIRED_WINS,
        "median_rss_within_105pct": rss[1] <= MAX_RSS_RATIO * rss[0],
        "gpmc_gap_narrows": p50[2] / p50[0] < p50[1] / p50[0],
    }
    return {
        "status": ("FAMILY_HARD_CASE_SIGNAL_ONLY" if all(checks.values())
                   else "PERFORMANCE_NO_GO"),
        "checks": checks,
        "gpmc_gap_before": p50[1] / p50[0],
        "gpmc_gap_after": p50[2] / p50[0],
    }


def hard_case_gate_self_test() -> None:
    """Positive and adversarial negatives must stay distinct from numeric CI."""
    good = {
        "status": "MATCHED_SAT_AND_SIGNED_COMPLEX_WMC",
        "paired_wins_vs_original_inline": 6,
        "stats": {
            "gpmc": {"p50_wall_ms": 50},
            "inline_full": {
                "p50_wall_ms": 100, "p90_wall_ms": 120,
                "p50_peak_rss_kib": 10_000,
            },
            "full_live_clause": {
                "p50_wall_ms": 94, "p90_wall_ms": 112,
                "p50_peak_rss_kib": 10_200,
            },
        },
    }
    assert hard_case_performance_gate(good)["status"] == "FAMILY_HARD_CASE_SIGNAL_ONLY"
    for change in ("p50", "p90", "wins", "rss", "missing", "numeric"):
        bad = json.loads(json.dumps(good))
        if change == "p50":
            bad["stats"]["full_live_clause"]["p50_wall_ms"] = 99
        elif change == "p90":
            bad["stats"]["full_live_clause"]["p90_wall_ms"] = 122
        elif change == "wins":
            bad["paired_wins_vs_original_inline"] = 4
        elif change == "rss":
            bad["stats"]["full_live_clause"]["p50_peak_rss_kib"] = 11_000
        elif change == "missing":
            del bad["stats"]["gpmc"]["p50_wall_ms"]
        else:
            bad["status"] = "CENSORED_OR_INCOMPLETE"
        assert hard_case_performance_gate(bad)["status"] == "PERFORMANCE_NO_GO", change
    print("FULL_LIVE_HARD_CASE_GATE_SELF_TEST_PASS", flush=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--family", choices=CASES, default="feynman")
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, type=Path)
    args = ap.parse_args()
    command_preflight()
    if args.self_test:
        hard_case_gate_self_test()
        return
    for attr in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, attr) is None:
            ap.error("missing --" + attr.replace("_", "-"))
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("PINNED_SOURCE_REVISION_DRIFT")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "study": "Exact unassigned source-clause residual key fast path",
        "family": args.family,
        "arms": list(ARMS),
        "trials": TRIALS,
        "source_lock": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "precision": "GPMC 30/100 decimal digits versus Rust MPFR 100/333 bits",
        "bounds": {"wall_seconds": DEADLINE_S, "address_space": ADDRESS_SPACE},
        "readme": "Correctness and full-process native walltime are separate gates",
        "cases": [],
    }
    def persist() -> None:
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    for case in CASES[args.family]:
        original = source_file(args, case)
        cnf = args.corpus / (case + "-gpmc.cnf")
        if not cnf.is_file():
            raise SystemExit("ORIGINAL_WEIGHTED_CNF_MISSING")
        item = {"case": case, "qasm_sha256": sha(original),
                "cnf_sha256": sha(cnf), "precisions": []}
        report["cases"].append(item)
        for digits, bits in BITS.items():
            cohort = {"digits": digits, "bits": bits, "status": "INCOMPLETE",
                      "records": []}
            item["precisions"].append(cohort)
            persist()
            for trial in range(TRIALS):
                rotated = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in rotated:
                    command = command_for(args, case, cnf, digits, arm)
                    row = run_one(command, arm, bits, digits)
                    row["trial"] = trial
                    cohort["records"].append(row)
                    persist()
                    print(f"FULL_LIVE_KEY_TRIAL case={case} bits={bits} "
                          f"trial={trial} arm={arm} status={row['status']}",
                          flush=True)
            rows = cohort["records"]
            if len(rows) != TRIALS * len(ARMS) or any(
                    r["status"] != "COMPLETED" for r in rows):
                cohort["status"] = "CENSORED_OR_INCOMPLETE"
                persist()
                raise SystemExit("FULL_LIVE_KEY_UNQUALIFIED_COHORT")
            wins = 0
            for trial in range(TRIALS):
                selected = {r["arm"]: r for r in rows if r["trial"] == trial}
                if set(selected) != set(ARMS):
                    raise SystemExit("UNBALANCED_FULL_LIVE_SOURCE_ARMS")
                gpmc, baseline, candidate = (selected[a] for a in ARMS)
                if not (agrees(gpmc, baseline, args.family, digits)
                        and agrees(gpmc, candidate, args.family, digits)):
                    raise SystemExit("FULL_LIVE_SIGNED_COMPLEX_WMC_MISMATCH")
                if (candidate["stdout_sha256"] != baseline["stdout_sha256"]
                        or (candidate["nodes"], candidate["cache_hits"],
                            candidate["decompositions"]) !=
                        (baseline["nodes"], baseline["cache_hits"],
                         baseline["decompositions"])):
                    raise SystemExit("FULL_LIVE_ORIGINAL_SEARCH_OR_BYTES_DRIFT")
                wins += candidate["wall_ns"] < baseline["wall_ns"]
            cohort["stats"] = {a: summary(rows, a) for a in ARMS}
            cohort["paired_wins_vs_original_inline"] = wins
            cohort["status"] = "MATCHED_SAT_AND_SIGNED_COMPLEX_WMC"
            if case == HARD_CASES[args.family]:
                cohort["hard_case_performance_gate"] = hard_case_performance_gate(cohort)
                print(f"FULL_LIVE_HARD_CASE_GATE case={case} bits={bits} "
                      f"decision={cohort['hard_case_performance_gate']['status']}",
                      flush=True)
            persist()
            s = cohort["stats"]
            print(f"FULL_LIVE_KEY_MATCH case={case} bits={bits} "
                  f"gpmc_ms={s['gpmc']['p50_wall_ms']:.3f} "
                  f"inline_ms={s['inline_full']['p50_wall_ms']:.3f} "
                  f"fast_ms={s['full_live_clause']['p50_wall_ms']:.3f} "
                  f"p90_inline_ms={s['inline_full']['p90_wall_ms']:.3f} "
                  f"p90_fast_ms={s['full_live_clause']['p90_wall_ms']:.3f} "
                  f"wins={wins}/{TRIALS} "
                  f"hits={s['full_live_clause']['p50_source_full_live_hits']:.0f}",
                  flush=True)
    hard = [cohort["hard_case_performance_gate"]
            for item in report["cases"]
            for cohort in item["precisions"]
            if "hard_case_performance_gate" in cohort]
    report["family_performance_gate"] = (
        "FAMILY_HARD_CASE_SIGNAL_ONLY"
        if len(hard) == len(BITS)
        and all(x["status"] == "FAMILY_HARD_CASE_SIGNAL_ONLY" for x in hard)
        else "HARD_CASE_PERFORMANCE_NO_GO"
    )
    # Even a two-precision family signal cannot pass independent
    # cross-family or production gates without the other family.
    report["status"] = "SOURCE_LOCKED_NUMERIC_EVIDENCE_NOT_PRODUCTION_GO"
    persist()
    print("FULL_LIVE_KEY_STUDY_FINISHED status=" + report["status"], flush=True)


if __name__ == "__main__":
    main()
