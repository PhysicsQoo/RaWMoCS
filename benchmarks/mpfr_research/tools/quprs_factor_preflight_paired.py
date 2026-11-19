#!/usr/bin/env python3
"""Research-only source-locked native A/B for exact structural Factor-VE preflight.

The QuPRS/QASM exporter is outside each fresh solver process WallTime.
Unchanged GPMC, Rust Inline and bounded exact bag14 are control arms.
Never infer production suitability from a single passing cohort.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import statistics
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from quprs_cnf_articulation_ab import (
    BITS, GPMC_SHA, QU_PRS_SHA, percentile, sha, source_head,
)
from quprs_cnf_factor_minfill_ab import (
    build_command as feynman_command,
    sample as feynman_sample,
)
from quprs_mqt_factor_ve_ab import (
    build_trial_command as mqt_command,
    sample as mqt_sample,
)
from quprs_cnf_projected_ab import SOURCE
from quprs_mqtbench_original_cnf_ab import ORIGINAL_MQT, complex_relative_witness

ARMS = ("gpmc", "inline_full", "bag14", "bag14_preflight")
TRIALS = 7
CASES = {
    "feynman": ("feynman_hwb6", "feynman_barenco_tof_5", "feynman_mod_mult_55"),
    "mqt": ("mqt_qnn_4", "mqt_vqe_4", "mqt_qpeexact_8"),
}


def command_for(args: argparse.Namespace, case: str, digits: int,
                arm: str, cnf: Path) -> list[str]:
    if arm not in ARMS:
        raise ValueError("UNKNOWN_PREFLIGHT_ARM")
    legacy = ("factor_ve_minfill_bag14" if args.family == "feynman"
              else "factor_ve_bag14")
    mapped = arm if arm in ("gpmc", "inline_full") else legacy
    command = (feynman_command(args, mapped, digits, cnf, case)
               if args.family == "feynman"
               else mqt_command(args, mapped, digits, cnf, case))
    if arm == "bag14_preflight":
        index = command.index("--hashbrown-borrowed")
        command.insert(index, "--factor-ve-preflight-cost")
    return command


def check_commands() -> None:
    for family in CASES:
        args = argparse.Namespace(
            family=family, gpmc=Path("/original-gpmc"), mpfr=Path("/rust-mpfr"),
        )
        for arm in ARMS:
            cmd = command_for(args, CASES[family][0], 30, arm,
                              Path("/source-locked.cnf"))
            expected = arm == "bag14_preflight"
            if (cmd.count("--factor-ve-preflight-cost") == 1) != expected:
                raise SystemExit("PREFLIGHT_ARM_FLAG_DRIFT")
            if (("--factor-variable-elimination" in cmd)
                    != (arm in ("bag14", "bag14_preflight"))):
                raise SystemExit("FACTOR_VE_CONTROL_CONTAMINATION")
            if arm not in ("bag14", "bag14_preflight"):
                if "--factor-ve-bag-probe-14" in cmd:
                    raise SystemExit("UNMODIFIED_CONTROL_HAS_BAG14")
            elif (cmd.count("--factor-ve-bag-probe-14") != 1
                  or cmd.count("--factor-ve-minfill-order") != 1):
                raise SystemExit("CHANGED_BAG14_STRUCTURAL_CONTROL")
    print("FACTOR_PREFLIGHT_SOURCE_ARM_SELF_TEST_PASS", flush=True)


def sample_arm(args: argparse.Namespace, case: str, digits: int,
               arm: str, cnf: Path) -> dict:
    command = command_for(args, case, digits, arm, cnf)
    legacy = (("factor_ve_minfill_bag14" if args.family == "feynman"
               else "factor_ve_bag14") if arm.startswith("bag14") else arm)
    sample = feynman_sample if args.family == "feynman" else mqt_sample
    row = sample(command, legacy, digits)
    row["arm"] = arm
    return row


def metrics(rows: list[dict], arm: str) -> dict:
    samples = [r for r in rows if r["arm"] == arm]
    return {
        "p50_wall_ms": statistics.median(r["wall_ns"] for r in samples) / 1e6,
        "p90_wall_ms": percentile([r["wall_ns"] for r in samples], 0.9) / 1e6,
        "p50_peak_rss_kib": statistics.median(r["peak_rss_kib"] for r in samples),
        "completed_ve": all(r["factor_ve_completed"] for r in samples)
                        if arm.startswith("bag14") else None,
    }


def numeric_match(reference: dict, other: dict, family: str,
                  digits: int) -> bool:
    if reference["sat"] != other["sat"]:
        return False
    if family == "mqt":
        return bool(complex_relative_witness(reference, other, digits)["pass"])
    from quprs_long_feynman_pilot import numerically_agree
    return (numerically_agree(Fraction(reference["real"]),
                              Fraction(other["real"]), digits)
            and numerically_agree(Fraction(reference["imag"]),
                                  Fraction(other["imag"]), digits))


def source_file(args: argparse.Namespace, case: str) -> Path:
    if args.family == "feynman":
        original = args.quprs / SOURCE[case]
        if not original.is_file():
            raise SystemExit("ORIGINAL_FEYNMAN_SOURCE_MISSING")
    else:
        original = args.quprs / "benchmarks" / "MQTBench" / ORIGINAL_MQT[case]
        copied = args.corpus / (case + ".qasm")
        if (not original.is_file() or not copied.is_file()
                or original.read_bytes() != copied.read_bytes()):
            raise SystemExit("ORIGINAL_MQT_SOURCE_BYTES_DRIFT")
        statuses = (args.corpus / "source-status.tsv").read_text().splitlines()
        if f"{case}\tEXPORTED" not in statuses:
            raise SystemExit("ORIGINAL_MQT_NOT_EXPORTED")
    return original


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--family", choices=CASES, default="feynman")
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        parser.add_argument("--" + name, type=Path)
    args = parser.parse_args()
    check_commands()
    if args.self_test:
        return
    for name in ("quprs", "gpmc_source", "corpus", "gpmc", "mpfr", "report"):
        if getattr(args, name) is None:
            parser.error("missing --" + name.replace("_", "-"))
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("ORIGINAL_GPMC_OR_QU_PRS_SHA_DRIFT")
    report = {
        "study": "Opt-in exact structure-only Factor-VE preflight",
        "family": args.family, "arms": list(ARMS), "trials": TRIALS,
        "quprs_sha": QU_PRS_SHA, "gpmc_sha": GPMC_SHA,
        "checkout_sha": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True).strip(),
        "precision": "GPMC 30/100 decimal digits, Rust MPFR 100/333 bits",
        "native_policy": "fresh process; 30s and 2GiB address space; GNU time RSS",
        "scope": "research-only selected positive and negative original CNF holdouts",
        "cohorts": [],
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    def save() -> None:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    for case in CASES[args.family]:
        original = source_file(args, case)
        cnf = args.corpus / (case + "-gpmc.cnf")
        if not cnf.is_file():
            raise SystemExit("ORIGINAL_WEIGHTED_CNF_NOT_FOUND")
        item = {"case": case, "qasm_sha256": sha(original),
                "cnf_sha256": sha(cnf), "precisions": []}
        report["cohorts"].append(item)
        for digits, bits in BITS.items():
            cohort = {"bits": bits, "digits": digits, "records": [],
                      "status": "INCOMPLETE"}
            item["precisions"].append(cohort)
            save()
            for trial in range(TRIALS):
                rotated = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in rotated:
                    row = sample_arm(args, case, digits, arm, cnf)
                    row["trial"] = trial
                    cohort["records"].append(row)
                    save()
                    print(f"FACTOR_PREFLIGHT_TRIAL case={case} bits={bits} "
                          f"trial={trial} arm={arm} status={row['status']}",
                          flush=True)
            rows = cohort["records"]
            if len(rows) != TRIALS * len(ARMS) or any(
                r["status"] != "COMPLETED" for r in rows
            ):
                cohort["status"] = "CENSORED_OR_INCOMPLETE"
                save()
                raise SystemExit("FACTOR_PREFLIGHT_CENSORED_OR_INCOMPLETE")
            wins = 0
            for trial in range(TRIALS):
                selected = {r["arm"]: r for r in rows if r["trial"] == trial}
                if set(selected) != set(ARMS):
                    raise SystemExit("FACTOR_PREFLIGHT_UNBALANCED_TRIAL")
                control, inline, bag, candidate = (selected[a] for a in ARMS)
                for other in (inline, bag, candidate):
                    if not numeric_match(control, other, args.family, digits):
                        raise SystemExit("FACTOR_PREFLIGHT_SIGNED_COMPLEX_MISMATCH")
                    if other["precision_bits"] != bits:
                        raise SystemExit("FACTOR_PREFLIGHT_PRECISION_MISMATCH")
                if (candidate["factor_ve_completed"]
                        and (not bag["factor_ve_completed"]
                             or candidate["stdout_sha256"] != bag["stdout_sha256"])):
                    raise SystemExit("FACTOR_PREFLIGHT_ACCEPTED_OUTPUT_DRIFT")
                if not candidate["factor_ve_completed"]:
                    if (candidate["nodes"], candidate["cache_hits"],
                            candidate["decompositions"], candidate["stdout_sha256"]) != (
                        inline["nodes"], inline["cache_hits"],
                        inline["decompositions"], inline["stdout_sha256"]
                    ):
                        raise SystemExit("FACTOR_PREFLIGHT_FALLBACK_NOT_EXACT")
                wins += candidate["wall_ns"] < bag["wall_ns"]
            cohort["stats"] = {arm: metrics(rows, arm) for arm in ARMS}
            cohort["paired_wins_preflight_vs_bag14"] = wins
            original_bag = cohort["stats"]["bag14"]
            preflight = cohort["stats"]["bag14_preflight"]
            gain50 = 1 - preflight["p50_wall_ms"] / original_bag["p50_wall_ms"]
            gain90 = 1 - preflight["p90_wall_ms"] / original_bag["p90_wall_ms"]
            rss_ratio = (preflight["p50_peak_rss_kib"]
                         / original_bag["p50_peak_rss_kib"])
            cohort["gain_vs_bag14_p50"] = gain50
            cohort["gain_vs_bag14_p90"] = gain90
            cohort["rss_ratio_vs_bag14"] = rss_ratio
            cohort["status"] = "MATCHED_SIGNED_COMPLEX_WMC"
            save()
            print(f"FACTOR_PREFLIGHT_MATCH case={case} bits={bits} "
                  f"gpmc_ms={cohort['stats']['gpmc']['p50_wall_ms']:.3f} "
                  f"inline_ms={cohort['stats']['inline_full']['p50_wall_ms']:.3f} "
                  f"bag14_ms={original_bag['p50_wall_ms']:.3f} "
                  f"preflight_ms={preflight['p50_wall_ms']:.3f} "
                  f"p50_gain={gain50:.3%} p90_gain={gain90:.3%} "
                  f"wins={wins}/{TRIALS} rss_ratio={rss_ratio:.3f} "
                  f"bag_completed={original_bag['completed_ve']} "
                  f"preflight_completed={preflight['completed_ve']}",
                  flush=True)
    report["status"] = "SCOPED_SOURCE_EXACT_NATIVE_EVIDENCE_NOT_PRODUCTION_GO"
    save()
    print("FACTOR_PREFLIGHT_STUDY_FINISHED status=" + report["status"], flush=True)


if __name__ == "__main__":
    main()
