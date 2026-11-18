#!/usr/bin/env python3
"""Native original-CNF GPMC versus exact scoped DSU source-Trail A/B.
Complete native WallTime and capped negative evidence.

The source generator is external, pinned, and NOT timed with the solvers.
"""
from __future__ import annotations
import argparse
from fractions import Fraction
from hashlib import sha256
import json
import os
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

ARMS = ("gpmc", "inline_full", "factor_ve", "residual_ve", "component_local_ve", "scoped_dsu")
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
    if arm == "factor_ve":
        cmd.append("--factor-variable-elimination")
    if arm == "residual_ve":
        cmd.append("--trail-residual-factor-ve")
    if arm == "component_local_ve":
        cmd.append("--trail-component-local-ve")
    if arm == "scoped_dsu":
        cmd.append("--trail-scoped-component-dsu")
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
            # Preserve the source checkout even when a child times out
            # before it can produce stdout, RSS or an exit code.
            return {"arm": arm, "status": "CENSORED_OUTER_TIMEOUT",
                    "source_checkout_sha": os.environ.get("GITHUB_SHA")}
        wall = time.perf_counter_ns() - start
        row = {"arm": arm, "wall_ns": wall, "exit_code": proc.returncode,
               # The exact code checkout used by this Actions run. The PR
               # event GITHUB_SHA is the checked-out synthetic merge commit.
               # Keep it on raw per-arm records so cross-family audits cannot
               # accidentally combine different source revisions.
               "source_checkout_sha": os.environ.get("GITHUB_SHA"),
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
                scoped_dsu = re.search(
                    r"^RESEARCH_MPFR_SCOPED_DSU enabled=(true|false) "
                    r"calls=(\d+) slots=(\d+)$", proc.stderr, re.M)
                ordered = re.search(
                    r"^RESEARCH_MPFR_ORDERED_KEY_FASTPATH enabled=(true|false) "
                    r"probes=(\d+) hits=(\d+)$", proc.stderr, re.M)
                key_arena = re.search(
                    r"^RESEARCH_MPFR_KEY_ARENA_REUSE enabled=(true|false) "
                    r"calls=(\d+) flat_reuses=(\d+) view_reuses=(\d+)$",
                    proc.stderr, re.M,
                )
                component = re.search(
                    r"^RESEARCH_MPFR_COMPONENT_LOCAL_VE enabled=(true|false) "
                    r"attempts=(\d+) completed=(\d+) capped=(\d+) "
                    r"products=(\d+) scans=(\d+)$", proc.stderr, re.M)
                hybrid = re.search(
                    r"^RESEARCH_MPFR_RESIDUAL_VE enabled=(true|false) "
                    r"attempts=(\d+) completed=(\d+) capped=(\d+) "
                    r"products=(\d+) max_width=(\d+)$",
                    proc.stderr, re.M)
                if not phase or not sep or not root_only or not failed or not centroid or not ve or not admission or not hybrid or not component or not scoped_dsu:
                    raise ValueError("missing original-CNF solver phase or articulation diagnostics")
                if (ve.group(1) == "true") != (arm == "factor_ve"):
                    raise ValueError("factor VE arm flag mismatch")
                if (hybrid.group(1) == "true") != (arm == "residual_ve"):
                    raise ValueError("residual VE arm flag mismatch")
                if (component.group(1) == "true") != (arm == "component_local_ve"):
                    raise ValueError("component local VE arm flag mismatch")
                if (scoped_dsu.group(1) == "true") != (arm in ("scoped_dsu", "scoped_profile")):
                    raise ValueError("scoped DSU arm flag mismatch")
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
                           residual_ve_enabled=(hybrid.group(1) == "true"),
                           residual_ve_attempts=int(hybrid.group(2)),
                           residual_ve_completed=int(hybrid.group(3)),
                           residual_ve_capped=int(hybrid.group(4)),
                           residual_ve_products=int(hybrid.group(5)),
                           residual_ve_width=int(hybrid.group(6)),
                           component_ve_enabled=component.group(1) == "true",
                           component_ve_attempts=int(component.group(2)),
                           component_ve_completed=int(component.group(3)),
                           component_ve_capped=int(component.group(4)),
                           component_ve_products=int(component.group(5)),
                           component_ve_scans=int(component.group(6)),
                           scoped_dsu_enabled=scoped_dsu.group(1) == "true",
                           scoped_dsu_calls=int(scoped_dsu.group(2)),
                           scoped_dsu_slots=int(scoped_dsu.group(3)),
                           ordered_key_enabled=(ordered.group(1) == "true") if ordered else False,
                           ordered_key_probes=int(ordered.group(2)) if ordered else 0,
                           ordered_key_hits=int(ordered.group(3)) if ordered else 0,
                           key_arena_enabled=(key_arena.group(1) == "true") if key_arena else False,
                           key_arena_calls=int(key_arena.group(2)) if key_arena else 0,
                           key_arena_flat_reuses=int(key_arena.group(3)) if key_arena else 0,
                           key_arena_view_reuses=int(key_arena.group(4)) if key_arena else 0,
                           precision_bits=fields[8])
                stage = re.search(
                    r"^RESEARCH_MPFR_TRAIL_STAGE_PROFILE enabled=(true|false) "
                    r"key_ns=(\d+) lookup_ns=(\d+) unit_ns=(\d+) "
                    r"frequency_ns=(\d+) components_ns=(\d+)$",
                    proc.stderr, re.M,
                )
                if stage is not None:
                    row.update(stage_enabled=stage.group(1) == "true",
                               stage_key_ns=int(stage.group(2)),
                               stage_lookup_ns=int(stage.group(3)),
                               stage_unit_ns=int(stage.group(4)),
                               stage_frequency_ns=int(stage.group(5)),
                               stage_components_ns=int(stage.group(6)))
                subphases = re.search(
                    r"^RESEARCH_MPFR_TRAIL_KEY_SUBPHASE enabled=(true|false) "
                    r"materialize_ns=(\d+) order_ns=(\d+) pack_ns=(\d+)$",
                    proc.stderr, re.M,
                )
                if subphases is not None:
                    row.update(key_subphase_enabled=subphases.group(1) == "true",
                               key_materialize_ns=int(subphases.group(2)),
                               key_order_ns=int(subphases.group(3)),
                               key_pack_ns=int(subphases.group(4)))
        except (OSError, ValueError) as exc:
            row["status"] = "CENSORED_INVALID_OUTPUT"
            row["error"] = str(exc)[:250]
            row["stderr_tail"] = proc.stderr[-1600:]
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
        "study": "Original Weighted CNF exact scoped-component DSU versus original Trail and pinned GPMC",
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
                    print(f"RESIDUAL_VE case={case} bits={bits} trial={trial} "
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
                hybrid = selected["residual_ve"]
                if not hybrid["residual_ve_enabled"] or inline["residual_ve_enabled"]:
                    raise SystemExit("RESIDUAL_VE_ARM_CONTAMINATION")
                if hybrid["residual_ve_attempts"] > 8:
                    raise SystemExit("UNBOUNDED_RESIDUAL_VE_ATTEMPTS")
                if hybrid["residual_ve_products"] > 8 * 600000:
                    raise SystemExit("UNBOUNDED_RESIDUAL_VE_PRODUCTS")
                if hybrid["residual_ve_completed"] == 0 and (
                    hybrid["nodes"], hybrid["cache_hits"], hybrid["decompositions"]
                ) != (inline["nodes"], inline["cache_hits"], inline["decompositions"]):
                    raise SystemExit("RESIDUAL_VE_FALLBACK_TOPOLOGY_DRIFT")
                local = selected["component_local_ve"]
                if not local["component_ve_enabled"] or any(
                    selected[arm]["component_ve_enabled"]
                    for arm in ARMS if arm != "component_local_ve" and arm != "gpmc"
                ):
                    raise SystemExit("COMPONENT_LOCAL_ARM_CONTAMINATION")
                if local["component_ve_attempts"] > 8 or (
                    local["component_ve_completed"] + local["component_ve_capped"]
                    != local["component_ve_attempts"]
                ):
                    raise SystemExit("COMPONENT_LOCAL_WORK_BUDGET")
                if local["component_ve_products"] > 8 * 600000:
                    raise SystemExit("COMPONENT_LOCAL_PRODUCTS_BUDGET")
                if local["component_ve_completed"] == 0 and (
                    local["nodes"], local["cache_hits"], local["decompositions"]
                ) != (inline["nodes"], inline["cache_hits"], inline["decompositions"]):
                    raise SystemExit("COMPONENT_LOCAL_FALLBACK_TOPOLOGY_DRIFT")
                compact = selected["scoped_dsu"]
                if not compact["scoped_dsu_enabled"] or any(
                    selected[a]["scoped_dsu_enabled"]
                    for a in ARMS[1:] if a != "scoped_dsu"
                ):
                    raise SystemExit("SCOPED_DSU_ARM_CONTAMINATION")
                if (compact["nodes"], compact["cache_hits"], compact["decompositions"]) != (
                    inline["nodes"], inline["cache_hits"], inline["decompositions"]
                ):
                    raise SystemExit("SCOPED_DSU_SEARCH_TOPOLOGY_DRIFT")
                if compact["scoped_dsu_calls"] != inline["scoped_dsu_calls"]:
                    raise SystemExit("SCOPED_DSU_CALL_COUNT_DRIFT")
                if compact["scoped_dsu_slots"] > inline["scoped_dsu_slots"]:
                    raise SystemExit("SCOPED_DSU_NOT_ACTUALLY_SCOPED")
                if not factor["factor_ve_enabled"]:
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
                    "p50_residual_ve_completed": statistics.median(
                        [e["residual_ve_completed"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_scoped_dsu_calls": statistics.median(
                        [e["scoped_dsu_calls"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_scoped_dsu_slots": statistics.median(
                        [e["scoped_dsu_slots"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_component_ve_attempts": statistics.median(
                        [e["component_ve_attempts"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_component_ve_completed": statistics.median(
                        [e["component_ve_completed"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_component_ve_products": statistics.median(
                        [e["component_ve_products"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_component_ve_scans": statistics.median(
                        [e["component_ve_scans"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_residual_ve_attempts": statistics.median(
                        [e["residual_ve_attempts"] for e in completed if e["arm"] == arm]
                    ) if arm != "gpmc" else None,
                    "p50_residual_ve_products": statistics.median(
                        [e["residual_ve_products"] for e in completed if e["arm"] == arm]
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
            print(f"SCOPED_DSU_MATCH case={case} bits={bits} " +
                  " ".join(f"{arm}_ms={cohort['stats'][arm]['p50_wall_ms']:.3f}"
                           for arm in ARMS) +
                  f" scoped_dsu_calls={cohort['stats']['scoped_dsu']['p50_scoped_dsu_calls']}" +
                  f" scoped_dsu_slots={cohort['stats']['scoped_dsu']['p50_scoped_dsu_slots']}" +
                  f" inline_dsu_slots={cohort['stats']['inline_full']['p50_scoped_dsu_slots']}" +
                  f" component_completed={cohort['stats']['component_local_ve']['p50_component_ve_completed']}" +
                  f" component_attempts={cohort['stats']['component_local_ve']['p50_component_ve_attempts']}" +
                  f" component_products={cohort['stats']['component_local_ve']['p50_component_ve_products']}" +
                  f" hybrid_completed={cohort['stats']['residual_ve']['p50_residual_ve_completed']}" +
                  f" hybrid_attempts={cohort['stats']['residual_ve']['p50_residual_ve_attempts']}" +
                  f" hybrid_products={cohort['stats']['residual_ve']['p50_residual_ve_products']}" +
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
        raise SystemExit("INSUFFICIENT_QUALIFIED_RESIDUAL_FACTOR_VE_COHORTS")
    print(f"SCOPED_DSU_STUDY_FINISHED qualified={qualified}", flush=True)

if __name__ == "__main__":
    main()
