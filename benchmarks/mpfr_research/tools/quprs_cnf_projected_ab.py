#!/usr/bin/env python3
"""Same original QuPRS Weighted CNF: GPMC versus Rust DPLL equivalent-literal and reversible immutable-source weighted DPLL trail ablation.

The QuPRS fixture generator is a pinned, independently maintained input
source. This runner does NO QASM/PathSum/RR/ANF conversion. Both solvers
receive the SAME CNF file bytes and precision. Never compare source-native
ANF/BDD with CNF solver performance.
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
import tempfile
import time

import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "rust_core" / "tools"))
from quprs_paired_native import parse_out
from quprs_long_feynman_pilot import numerically_agree

QU_PRS_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
SOURCE = {
    "feynman_hwb6": "benchmarks/Feynman/hwb6.qasm",
    "feynman_barenco_tof_5": "benchmarks/Feynman/barenco_tof_5.qasm",
    "feynman_mod_mult_55": "benchmarks/Feynman/mod_mult_55.qasm",
    "feynman_grover_5": "benchmarks/Feynman/grover_5.qasm",
}
BITS = {30: 100, 100: 333}
ARMS = ("gpmc", "control", "equivalence", "trail", "parent_ids", "compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"
BUDGET_S = 30
AS_BYTES = 2147483648


def sha(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def source_head(path: Path) -> str:
    return subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True
    ).strip()


def sample(cmd: list[str], arm: str, precision: int) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        rss_file = Path(tmp) / "rss"
        timed_cmd = [
            "/usr/bin/timeout", "-k", "2s", f"{BUDGET_S}s",
            "prlimit", f"--as={AS_BYTES}", "--",
            "/usr/bin/time", "-f", "%M", "-o", str(rss_file),
            *cmd,
        ]
        start = time.perf_counter_ns()
        try:
            proc = subprocess.run(
                timed_cmd, capture_output=True, text=True, timeout=BUDGET_S + 7
            )
        except subprocess.TimeoutExpired:
            return {"arm": arm, "status": "CENSORED_OUTER_TIMEOUT"}
        wall = time.perf_counter_ns() - start
        row = {
            "arm": arm,
            "status": "COMPLETED" if proc.returncode == 0 else
                      ("CENSORED_TIME" if proc.returncode == 124 else "CENSORED_EXIT"),
            "wall_ns": wall, "exit": proc.returncode,
        }
        if proc.returncode:
            row["stderr_tail"] = proc.stderr[-400:]
            return row
        try:
            sat, (real, imag) = parse_out(
                proc.stdout, "gpmc" if arm == "gpmc" else "rawmocs_mpfr"
            )
            row.update({
                "sat": sat, "real": str(real), "imag": str(imag),
                "peak_rss_kib": int(rss_file.read_text().strip()),
                "stdout_sha256": sha256(proc.stdout.encode()).hexdigest(),
            })
            if arm != "gpmc":
                phase = re.search(
                    r"^RESEARCH_MPFR_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) "
                    r"output=(\d+) nodes=(\d+) cache_hits=(\d+) decompositions=(\d+) "
                    r"digits=(\d+) bits=(\d+)$",
                    proc.stderr, re.MULTILINE,
                )
                probe = re.search(
                    r"^RESEARCH_MPFR_WEIGHTED_EQ enabled=(true|false) "
                    r"binary=(\d+) eliminated=(\d+) removed_clauses=(\d+) "
                    r"representatives=(\d+) contradiction=(true|false) censored=(true|false)$",
                    proc.stderr, re.MULTILINE,
                )
                trail = re.search(
                    r"^RESEARCH_MPFR_TRAIL enabled=(true|false) "
                    r"assignments=(\d+) cache_keys=(\d+)$",
                    proc.stderr, re.MULTILINE,
                )
                parent_ids = re.search(
                    r"^RESEARCH_MPFR_TRAIL_PARENT_IDS enabled=(true|false)$",
                    proc.stderr, re.MULTILINE,
                )
                compact = re.search(
                    r"^RESEARCH_MPFR_TRAIL_COMPACT_RESIDUAL enabled=(true|false)$",
                    proc.stderr, re.MULTILINE,
                )
                prehash = re.search(
                    r"^RESEARCH_MPFR_TRAIL_PREHASHED_CACHE enabled=(true|false)$",
                    proc.stderr, re.MULTILINE,
                )
                fused = re.search(
                    r"^RESEARCH_MPFR_TRAIL_FUSED_COMPACT enabled=(true|false)$",
                    proc.stderr, re.MULTILINE,
                )
                weighted_cache = re.search(
                    r"^RESEARCH_MPFR_TRAIL_WEIGHT_CLASS_CACHE enabled=(true|false)$",
                    proc.stderr, re.MULTILINE,
                )
                scoped_key = re.search(
                    r"^RESEARCH_MPFR_TRAIL_WEIGHT_CLASS_SCOPE_MAX max=(\d+) "
                    r"localized_keys=(\d+) original_keys=(\d+)$",
                    proc.stderr, re.MULTILINE,
                )
                inline_flag = re.search(
                    r"^RESEARCH_MPFR_TRAIL_INLINE_WEIGHT_RELABEL enabled=(true|false)$",
                    proc.stderr, re.MULTILINE,
                )
                post_flag = re.search(
                    r"^RESEARCH_MPFR_POST_UNIT_CACHE enabled=(true|false) "
                    r"hits=(\d+) visited=(\d+) forced=(\d+)$",
                    proc.stderr, re.MULTILINE,
                )
                projected_flag = re.search(
                    r"^RESEARCH_MPFR_POST_UNIT_PROJECTED enabled=(true|false) "
                    r"unchanged=(\d+) projected=(\d+)$",
                    proc.stderr, re.MULTILINE,
                )
                if not phase or not probe or not trail or not parent_ids or not compact or not prehash or not fused or not weighted_cache or not scoped_key or not inline_flag or not post_flag or not projected_flag:
                    raise ValueError("missing Rust solve/equivalence/trail diagnostics")
                if (trail.group(1) == "true") != (arm in ("trail", "parent_ids", "compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("trail selection arm did not match CLI")
                if (parent_ids.group(1) == "true") != (arm in ("parent_ids", "compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("parent clause-ID option does not match arm")
                if (compact.group(1) == "true") != (arm in ("compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("compact-residual flag differs from arm")
                if (prehash.group(1) == "true") != (arm in ("prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("prehashed exact-key option differs from arm")
                if (fused.group(1) == "true") != (arm in ("fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("fused compact literal filter CLI option mismatch")
                if (weighted_cache.group(1) == "true") != (arm in ("weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("weight-class component cache CLI mismatch")
                expected_max = {"scoped16": 16, "scoped64": 64,
                                "inline16": 16, "post16": 16, "projected16": 16}.get(arm, 0)
                if int(scoped_key.group(1)) != expected_max:
                    raise ValueError("small-component key admission threshold mismatch")
                if (inline_flag.group(1) == "true") != (arm in ("inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("inline weighted key option mismatch")
                if (post_flag.group(1) == "true") != (arm in ("post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("post-unit residual cache CLI flag mismatch")
                if (projected_flag.group(1) == "true") != (arm in ("projected_full", "projected16")):
                    raise ValueError("projected post-unit cache CLI flag mismatch")
                stats = [int(v) for v in phase.groups()]
                if stats[7:] != [precision, BITS[precision]]:
                    raise ValueError("Rust binary MPFR precision drift")
                if (probe.group(1) == "true") != (arm in ("equivalence", "trail", "parent_ids", "compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16"):
                    raise ValueError("Rust weighted equivalence arm drift")
                row.update({
                    "solve_ns": stats[2],
                    "nodes": stats[4],
                    "cache_hits": stats[5],
                    "decompositions": stats[6],
                    "precision_bits": stats[8],
                    "equivalence": {
                        "binary_clauses": int(probe.group(2)),
                        "eliminated_vars": int(probe.group(3)),
                        "removed_clauses": int(probe.group(4)),
                        "representatives": int(probe.group(5)),
                        "contradiction": probe.group(6) == "true",
                        "censored": probe.group(7) == "true",
                    },
                    "trail": {
                        "assignments": int(trail.group(2)),
                        "cache_keys": int(trail.group(3)),
                        "parent_ids": parent_ids.group(1) == "true",
                        "compact": compact.group(1) == "true",
                        "prehash": prehash.group(1) == "true",
                        "fused": fused.group(1) == "true",
                        "weight_class_cache": weighted_cache.group(1) == "true",
                        "scope_max": int(scoped_key.group(1)),
                        "post_unit_cache": {
                            "enabled": post_flag.group(1) == "true",
                            "hits": int(post_flag.group(2)),
                            "visited": int(post_flag.group(3)),
                            "forced": int(post_flag.group(4)),
                            "unchanged_keys": int(projected_flag.group(2)),
                            "projected_keys": int(projected_flag.group(3)),
                        },
                        "localized_keys": int(scoped_key.group(2)),
                        "original_keys": int(scoped_key.group(3)),
                    }
                })
        except (ValueError, OSError) as error:
            row["status"] = "CENSORED_OUTPUT_OR_RESOURCE"
            row["message"] = str(error)[:250]
        return row


def percentile(values: list[int], fraction: float) -> float:
    ordered = sorted(values)
    cursor = (len(ordered) - 1) * fraction
    floor = int(cursor)
    return ordered[floor] + (ordered[min(floor + 1, len(ordered) - 1)]
                              - ordered[floor]) * (cursor - floor)


def main() -> None:
    ap = argparse.ArgumentParser()
    for name in ("quprs", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, type=Path, required=True)
    ap.add_argument("--trials", type=int, default=7)
    args = ap.parse_args()
    if args.trials != 7:
        raise SystemExit("confirmation requires seven rotated fresh child trials")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("PINNED_SOURCE_REVISION_DRIFT")
    report = {
        "study": "Domain-separated, bounded 16/64 variable scope admission vs all-scope weight-class component memoization",
        "quprs_revision": QU_PRS_SHA,
        "gpmc_revision": GPMC_SHA,
        "input_boundary": "identical original QuPRS Weighted CNF bytes for all arms; no native ANF path",
        "precision": "GPMC 30/100 decimal digits ↔ Rust 100/333 MPFR bits",
        "resource": {"wall_s": BUDGET_S, "as_bytes": AS_BYTES,
                     "rss": "GNU time observed max RSS, not hard cgroup RSS"},
        "method": "7 rotated fresh child trials per completed arm; one trial for expensive Grover5 (all arms attempted); every failure archived",
        "caveat": "large-source numerical agreement, not independent exact Full WMC certificate",
        "rows": [],
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    qualified = 0
    for case, qasm_path in SOURCE.items():
        cnf = args.corpus / f"{case}-gpmc.cnf"
        if not cnf.is_file() or not (args.quprs / qasm_path).is_file():
            report["rows"].append({"case": case, "status": "CENSORED_MISSING_SOURCE"})
            continue
        model = cnf.read_bytes()
        if not model.startswith(b"p cnf "):
            raise SystemExit(f"{case}: invalid original Weighted CNF")
        case_row = {
            "case": case, "original_qasm_sha256": sha(args.quprs / qasm_path),
            "exact_cnf_sha256": sha(cnf), "cnf_bytes": len(model),
            "precisions": [],
        }
        report["rows"].append(case_row)
        for digits, bits in BITS.items():
            records: list[dict] = []
            cohort = {"digits": digits, "bits": bits, "records": records,
                      "status": "INCOMPLETE"}
            case_row["precisions"].append(cohort)
            case_trials = 1 if case == "feynman_grover_5" else args.trials
            for trial in range(case_trials):
                order = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in order:
                    if arm == "gpmc":
                        cmd = [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}",
                               str(cnf.resolve())]
                    else:
                        cmd = [str(args.mpfr.resolve())]
                        if case in ("feynman_hwb6", "feynman_grover_5"):
                            cmd += ["--node-budget=1000000"]
                        cmd += ["--cache-limit=50000",
                                "--indexed-unit-prop-csr-after-8192",
                                "--packed-residual-key-fast",
                                "--fast-assign", "--reuse-frequency-scratch"]
                        if arm in ("equivalence", "trail", "parent_ids", "compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--weighted-equivalence"]
                        if arm in ("trail", "parent_ids", "compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-count"]
                        if arm in ("parent_ids", "compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-parent-ids"]
                        if arm in ("compact", "prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-compact-residual"]
                        if arm in ("prehash", "fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-prehashed-cache"]
                        if arm in ("fused", "weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-fused-compact"]
                        if arm in ("weight_cache", "scoped16", "scoped64", "inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-weight-class-cache"]
                        if arm in ("scoped16", "scoped64", "inline16", "post16", "projected16"):
                            cmd += ["--trail-cache-scope-max=" + str(
                                {"scoped16": 16, "scoped64": 64, "inline16": 16,
                                 "post16": 16, "projected16": 16}[arm]
                            )]
                        if arm in ("inline16", "inline_full", "post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-inline-weight-relabel"]
                        if arm in ("post_full", "post16", "projected_full", "projected16":
                            cmd += ["--trail-post-unit-cache"]
                        if arm in ("projected_full", "projected16"):
                            cmd += ["--trail-post-unit-projected"]
                        cmd += ["--hashbrown-borrowed", str(digits), str(cnf.resolve())]
                    result = sample(cmd, arm, digits)
                    result["trial"] = trial
                    records.append(result)
                    args.report.write_text(json.dumps(report, indent=2) + "\n")
                    print(f"CNF_TRAIL case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={result['status']} "
                          f"nodes={result.get('nodes', 'NA')} "
                          f"eliminated={result.get('equivalence', {}).get('eliminated_vars', 'NA')} "
                          f"removed={result.get('equivalence', {}).get('removed_clauses', 'NA')} "
                          f"fallback={result.get('equivalence', {}).get('censored', 'NA')} "
                          f"trail_assignments={result.get('trail', {}).get('assignments', 'NA')} "
                          f"trail_cache_keys={result.get('trail', {}).get('cache_keys', 'NA')}", flush=True)
                    if result["status"] != "COMPLETED":
                        cohort["status"] = "CENSORED_FAILED_ARM"
                        cohort.setdefault("censored_arms", []).append(
                            {"arm": arm, "status": result["status"]}
                        )
                if cohort["status"] == "CENSORED_FAILED_ARM":
                    break
            if cohort["status"].startswith("CENSORED"):
                args.report.write_text(json.dumps(report, indent=2) + "\n")
                # Do not rerun a long source at 333 bits if 100-bit Rust
                # already hit its strict node or WallTime limit.
                if digits == 30:
                    break
                continue
            if len(records) != len(ARMS) * case_trials:
                raise SystemExit("unbalanced native source trial count")
            # A different in-place search engine may change node and cache
            # topology. Verify SAT and BOTH Complex WMC components against
            # the unchanged original GPMC; never demand same stdout bytes.
            # Parent clause IDs can only shrink as assignments accumulate.
            # This implementation optimization must leave the weighted search
            # semantics AND traversal topology exactly unchanged relative to
            # the source-scanning Trail engine on the same fixed bytes.
            for trial in range(case_trials):
                base = next(r for r in records
                            if r["trial"] == trial and r["arm"] == "trail")
                optimized = next(r for r in records
                                 if r["trial"] == trial and r["arm"] == "parent_ids")
                fields = (
                    "sat", "real", "imag", "stdout_sha256",
                    "nodes", "cache_hits", "decompositions",
                )
                if any(base[field] != optimized[field] for field in fields):
                    cohort["status"] = "FAILED_TRAIL_PARENT_ID_EXACT_PARITY"
                    cohort["parity_trial"] = trial
                    args.report.write_text(json.dumps(report, indent=2) + "\n")
                    raise SystemExit("FAILED_TRAIL_PARENT_ID_EXACT_PARITY")
            # Compact literals must be exactly equivalent to Parent-ID Trail.
            # The WMC complex components, stdout bytes, search nodes, cache
            # and decompositions must not change because this is only data
            # layout, not a different counting/search strategy.
            for trial in range(case_trials):
                old = next(r for r in records
                           if r["trial"] == trial and r["arm"] == "parent_ids")
                new = next(r for r in records
                           if r["trial"] == trial and r["arm"] == "compact")
                for field in ("sat", "real", "imag", "stdout_sha256",
                              "nodes", "cache_hits", "decompositions"):
                    if old[field] != new[field]:
                        cohort["status"] = "FAILED_COMPACT_EXACT_PARITY"
                        cohort["parity_failure_trial"] = trial
                        cohort["parity_failure_field"] = field
                        args.report.write_text(json.dumps(report, indent=2) + "\n")
                        raise SystemExit("FAILED_COMPACT_EXACT_PARITY")
            # Exact prehash must preserve SAT, both MPFR WMC components,
            # stdout bytes, and the entire cached search topology relative
            # to the already-qualified Compact engine, every completed trial.
            for trial in range(case_trials):
                control = next(r for r in records
                               if r["trial"] == trial and r["arm"] == "compact")
                optimized = next(r for r in records
                                 if r["trial"] == trial and r["arm"] == "prehash")
                for field in ("sat", "real", "imag", "stdout_sha256",
                              "nodes", "cache_hits", "decompositions"):
                    if control[field] != optimized[field]:
                        cohort["status"] = "FAILED_PREHASH_EXACT_CACHE_PARITY"
                        cohort["parity_failure_trial"] = trial
                        cohort["parity_failure_field"] = field
                        args.report.write_text(json.dumps(report, indent=2) + "\n")
                        raise SystemExit("FAILED_PREHASH_EXACT_CACHE_PARITY")
            # Per-trial exact parity: the new single-pass source-clause
            # filter must preserve all complete model-count and traversal
            # semantics of the original prehashed Compact Trail.
            for trial in range(case_trials):
                old = next(r for r in records
                           if r["trial"] == trial and r["arm"] == "prehash")
                new = next(r for r in records
                           if r["trial"] == trial and r["arm"] == "fused")
                for field in ("sat", "real", "imag", "stdout_sha256",
                              "nodes", "cache_hits", "decompositions"):
                    if old[field] != new[field]:
                        cohort["status"] = "FAILED_FUSED_COMPACT_EXACT_PARITY"
                        cohort["parity_failure_trial"] = trial
                        cohort["parity_failure_field"] = field
                        args.report.write_text(json.dumps(report, indent=2) + "\n")
                        raise SystemExit("FAILED_FUSED_COMPACT_EXACT_PARITY")
            for trial in range(case_trials):
                for old_arm, new_arm in (("scoped16", "inline16"), ("weight_cache", "inline_full")):
                    old = next(r for r in records if r["trial"] == trial and r["arm"] == old_arm)
                    new = next(r for r in records if r["trial"] == trial and r["arm"] == new_arm)
                    for field in ("sat", "real", "imag", "stdout_sha256", "nodes", "cache_hits", "decompositions"):
                        if old[field] != new[field]:
                            cohort["status"] = "FAILED_INLINE_EXACT_PARITY"
                            cohort["parity_trial"] = trial
                            cohort["parity_arms"] = [old_arm, new_arm]
                            cohort["parity_field"] = field
                            args.report.write_text(json.dumps(report, indent=2) + "\n")
                            raise SystemExit("FAILED_INLINE_EXACT_PARITY")
            # The projected arm only changes exact cache-key construction.
            # Require complete byte-level WMC and traversal equality.
            for trial in range(case_trials):
                for old_arm, new_arm in (("post_full", "projected_full"),
                                         ("post16", "projected16")):
                    old = next(r for r in records if r["trial"] == trial and r["arm"] == old_arm)
                    new = next(r for r in records if r["trial"] == trial and r["arm"] == new_arm)
                    for field in ("sat", "real", "imag", "stdout_sha256",
                                  "nodes", "cache_hits", "decompositions"):
                        if old[field] != new[field]:
                            cohort["status"] = "FAILED_PROJECTED_EXACT_PARITY"
                            cohort["parity_trial"] = trial
                            cohort["parity_arms"] = [old_arm, new_arm]
                            cohort["parity_field"] = field
                            args.report.write_text(json.dumps(report, indent=2) + "\n")
                            raise SystemExit("FAILED_PROJECTED_EXACT_PARITY")
                    for field in ("hits", "visited", "forced"):
                        if old["trail"]["post_unit_cache"][field] != new["trail"]["post_unit_cache"][field]:
                            cohort["status"] = "FAILED_PROJECTED_POST_UNIT_TOPOLOGY"
                            cohort["parity_trial"] = trial
                            cohort["parity_field"] = field
                            args.report.write_text(json.dumps(report, indent=2) + "\n")
                            raise SystemExit("FAILED_PROJECTED_POST_UNIT_TOPOLOGY")
            reference = next(r for r in records if r["arm"] == "gpmc")
            ref_real, ref_imag = Fraction(reference["real"]), Fraction(reference["imag"])
            agreement = all(
                r["sat"] == reference["sat"] and
                numerically_agree(Fraction(r["real"]), ref_real, digits) and
                numerically_agree(Fraction(r["imag"]), ref_imag, digits)
                for r in records
            )
            by_arm = {
                arm: [r for r in records if r["arm"] == arm] for arm in ARMS
            }
            eq_fallback = any(
                r["equivalence"]["censored"] for r in by_arm["equivalence"]
            )
            if not agreement:
                cohort["status"] = "FAILED_NUMERICAL_AGREEMENT"
                args.report.write_text(json.dumps(report, indent=2) + "\n")
                raise SystemExit("FAILED_FULL_COMPLEX_WMC_OR_SAT")
            else:
                cohort["status"] = ("SOURCE_MPFR_AGREEMENT_EQ_FALLBACK"
                    if eq_fallback else "SOURCE_MPFR_AGREEMENT_ONLY")
                qualified += 1
                cohort["stats"] = {
                    arm: {
                        "p50_wall_ms": statistics.median(
                            r["wall_ns"] for r in by_arm[arm]) / 1e6,
                        "p90_wall_ms": percentile(
                            [r["wall_ns"] for r in by_arm[arm]], 0.9) / 1e6,
                        "p50_rss_kib": statistics.median(
                            r["peak_rss_kib"] for r in by_arm[arm]),
                        "p50_nodes": (statistics.median(
                            r["nodes"] for r in by_arm[arm])
                            if arm != "gpmc" else None),
                        "p50_cache_hits": (statistics.median(
                            r["cache_hits"] for r in by_arm[arm])
                            if arm != "gpmc" else None),
                        "p50_localized_keys": (statistics.median(
                            r["trail"]["localized_keys"] for r in by_arm[arm])
                            if arm != "gpmc" else None),
                    } for arm in ARMS
                }
                stats = cohort["stats"]
                print(f"CNF_TRAIL_MATCHED case={case} bits={bits} "
                      f"gpmc_ms={stats['gpmc']['p50_wall_ms']:.3f} "
                      f"control_ms={stats['control']['p50_wall_ms']:.3f} "
                      f"equivalence_ms={stats['equivalence']['p50_wall_ms']:.3f} "
                      f"trail_ms={stats['trail']['p50_wall_ms']:.3f} "
                      f"parent_ids_ms={stats['parent_ids']['p50_wall_ms']:.3f} "
                      f"compact_ms={stats['compact']['p50_wall_ms']:.3f} "
                      f"prehash_ms={stats['prehash']['p50_wall_ms']:.3f} "
                      f"fused_ms={stats['fused']['p50_wall_ms']:.3f} "
                      f"weight_cache_ms={stats['weight_cache']['p50_wall_ms']:.3f} "
                      f"scoped16_ms={stats['scoped16']['p50_wall_ms']:.3f} "
                      f"scoped64_ms={stats['scoped64']['p50_wall_ms']:.3f} "
                      f"inline16_ms={stats['inline16']['p50_wall_ms']:.3f} "
                      f"inline_full_ms={stats['inline_full']['p50_wall_ms']:.3f} "
                      f"post_full_ms={stats['post_full']['p50_wall_ms']:.3f} "
                      f"post16_ms={stats['post16']['p50_wall_ms']:.3f} "
                      f"projected_full_ms={stats['projected_full']['p50_wall_ms']:.3f} "
                      f"projected16_ms={stats['projected16']['p50_wall_ms']:.3f} "
                      f"control_nodes={stats['control']['p50_nodes']} "
                      f"equivalence_nodes={stats['equivalence']['p50_nodes']} "
                      f"trail_nodes={stats['trail']['p50_nodes']} "
                      f"parent_ids_nodes={stats['parent_ids']['p50_nodes']} "
                      f"compact_nodes={stats['compact']['p50_nodes']} "
                      f"prehash_nodes={stats['prehash']['p50_nodes']} "
                      f"fused_nodes={stats['fused']['p50_nodes']} "
                      f"weight_cache_nodes={stats['weight_cache']['p50_nodes']} "
                      f"scoped16_nodes={stats['scoped16']['p50_nodes']} "
                      f"scoped64_nodes={stats['scoped64']['p50_nodes']} "
                      f"inline16_nodes={stats['inline16']['p50_nodes']} "
                      f"inline_full_nodes={stats['inline_full']['p50_nodes']} "
                      f"post_full_nodes={stats['post_full']['p50_nodes']} "
                      f"post16_nodes={stats['post16']['p50_nodes']} "
                      f"projected_full_nodes={stats['projected_full']['p50_nodes']} "
                      f"projected16_nodes={stats['projected16']['p50_nodes']} "
                      f"eq_fallback={eq_fallback}", flush=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    report["qualified_cohorts"] = qualified
    report["status"] = "FINISHED_WITH_CENSORED_HOLDOUTS" if qualified else "NO_QUALIFIED_COHORTS"
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    if qualified < 4:
        raise SystemExit("insufficient qualified same-precision weighted-CNF cohorts")
    print(f"CNF_TRAIL_STUDY_FINISHED qualified={qualified}", flush=True)


if __name__ == "__main__":
    main()
