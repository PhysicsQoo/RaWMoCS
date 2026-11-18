#!/usr/bin/env python3
"""Native GPMC vs Inline vs bounded ADD and Boolean-BDD Weighted-CNF pilot.

Each arm gets the identical original QuPRS-emitted source CNF, signed complex
literal weights and precision; all preprocessing and BDD planning are timed.
No QASM rewrite is timed or attributed to the solver. Preserve ALL censors.
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

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "rust_core" / "tools"))
from quprs_paired_native import parse_out
from quprs_long_feynman_pilot import numerically_agree
from quprs_cnf_projected_ab import SOURCE, BITS, QU_PRS_SHA, GPMC_SHA, sha, source_head, percentile

ARMS = (
    "gpmc", "inline_full", "factor_ve_minfill320",
    "factor_add", "factor_bool_bdd_natural", "factor_bool_bdd_reverse",
)
BUDGET_S = 30
AS_BYTES = 2_147_483_648
FLAGS = (
    "--weighted-equivalence", "--trail-count", "--trail-parent-ids",
    "--trail-compact-residual", "--trail-prehashed-cache",
    "--trail-fused-compact", "--trail-weight-class-cache",
    "--trail-inline-weight-relabel",
)
PHASE_RE = re.compile(
    r"^RESEARCH_MPFR_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) "
    r"output=(\d+) nodes=(\d+) cache_hits=(\d+) decompositions=(\d+) "
    r"digits=(\d+) bits=(\d+)$", re.M)
ADD_RE = re.compile(
    r"^RESEARCH_MPFR_FACTOR_ADD enabled=(true|false) completed=(true|false) "
    r"eligible=(true|false) capped=(true|false) reason=([a-z_]+) "
    r"nodes=(\d+) terminals=(\d+) apply_calls=(\d+) apply_hits=(\d+) "
    r"abstractions=(\d+) factors=(\d+) eliminated=(\d+) "
    r"max_bucket=(\d+) add_ns=(\d+)$", re.M)
BDD_RE = re.compile(
    r"^RESEARCH_MPFR_BOOL_BDD enabled=(true|false) completed=(true|false) "
    r"eligible=(true|false) capped=(true|false) reason=([a-z_]+) "
    r"reverse_minfill=(true|false) vars=(\d+) clauses=(\d+) nodes=(\d+) "
    r"apply_calls=(\d+) apply_hits=(\d+) eval_calls=(\d+) eval_hits=(\d+) "
    r"minfill_bucket=(\d+) bdd_ns=(\d+)$", re.M)
VE_RE = re.compile(
    r"^RESEARCH_MPFR_FACTOR_VE enabled=(true|false) "
    r"completed=(true|false) eligible=(true|false) capped=(true|false) "
    r"input_factors=(\d+) eliminated=(\d+) width=(\d+) cells=(\d+) "
    r"products=(\d+) zero_sat=(\d+) ve_ns=(\d+)$", re.M)


def command(args, arm, digits, cnf, case):
    if arm == "gpmc":
        return [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(cnf.resolve())]
    cmd = [str(args.mpfr.resolve())]
    if case in ("feynman_hwb6", "feynman_grover_5"):
        cmd.append("--node-budget=1000000")
    cmd.extend(("--cache-limit=50000", "--indexed-unit-prop-csr-after-8192",
                "--packed-residual-key-fast", "--fast-assign",
                "--reuse-frequency-scratch", *FLAGS))
    if arm == "factor_ve_minfill320":
        cmd.extend(("--factor-variable-elimination",
                    "--factor-ve-scope-probe-320", "--factor-ve-minfill-order"))
    if arm == "factor_add":
        cmd.append("--factor-add-elimination")
    if arm.startswith("factor_bool_bdd"):
        cmd.append("--factor-boolean-bdd")
    if arm == "factor_bool_bdd_reverse":
        cmd.append("--factor-bdd-reverse-minfill")
    cmd.extend(("--hashbrown-borrowed", str(digits), str(cnf.resolve())))
    return cmd


def sample(command, arm, digits):
    with tempfile.TemporaryDirectory() as tmp:
        rss = Path(tmp) / "rss"
        bounded = [
            "/usr/bin/timeout", "-k", "2s", f"{BUDGET_S}s",
            "prlimit", f"--as={AS_BYTES}", "--",
            "/usr/bin/time", "-f", "%M", "-o", str(rss), *command,
        ]
        begin = time.perf_counter_ns()
        try:
            proc = subprocess.run(
                bounded, capture_output=True, text=True, timeout=BUDGET_S + 7)
        except subprocess.TimeoutExpired:
            return {"arm": arm, "status": "CENSORED_OUTER_TIMEOUT"}
        row = {
            "arm": arm, "wall_ns": time.perf_counter_ns() - begin,
            "exit_code": proc.returncode,
            "status": ("COMPLETED" if proc.returncode == 0 else
                       "CENSORED_TIME" if proc.returncode == 124
                       else "CENSORED_EXIT"),
        }
        if proc.returncode:
            row["stderr_tail"] = proc.stderr[-1100:]
            return row
        try:
            sat, (real, imag) = parse_out(
                proc.stdout, "gpmc" if arm == "gpmc" else "rawmocs_mpfr")
            row.update(
                sat=sat, real=str(real), imag=str(imag),
                peak_rss_kib=int(rss.read_text().strip()),
                stdout_sha256=sha256(proc.stdout.encode()).hexdigest())
            if arm != "gpmc":
                phase = PHASE_RE.search(proc.stderr)
                bdd = BDD_RE.search(proc.stderr)
                add = ADD_RE.search(proc.stderr)
                dense = VE_RE.search(proc.stderr)
                if not (phase and bdd and add and dense):
                    raise ValueError("missing complete solver/BDD/ADD/Dense diagnostics")
                if int(phase.group(8)) != digits or int(phase.group(9)) != BITS[digits]:
                    raise ValueError("original GPMC versus MPFR precision drift")
                if (bdd.group(1) == "true") != arm.startswith("factor_bool_bdd"):
                    raise ValueError("Boolean BDD CLI dispatch drift")
                if (bdd.group(6) == "true") != (arm == "factor_bool_bdd_reverse"):
                    raise ValueError("Boolean BDD ordering drift")
                if (add.group(1) == "true") != (arm == "factor_add"):
                    raise ValueError("ADD CLI dispatch drift")
                if (dense.group(1) == "true") != (arm == "factor_ve_minfill320"):
                    raise ValueError("Dense Factor-VE dispatch drift")
                row.update(
                    dpll_nodes=int(phase.group(5)),
                    solver_ns=int(phase.group(3)),
                    bdd_completed=bdd.group(2) == "true",
                    bdd_eligible=bdd.group(3) == "true",
                    bdd_capped=bdd.group(4) == "true",
                    bdd_reason=bdd.group(5),
                    bdd_reverse=bdd.group(6) == "true",
                    bdd_vars=int(bdd.group(7)),
                    bdd_clauses=int(bdd.group(8)),
                    bdd_nodes=int(bdd.group(9)),
                    bdd_apply_calls=int(bdd.group(10)),
                    bdd_apply_hits=int(bdd.group(11)),
                    bdd_eval_calls=int(bdd.group(12)),
                    bdd_eval_hits=int(bdd.group(13)),
                    bdd_minfill_bucket=int(bdd.group(14)),
                    bdd_ns=int(bdd.group(15)),
                    add_completed=add.group(2) == "true",
                    add_capped=add.group(4) == "true",
                    add_reason=add.group(5),
                    add_nodes=int(add.group(6)),
                    add_terminals=int(add.group(7)),
                    dense_completed=dense.group(2) == "true",
                    dense_width=int(dense.group(7)),
                )
                if arm.startswith("factor_bool_bdd"):
                    if row["bdd_completed"] == row["bdd_capped"]:
                        raise ValueError("BDD complete/cap invariant")
                    if row["bdd_completed"] and not row["bdd_eligible"]:
                        raise ValueError("BDD admitted without valid source scope")
                    if row["bdd_nodes"] > 50000:
                        raise ValueError("BDD node bound exceeded")
                else:
                    if (row["bdd_completed"] or row["bdd_capped"]
                        or row["bdd_nodes"] or row["bdd_apply_calls"]):
                        raise ValueError("BDD opt-out unexpectedly executed")
                if arm == "factor_add" and row["add_completed"] == row["add_capped"]:
                    raise ValueError("ADD complete/cap invariant")
        except (OSError, ValueError) as exc:
            row["status"] = "CENSORED_INVALID_OUTPUT"
            row["error"] = str(exc)[:280]
        return row


def main():
    parser = argparse.ArgumentParser()
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--trials", required=True, type=int)
    args = parser.parse_args()
    if args.trials != 7:
        raise SystemExit("REQUIRE_SEVEN_ROTATED_FRESH_PROCESSES")
    if (source_head(args.quprs) != QU_PRS_SHA
        or source_head(args.gpmc_source) != GPMC_SHA):
        raise SystemExit("PINNED_QU_PRS_OR_GPMC_SOURCE_DRIFT")
    report = {
        "study": "original QuPRS Weighted CNF Boolean ROBDD before MPFR WMC",
        "source_lock": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "precision": "unchanged GPMC 30/100 decimal digits ↔ Rust MPFR 100/333 bits",
        "limits": {"child_wall_seconds": BUDGET_S, "address_space_bytes": AS_BYTES,
                   "rss": "GNU time observed peak RSS, not strict RSS limit"},
        "arms": ARMS, "rows": [],
    }

    def persist():
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n")

    qualified = 0
    for case, relative in SOURCE.items():
        cnf = args.corpus / f"{case}-gpmc.cnf"
        original_qasm = args.quprs / relative
        if (not cnf.is_file() or not original_qasm.is_file()
            or not cnf.read_bytes().startswith(b"p cnf ")):
            raise SystemExit("MISSING_SOURCE_PINNED_ORIGINAL_WEIGHTED_CNF")
        row = {
            "case": case, "cnf_sha256": sha(cnf),
            "qasm_sha256": sha(original_qasm), "cnf_bytes": cnf.stat().st_size,
            "precisions": [],
        }
        report["rows"].append(row)
        for digits, bits in BITS.items():
            if case == "feynman_grover_5" and digits != 30:
                continue
            trials = 1 if case == "feynman_grover_5" else args.trials
            cohort = {"digits": digits, "bits": bits, "records": [],
                      "status": "INCOMPLETE"}
            row["precisions"].append(cohort)
            for trial in range(trials):
                order = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in order:
                    entry = sample(command(args, arm, digits, cnf, case), arm, digits)
                    entry["trial"] = trial
                    cohort["records"].append(entry)
                    persist()
                    print(
                        f"BOOL_BDD_PILOT case={case} bits={bits} trial={trial} "
                        f"arm={arm} status={entry['status']} "
                        f"bdd_complete={entry.get('bdd_completed','NA')}",
                        flush=True)
            completed = [e for e in cohort["records"] if e["status"] == "COMPLETED"]
            if len(completed) != trials * len(ARMS):
                cohort["status"] = "CENSORED_ARMS"
                persist()
                if case == "feynman_grover_5":
                    continue
                raise SystemExit("CENSORED_REQUIRED_SOURCE_COHORT")
            for trial in range(trials):
                selected = {e["arm"]: e for e in completed if e["trial"] == trial}
                if len(selected) != len(ARMS):
                    raise SystemExit("UNBALANCED_ORDER_ROTATED_TRIALS")
                gold = selected["gpmc"]
                for arm in ARMS[1:]:
                    current = selected[arm]
                    if current["sat"] != gold["sat"] or not (
                        numerically_agree(Fraction(current["real"]),
                                          Fraction(gold["real"]), digits)
                        and numerically_agree(Fraction(current["imag"]),
                                              Fraction(gold["imag"]), digits)
                    ):
                        cohort["status"] = "FAILED_SIGNED_COMPLEX_WMC"
                        cohort["trial"] = trial
                        cohort["failed_arm"] = arm
                        persist()
                        raise SystemExit("SIGNED_COMPLEX_WMC_MISMATCH")
                for arm in ("factor_add", "factor_bool_bdd_natural",
                            "factor_bool_bdd_reverse"):
                    current = selected[arm]
                    completed_key = ("add_completed" if arm == "factor_add"
                                     else "bdd_completed")
                    capped_key = ("add_capped" if arm == "factor_add"
                                  else "bdd_capped")
                    if not current[completed_key] and current[capped_key]:
                        if current["dpll_nodes"] != selected["inline_full"]["dpll_nodes"]:
                            cohort["status"] = "FALLBACK_TOPOLOGY_DRIFT"
                            cohort["failed_arm"] = arm
                            persist()
                            raise SystemExit("FALLBACK_DPLL_TOPOLOGY_DRIFT")
            cohort["status"] = "MATCHED"
            cohort["stats"] = {
                arm: {
                    "p50_wall_ms": statistics.median(
                        e["wall_ns"] for e in completed if e["arm"] == arm) / 1e6,
                    "p90_wall_ms": percentile(
                        [e["wall_ns"] for e in completed if e["arm"] == arm], 0.9) / 1e6,
                    "p50_rss_kib": statistics.median(
                        e["peak_rss_kib"] for e in completed if e["arm"] == arm),
                    "bdd_all_complete": all(
                        e["bdd_completed"] for e in completed if e["arm"] == arm)
                    if arm.startswith("factor_bool_bdd") else None,
                    "bdd_cap_reasons": sorted(set(
                        e["bdd_reason"] for e in completed if e["arm"] == arm
                    )) if arm.startswith("factor_bool_bdd") else None,
                    "p50_bdd_nodes": statistics.median(
                        e["bdd_nodes"] for e in completed if e["arm"] == arm
                    ) if arm.startswith("factor_bool_bdd") else None,
                    "p50_bdd_apply_calls": statistics.median(
                        e["bdd_apply_calls"] for e in completed if e["arm"] == arm
                    ) if arm.startswith("factor_bool_bdd") else None,
                } for arm in ARMS
            }
            qualified += 1
            persist()
            print(
                "BOOL_BDD_MATCH " + f"case={case} bits={bits} " +
                " ".join(f"{arm}_ms={cohort['stats'][arm]['p50_wall_ms']:.3f}"
                         for arm in ARMS) +
                " " + " ".join(
                    f"{arm}_complete={cohort['stats'][arm]['bdd_all_complete']}"
                    f" cap={cohort['stats'][arm]['bdd_cap_reasons']}"
                    for arm in ("factor_bool_bdd_natural", "factor_bool_bdd_reverse")
                ), flush=True)
    report["qualified_cohorts"] = qualified
    report["status"] = "QUALIFIED_WITH_CENSORS" if qualified == 6 else "INCOMPLETE"
    persist()
    if qualified != 6:
        raise SystemExit("MISSING_REQUIRED_SOURCE_PRECISION_COHORTS")
    print(f"BOOL_BDD_FINISHED qualified={qualified}", flush=True)


if __name__ == "__main__":
    main()
