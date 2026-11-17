#!/usr/bin/env python3
"""Same original QuPRS Weighted CNF: unchanged GPMC, Rust Inline and exact
opt-in exact signed-complex sum-product factor elimination. Complete native WallTime only.

The source generator is external, pinned, and NOT timed with the solvers.
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

ARMS = ("gpmc", "inline_full", "factor_ve", "scope320")
BUDGET_S = 30
AS_BYTES = 2_147_483_648
INLINE_FLAGS = (
    "--weighted-equivalence", "--trail-count", "--trail-parent-ids",
    "--trail-compact-residual", "--trail-prehashed-cache",
    "--trail-fused-compact", "--trail-weight-class-cache",
    "--trail-inline-weight-relabel",
)

def build_command(args: argparse.Namespace, arm: str, digits: int, cnf: Path, case: str) -> list[str]:
    if arm == "gpmc":
        return [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(cnf.resolve())]
    cmd = [str(args.mpfr.resolve())]
    if case in ("feynman_hwb6", "feynman_grover_5"):
        cmd.append("--node-budget=1000000")
    cmd += ["--cache-limit=50000", "--indexed-unit-prop-csr-after-8192",
            "--packed-residual-key-fast", "--fast-assign",
            "--reuse-frequency-scratch", *INLINE_FLAGS]
    if arm in ("factor_ve", "scope320"):
        cmd.append("--factor-variable-elimination")
    if arm == "scope320":
        cmd.append("--factor-ve-scope320")
    cmd += ["--hashbrown-borrowed", str(digits), str(cnf.resolve())]
    return cmd

def sample(command: list[str], arm: str, digits: int) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        rss = Path(tmp) / "rss"
        timed = ["/usr/bin/timeout", "-k", "2s", f"{BUDGET_S}s",
                 "prlimit", f"--as={AS_BYTES}", "--",
                 "/usr/bin/time", "-f", "%M", "-o", str(rss), *command]
        start = time.perf_counter_ns()
        try:
            proc = subprocess.run(timed, capture_output=True, text=True, timeout=BUDGET_S + 7)
        except subprocess.TimeoutExpired:
            return {"arm": arm, "status": "CENSORED_OUTER_TIMEOUT"}
        wall = time.perf_counter_ns() - start
        row = {"arm": arm, "wall_ns": wall, "exit_code": proc.returncode,
               "status": ("COMPLETED" if proc.returncode == 0 else
                          "CENSORED_TIME" if proc.returncode == 124 else "CENSORED_EXIT")}
        if proc.returncode:
            row["stderr_tail"] = proc.stderr[-700:]
            return row
        try:
            sat, (real, imag) = parse_out(proc.stdout, "gpmc" if arm == "gpmc" else "rawmocs_mpfr")
            row.update(sat=sat, real=str(real), imag=str(imag),
                       peak_rss_kib=int(rss.read_text().strip()),
                       stdout_sha256=sha256(proc.stdout.encode()).hexdigest())
            if arm != "gpmc":
                phase = re.search(
                    r"^RESEARCH_MPFR_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) "
                    r"output=(\d+) nodes=(\d+) cache_hits=(\d+) decompositions=(\d+) "
                    r"digits=(\d+) bits=(\d+)$", proc.stderr, re.M)
                root_only = re.search(
                    r"^RESEARCH_MPFR_TRAIL_ARTICULATION_ROOT_ONLY enabled=(true|false)$",
                    proc.stderr, re.M)
                sep = re.search(
                    r"^RESEARCH_MPFR_TRAIL_ARTICULATION enabled=(true|false) "
                    r"scans=(\d+) candidates=(\d+) chosen=(\d+) skipped=(\d+)$",
                    proc.stderr, re.M)
                failed = re.search(
                    r"^RESEARCH_MPFR_ROOT_FAILED_LITERAL enabled=(true|false) "
                    r"eligible=(\d+) candidates=(\d+) probes=(\d+) implied=(\d+) "
                    r"refutations=(\d+) work=(\d+) contradiction=(true|false) "
                    r"capped=(true|false)$", proc.stderr, re.M)
                centroid = re.search(
                    r"^RESEARCH_MPFR_PRIMAL_BAG_CENTROID enabled=(true|false) "
                    r"root_only=(true|false) eligible=(true|false) capped=(true|false) "
                    r"vars=(\d+) edges=(\d+) width=(\d+) fill=(\d+) "
                    r"distance=(\d+) build_ns=(\d+) scans=(\d+) overrides=(\d+)$",
                    proc.stderr, re.M)
                ve = re.search(
                    r"^RESEARCH_MPFR_FACTOR_VE enabled=(true|false) "
                    r"completed=(true|false) eligible=(true|false) capped=(true|false) "
                    r"input_factors=(\d+) eliminated=(\d+) width=(\d+) cells=(\d+) "
                    r"products=(\d+) zero_sat=(\d+) ve_ns=(\d+)$",
                    proc.stderr, re.M)
                admission = re.search(
                    r"^RESEARCH_MPFR_FACTOR_VE_ADMISSION source_vars=(\d+) "
                    r"scope_vars=(\d+) source_clauses=(\d+) "
                    r"max_clause_width=(\d+) cap_reason=([a-z_]+)$",
                    proc.stderr, re.M)
                scope_max = re.search(
                    r"^RESEARCH_MPFR_FACTOR_VE_SCOPE_MAX max=(256|320)$",
                    proc.stderr, re.M)
                if not phase or not sep or not root_only or not failed or not centroid or not ve or not admission or not scope_max:
                    raise ValueError("missing original-CNF solver phase or articulation diagnostics")
                if (ve.group(1) == "true") != (arm in ("factor_ve", "scope320")):
                    raise ValueError("factor VE arm flag mismatch")
                if (scope_max.group(1) == "320") != (arm == "scope320"):
                    raise ValueError("scope320 arm flag mismatch")
                fields = [int(v) for v in phase.groups()]
                if fields[7] != digits or fields[8] != BITS[digits]:
                    raise ValueError("GPMC↔Rust MPFR precision drift")
                if (sep.group(1) == "true"):
                    raise ValueError("separator flag did not match arm")
                if (failed.group(1) == "true"):
                    raise ValueError("failed-literal flag does not match arm")
                if (root_only.group(1) == "true"):
                    raise ValueError("root-only separator arm flag mismatch")
                if any(int(x) for x in sep.groups()[1:]):
                    raise ValueError("disabled separator emitted nonzero counters")
                row.update(nodes=fields[4], cache_hits=fields[5],
                           decompositions=fields[6], solver_ns=fields[2],
                           separator_scans=int(sep.group(2)),
                           separator_candidates=int(sep.group(3)),
                           separator_chosen=int(sep.group(4)),
                           separator_skipped=int(sep.group(5)),
                           failed_candidates=int(failed.group(3)),
                           failed_probes=int(failed.group(4)),
                           implied_units=int(failed.group(5)),
                           failed_refutations=int(failed.group(6)),
                           failed_work=int(failed.group(7)),
                           failed_contradicted=(failed.group(8) == "true"),
                           failed_capped=(failed.group(9) == "true"),
                           centroid_enabled=(centroid.group(1) == "true"),
                           centroid_root_only=(centroid.group(2) == "true"),
                           centroid_eligible=(centroid.group(3) == "true"),
                           centroid_capped=(centroid.group(4) == "true"),
                           centroid_vars=int(centroid.group(5)),
                           centroid_edges=int(centroid.group(6)),
                           centroid_width=int(centroid.group(7)),
                           centroid_fill=int(centroid.group(8)),
                           centroid_distance=int(centroid.group(9)),
                           centroid_build_ns=int(centroid.group(10)),
                           centroid_scans=int(centroid.group(11)),
                           centroid_overrides=int(centroid.group(12)),
                           factor_ve_enabled=(ve.group(1) == "true"),
                           factor_ve_completed=(ve.group(2) == "true"),
                           factor_ve_eligible=(ve.group(3) == "true"),
                           factor_ve_capped=(ve.group(4) == "true"),
                           factor_ve_factors=int(ve.group(5)),
                           factor_ve_eliminations=int(ve.group(6)),
                           factor_ve_width=int(ve.group(7)),
                           factor_ve_cells=int(ve.group(8)),
                           factor_ve_products=int(ve.group(9)),
                           factor_ve_zero_sat=int(ve.group(10)),
                           factor_ve_ns=int(ve.group(11)),
                           factor_ve_source_vars=int(admission.group(1)),
                           factor_ve_scope_vars=int(admission.group(2)),
                           factor_ve_source_clauses=int(admission.group(3)),
                           factor_ve_max_clause_width=int(admission.group(4)),
                           factor_ve_cap_reason=admission.group(5),
                           factor_ve_scope_max=int(scope_max.group(1)),
                           precision_bits=fields[8])
        except (OSError, ValueError) as exc:
            row["status"] = "CENSORED_INVALID_OUTPUT"
            row["error"] = str(exc)[:250]
        return row

def main() -> None:
    ap = argparse.ArgumentParser()
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, type=Path, required=True)
    ap.add_argument("--trials", type=int, default=7)
    args = ap.parse_args()
    if args.trials != 7:
        raise SystemExit("exactly 7 order-rotated native trials required")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("PINNED_SOURCE_REVISION_DRIFT")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "study": "Original Feynman Weighted CNF guarded source-scope 256 versus 320 exact factor VE",
        "source_lock": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
        "precision": "GPMC 30/100 decimal digits ↔ Rust 100/333 MPFR bits",
        "policy": {"wall_limit_seconds": BUDGET_S, "address_space_bytes": AS_BYTES,
                   "rss": "observed GNU time maxRSS, not strict RSS limit"},
        "arms": list(ARMS), "rows": [],
    }
    qualified = 0
    for case, relative in SOURCE.items():
        cnf = args.corpus / f"{case}-gpmc.cnf"
        qasm = args.quprs / relative
        if not (cnf.is_file() and qasm.is_file() and cnf.read_bytes().startswith(b"p cnf ")):
            report["rows"].append({"case": case, "status": "MISSING_OR_INVALID_SOURCE"})
            continue
        row = {"case": case, "cnf_sha256": sha(cnf),
               "qasm_sha256": sha(qasm), "cnf_bytes": cnf.stat().st_size,
               "precisions": []}
        report["rows"].append(row)
        for digits, bits in BITS.items():
            trials = 1 if case == "feynman_grover_5" else args.trials
            cohort = {"digits": digits, "bits": bits, "records": [], "status": "INCOMPLETE"}
            row["precisions"].append(cohort)
            for trial in range(trials):
                rotated = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in rotated:
                    cmd = build_command(args, arm, digits, cnf, case)
                    entry = sample(cmd, arm, digits)
                    entry["trial"] = trial
                    cohort["records"].append(entry)
                    args.report.write_text(json.dumps(report, indent=2) + "\n")
                    print(f"SCOPE320 case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={entry['status']} "
                          f"nodes={entry.get('nodes','NA')} "
                          f"ve_complete={entry.get('factor_ve_completed','NA')}",
                          flush=True)
            completed = [e for e in cohort["records"] if e["status"] == "COMPLETED"]
            if len(completed) != trials * len(ARMS):
                cohort["status"] = "CENSORED_ARMS"
                if case == "feynman_grover_5":
                    break
                continue
            for trial in range(trials):
                selected = {e["arm"]: e for e in completed if e["trial"] == trial}
                if len(selected) != len(ARMS):
                    raise SystemExit("UNBALANCED_TRIALS")
                control = selected["gpmc"]
                for arm in ARMS[1:]:
                    compared = selected[arm]
                    if compared["sat"] != control["sat"] or not (
                        numerically_agree(Fraction(compared["real"]),
                                          Fraction(control["real"]), digits) and
                        numerically_agree(Fraction(compared["imag"]),
                                          Fraction(control["imag"]), digits)
                    ):
                        cohort["status"] = "FAILED_SIGNED_COMPLEX_WMC"
                        cohort["error"] = {"trial": trial, "arm": arm}
                        args.report.write_text(json.dumps(report, indent=2) + "\n")
                        raise SystemExit("FAILED_SIGNED_COMPLEX_WMC")
            for trial in range(trials):
                selected = {e["arm"]: e for e in completed if e["trial"] == trial}
                inline = selected["inline_full"]
                factor = selected["factor_ve"]
                widened = selected["scope320"]
                if widened["factor_ve_scope_max"] != 320 or factor["factor_ve_scope_max"] != 256:
                    raise SystemExit("UNBALANCED_SOURCE_SCOPE_GUARDS")
                if widened["factor_ve_width"] > 11 or widened["factor_ve_products"] > 600_000:
                    raise SystemExit("WIDENED_VE_BREACHED_FIXED_EXPONENTIAL_CAPS")
                if not widened["factor_ve_completed"] and (
                    widened["nodes"], widened["cache_hits"], widened["decompositions"]
                ) != (inline["nodes"], inline["cache_hits"], inline["decompositions"]):
                    raise SystemExit("WIDENED_VE_FALLBACK_TOPOLOGY_DRIFT")
                if not factor["factor_ve_enabled"] or not widened["factor_ve_enabled"]:
                    raise SystemExit("MISSING_FACTOR_VE_FLAG")
                if factor["factor_ve_width"] > 11 or factor["factor_ve_products"] > 600000:
                    raise SystemExit("UNBOUNDED_FACTOR_VE_WORK")
                if not factor["factor_ve_completed"] and (
                    factor["nodes"], factor["cache_hits"], factor["decompositions"]
                ) != (inline["nodes"], inline["cache_hits"], inline["decompositions"]):
                    raise SystemExit("FACTOR_VE_FALLBACK_TOPOLOGY_DRIFT")
            cohort["status"] = "MATCHED"
            cohort["stats"] = {
                arm: {
                    "p50_wall_ms": statistics.median(
                        e["wall_ns"] for e in completed if e["arm"] == arm) / 1e6,
                    "p90_wall_ms": percentile(
                        [e["wall_ns"] for e in completed if e["arm"] == arm], 0.9) / 1e6,
                    "p50_peak_rss_kib": statistics.median(
                        e["peak_rss_kib"] for e in completed if e["arm"] == arm),
                    "p50_nodes": statistics.median(
                        [e["nodes"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_factor_ve_completed": all(
                        e["factor_ve_completed"] for e in completed if e["arm"] == arm
                    ) if arm != "gpmc" else None,
                    "p50_factor_ve_products": statistics.median(
                        [e["factor_ve_products"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_factor_ve_width": statistics.median(
                        [e["factor_ve_width"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "factor_ve_cap_reasons": sorted(set(
                        e["factor_ve_cap_reason"] for e in completed if e["arm"] == arm
                    )) if arm != "gpmc" else None,
                    "source_envelope": {
                        "vars": max(e["factor_ve_source_vars"] for e in completed if e["arm"] == arm),
                        "scope": max(e["factor_ve_scope_vars"] for e in completed if e["arm"] == arm),
                        "clauses": max(e["factor_ve_source_clauses"] for e in completed if e["arm"] == arm),
                        "max_clause_width": max(e["factor_ve_max_clause_width"] for e in completed if e["arm"] == arm),
                    } if arm != "gpmc" else None,

                } for arm in ARMS
            }
            qualified += 1
            print(f"SCOPE320_MATCH case={case} bits={bits} " +
                  " ".join(f"{arm}_ms={cohort['stats'][arm]['p50_wall_ms']:.3f}"
                           for arm in ARMS) +
                  f" widened_complete={cohort['stats']['scope320']['p50_factor_ve_completed']}" +
                  f" widened_width={cohort['stats']['scope320']['p50_factor_ve_width']}" +
                  f" widened_products={cohort['stats']['scope320']['p50_factor_ve_products']}" +
                  f" widened_reasons={','.join(cohort['stats']['scope320']['factor_ve_cap_reasons'])}" +
                  f" ve_complete={cohort['stats']['factor_ve']['p50_factor_ve_completed']}" +
                  f" ve_width={cohort['stats']['factor_ve']['p50_factor_ve_width']}" +
                  f" ve_products={cohort['stats']['factor_ve']['p50_factor_ve_products']}" +
                  f" cap_reason={','.join(cohort['stats']['factor_ve']['factor_ve_cap_reasons'])}" +
                  f" source_envelope={cohort['stats']['factor_ve']['source_envelope']}",


                  flush=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    report["qualified_cohorts"] = qualified
    report["status"] = "FINISHED_WITH_CENSORS" if qualified else "NO_QUALIFIED_COHORTS"
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    if qualified < 6:
        raise SystemExit("INSUFFICIENT_QUALIFIED_SOURCE_SCOPE320_COHORTS")
    print(f"SCOPE320_STUDY_FINISHED qualified={qualified}", flush=True)

if __name__ == "__main__":
    main()
