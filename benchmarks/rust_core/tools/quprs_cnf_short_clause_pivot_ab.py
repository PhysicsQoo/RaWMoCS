#!/usr/bin/env python3
"""Same original QuPRS Weighted CNF: GPMC versus Rust CNF-only short-clause pivot ablation.

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
ARMS = ("gpmc", "frequency", "short", "balanced")
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
                pivot = re.search(
                    r"^RESEARCH_MPFR_PIVOT_POLICY (frequency|short-clause|balanced-short-clause)$",
                    proc.stderr, re.MULTILINE,
                )
                if not phase or not pivot:
                    raise ValueError("missing Rust solver/pivot diagnostic")
                stats = [int(v) for v in phase.groups()]
                if stats[7:] != [precision, BITS[precision]]:
                    raise ValueError("Rust binary MPFR precision drift")
                expected_policy = {
                    "frequency": "frequency", "short": "short-clause",
                    "balanced": "balanced-short-clause",
                }[arm]
                if pivot.group(1) != expected_policy:
                    raise ValueError("Rust CNF pivot arm drift")
                row.update({
                    "solve_ns": stats[2],
                    "nodes": stats[4],
                    "cache_hits": stats[5],
                    "decompositions": stats[6],
                    "precision_bits": stats[8],
                    "pivot_policy": pivot.group(1),
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
    ap.add_argument("--trials", type=int, default=4)
    args = ap.parse_args()
    if args.trials != 4:
        raise SystemExit("pilot requires four balanced fresh process trials")
    if source_head(args.quprs) != QU_PRS_SHA or source_head(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("PINNED_SOURCE_REVISION_DRIFT")
    report = {
        "study": "Rust source-CNF-only weighted short-clause / polarity-balanced pivot policy",
        "quprs_revision": QU_PRS_SHA,
        "gpmc_revision": GPMC_SHA,
        "input_boundary": "identical original QuPRS Weighted CNF bytes for all arms; no native ANF path",
        "precision": "GPMC 30/100 decimal digits ↔ Rust 100/333 MPFR bits",
        "resource": {"wall_s": BUDGET_S, "as_bytes": AS_BYTES,
                     "rss": "GNU time observed max RSS, not hard cgroup RSS"},
        "method": "4 balanced rotated fresh process trials per arm; Grover5 may be censored; each trial saved",
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
            for trial in range(args.trials):
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
                        if arm == "short":
                            cmd += ["--pivot-short-clause"]
                        elif arm == "balanced":
                            cmd += ["--pivot-balanced-short-clause"]
                        cmd += ["--hashbrown-borrowed", str(digits), str(cnf.resolve())]
                    result = sample(cmd, arm, digits)
                    result["trial"] = trial
                    records.append(result)
                    args.report.write_text(json.dumps(report, indent=2) + "\n")
                    print(f"CNF_PIVOT case={case} bits={bits} trial={trial} "
                          f"arm={arm} status={result['status']} "
                          f"nodes={result.get('nodes', 'NA')} "
                          f"pivot={result.get('pivot_policy', 'NA')}", flush=True)
                    if result["status"] != "COMPLETED":
                        cohort["status"] = "CENSORED_FAILED_ARM"
                        cohort["reason"] = {"arm": arm, "status": result["status"]}
                # First pilot attempts all four strategies so a censored
                # baseline does not suppress a successful alternative.
                if cohort["status"] == "CENSORED_FAILED_ARM":
                    break
            if cohort["status"].startswith("CENSORED"):
                args.report.write_text(json.dumps(report, indent=2) + "\n")
                # Do not rerun a long source at 333 bits if 100-bit Rust
                # already hit its strict node or WallTime limit.
                if digits == 30:
                    break
                continue
            if len(records) != len(ARMS) * args.trials:
                raise SystemExit("unbalanced native source trial count")
            reference = next(r for r in records if r["arm"] == "gpmc")
            ref_real, ref_imag = Fraction(reference["real"]), Fraction(reference["imag"])
            agreement = all(
                r["sat"] == reference["sat"] and
                numerically_agree(Fraction(r["real"]), ref_real, digits) and
                numerically_agree(Fraction(r["imag"]), ref_imag, digits)
                for r in records
            )
            rust_arms = [r for r in records if r["arm"] != "gpmc"]
            by_arm = {
                arm: [r for r in records if r["arm"] == arm] for arm in ARMS
            }
            if not agreement:
                cohort["status"] = "FAILED_NUMERICAL_OR_PIVOT_ADMISSION"
            else:
                cohort["status"] = "SOURCE_MPFR_AGREEMENT_ONLY"
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
                    } for arm in ARMS
                }
                stats = cohort["stats"]
                print(f"CNF_PIVOT_MATCHED case={case} bits={bits} "
                      f"gpmc_ms={stats['gpmc']['p50_wall_ms']:.3f} "
                      f"frequency_ms={stats['frequency']['p50_wall_ms']:.3f} "
                      f"short_ms={stats['short']['p50_wall_ms']:.3f} "
                      f"balanced_ms={stats['balanced']['p50_wall_ms']:.3f} "
                      f"frequency_nodes={stats['frequency']['p50_nodes']} "
                      f"short_nodes={stats['short']['p50_nodes']} "
                      f"balanced_nodes={stats['balanced']['p50_nodes']}", flush=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    report["qualified_cohorts"] = qualified
    report["status"] = "FINISHED_WITH_CENSORED_HOLDOUTS" if qualified else "NO_QUALIFIED_COHORTS"
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    if qualified < 4:
        raise SystemExit("insufficient qualified same-precision weighted-CNF cohorts")
    print(f"CNF_SHORT_CLAUSE_PIVOT_STUDY_FINISHED qualified={qualified}", flush=True)


if __name__ == "__main__":
    main()
