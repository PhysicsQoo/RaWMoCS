#!/usr/bin/env python3
"""Bounded *new* original QuPRS Feynman source-CNF cohort.

No fabricated score for censored export/solve. Same MPFR precision and
same emitted CNF are required for any native GPMC/Rust WallTime pair.
The large instances have NO independent exhaustive Full WMC oracle.
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
from typing import Any

from quprs_encoder_validate import parse_cnf
from quprs_paired_native import parse_out

QU_PRS_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
SOURCE_FILES = {
    "feynman_grover_5": "benchmarks/Feynman/grover_5.qasm",
    "feynman_hwb6": "benchmarks/Feynman/hwb6.qasm",
    "feynman_barenco_tof_5": "benchmarks/Feynman/barenco_tof_5.qasm",
    "feynman_mod_mult_55": "benchmarks/Feynman/mod_mult_55.qasm",
}
NUMERIC_BITS = {30: 100, 100: 333}
SOURCE_MAX_BYTES = 2_000_000
WALL_BUDGET_SECONDS = 30
AS_BUDGET_BYTES = 2_147_483_648


def sha_file(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def native(command: list[str], tool: str) -> dict[str, Any]:
    with tempfile.TemporaryDirectory() as folder:
        rss_path = Path(folder) / "rss"
        cmd = [
            "/usr/bin/timeout", "-k", "2s", f"{WALL_BUDGET_SECONDS}s",
            "prlimit", f"--as={AS_BUDGET_BYTES}", "--",
            "/usr/bin/time", "-f", "%M", "-o", str(rss_path),
            *command,
        ]
        start = time.perf_counter_ns()
        try:
            run = subprocess.run(cmd, capture_output=True, text=True, timeout=36)
        except subprocess.TimeoutExpired:
            return {"status": "OUTER_TIMEOUT", "tool": tool}
        wall_ns = time.perf_counter_ns() - start
        if run.returncode:
            return {
                "status": "TIMEOUT" if run.returncode == 124 else "NATIVE_ERROR_OR_UNKNOWN",
                "tool": tool, "exit_code": run.returncode,
                "stderr_tail": run.stderr[-450:],
            }
        try:
            sat, counts = parse_out(run.stdout, tool)
            rss = int(rss_path.read_text().strip())
        except (ValueError, OSError) as error:
            return {"status": "INVALID_NATIVE_OUTPUT", "tool": tool, "error": str(error)}
        phase = re.search(
            r"^RESEARCH_MPFR_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) "
            r"output=(\d+) nodes=(\d+) cache_hits=(\d+) decompositions=(\d+) "
            r"digits=(\d+) bits=(\d+)$",
            run.stderr, flags=re.MULTILINE,
        ) if tool == "rawmocs_mpfr" else None
        record = {
            "status": "COMPLETED", "tool": tool, "sat": sat,
            "real": str(counts[0]), "imag": str(counts[1]),
            "wall_ns": wall_ns, "peak_rss_kib": rss,
            "stdout_sha256": sha256(run.stdout.encode("utf-8")).hexdigest(),
            "mpfr_precision_bits": int(phase.group(9)) if phase else None,
        }
        if phase:
            fields = [int(x) for x in phase.groups()]
            record["solve_ns"] = fields[2]
            record["dpll_nodes"] = fields[4]
            record["cache_hits"] = fields[5]
            record["component_splits"] = fields[6]
        return record


def numerically_agree(a: Fraction, b: Fraction, digits: int) -> bool:
    # Different summation order can change low bits even for identical
    # MPFR mantissa widths. This is an agreement guard, NOT a certificate.
    threshold = Fraction(1, 10 ** (digits - 6))
    return abs(a - b) <= threshold * max(Fraction(1), abs(a), abs(b))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", type=Path, required=True)
    ap.add_argument("--gpmc-source", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--gpmc", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=2)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if args.trials != 2:
        raise SystemExit("exploratory long holdout is fixed at two trials per tool")
    if subprocess.check_output(
        ["git", "-C", str(args.source), "rev-parse", "HEAD"], text=True
    ).strip() != QU_PRS_SHA:
        raise SystemExit("QuPRS source revision drift")
    if subprocess.check_output(
        ["git", "-C", str(args.gpmc_source), "rev-parse", "HEAD"], text=True
    ).strip() != GPMC_SHA:
        raise SystemExit("native GPMC source revision drift")
    statuses = {}
    status_file = args.corpus / "source-status.tsv"
    for line in status_file.read_text().splitlines():
        bits = line.split("\t")
        if len(bits) != 2 or bits[0] not in SOURCE_FILES:
            raise SystemExit("invalid source exporter status log")
        statuses[bits[0]] = bits[1]
    if set(statuses) != set(SOURCE_FILES):
        raise SystemExit("not all source export attempts accounted for")
    report: dict[str, Any] = {
        "study": "original Feynman larger QuPRS PathSum/WmcManager source scout",
        "quprs_source_sha": QU_PRS_SHA,
        "gpmc_source_sha": GPMC_SHA,
        "numeric_contract": "GPMC/Rust native same 100- or 333-bit MPFR, not exact rational vs float",
        "limits": {
            "export_timeout_seconds": 30,
            "solver_wall_seconds": WALL_BUDGET_SECONDS,
            "virtual_address_bytes": AS_BUDGET_BYTES,
            "max_cnf_bytes_for_solver": SOURCE_MAX_BYTES,
        },
        "accuracy_limitations": "No independent complete Full WMC oracle on these larger sources; solver agreement only",
        "selection": "original committed Feynman QASMs only; zero QASM modification or transpilation",
        "rows": [],
    }
    for case, relative in SOURCE_FILES.items():
        original = args.source / relative
        row: dict[str, Any] = {
            "case": case, "original_source": relative,
            "original_qasm_sha256": sha_file(original),
            "source_status": statuses[case], "samples": [],
        }
        if statuses[case] != "EXPORTED":
            row["status"] = "CENSORED_SOURCE_EXPORT"
            report["rows"].append(row)
            print(f"LONG_SOURCE_CENSORED case={case} reason={statuses[case]}")
            continue
        emitted = args.corpus / f"{case}.qasm"
        first = args.corpus / f"{case}-gpmc.cnf"
        second = args.corpus / f"{case}-ganak.cnf"
        phase_a = args.corpus / f"{case}-gpmc-phases.tsv"
        phase_b = args.corpus / f"{case}-ganak-phases.tsv"
        paths = (emitted, first, second, phase_a, phase_b)
        if not all(p.is_file() for p in paths):
            row["status"] = "CENSORED_INCOMPLETE_SOURCE"
        elif emitted.read_bytes() != original.read_bytes():
            raise SystemExit(f"{case}: original QASM source bytes changed")
        elif max(p.stat().st_size for p in paths) > SOURCE_MAX_BYTES:
            row["status"] = "CENSORED_INPUT_EXCEEDS_SOLVER_BOUNDS"
        else:
            try:
                a, b = parse_cnf(first, "gpmc"), parse_cnf(second, "ganak")
            except (ValueError, OSError) as exc:
                row["status"] = "CENSORED_BAD_SOURCE_DIALECT"
                row["detail"] = str(exc)
            else:
                if (a["variables"], a["clauses"], a["weights"]) != (
                    b["variables"], b["clauses"], b["weights"]
                ) or phase_a.read_bytes() != phase_b.read_bytes():
                    raise SystemExit(f"{case}: original GPMC/Ganak source mismatch")
                row.update({
                    "source_cnf_sha256": sha_file(first),
                    "source_phase_sha256": sha_file(phase_a),
                    "variables": a["variables"], "clauses": len(a["clauses"]),
                })
                if a["variables"] > 4096 or len(a["clauses"]) > 50_000:
                    row["status"] = "CENSORED_ENGINE_CNF_LIMIT"
                else:
                    row["precision_cohorts"] = []
                    for digits, width in NUMERIC_BITS.items():
                        # The 333-bit cohort is only attempted when 100-bit
                        # already gave a matched, fully completed reference.
                        if digits > 30 and row["precision_cohorts"][0]["status"] != "SOLVER_AGREEMENT_ONLY":
                            row["precision_cohorts"].append({
                                "bits": width, "status": "NOT_RUN_100BIT_PAIR_INCOMPLETE"
                            })
                            continue
                        samples = []
                        for trial in range(args.trials):
                            order = ("gpmc", "rawmocs_mpfr") if trial % 2 == 0 else (
                                "rawmocs_mpfr", "gpmc"
                            )
                            for tool in order:
                                cmd = (
                                    [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(first.resolve())]
                                    if tool == "gpmc"
                                    else [
                                        str(args.mpfr.resolve()), "--fast-assign", "--hashbrown-borrowed",
                                        str(digits), str(first.resolve()),
                                    ]
                                )
                                sample = native(cmd, tool)
                                sample["trial"], sample["digits"] = trial, digits
                                samples.append(sample)
                        row["samples"].extend(samples)
                        cohort: dict[str, Any] = {"bits": width, "digits": digits}
                        if any(s["status"] != "COMPLETED" for s in samples):
                            cohort["status"] = "CENSORED_NATIVE_TIMEOUT_ERROR_OR_UNKNOWN"
                            cohort["trial_status"] = [
                                (s["tool"], s["status"]) for s in samples
                            ]
                        elif any(
                            s["tool"] == "rawmocs_mpfr" and s["mpfr_precision_bits"] != width
                            for s in samples
                        ):
                            cohort["status"] = "CENSORED_RUST_MPFR_PRECISION_DRIFT"
                        elif len({s["sat"] for s in samples}) != 1:
                            cohort["status"] = "CENSORED_SAT_DISAGREEMENT"
                        elif not all(
                            numerically_agree(
                                Fraction(s["real"]), Fraction(samples[0]["real"]), digits
                            ) and numerically_agree(
                                Fraction(s["imag"]), Fraction(samples[0]["imag"]), digits
                            ) for s in samples
                        ):
                            cohort["status"] = "CENSORED_NUMERIC_DISAGREEMENT"
                        else:
                            cohort["status"] = "SOLVER_AGREEMENT_ONLY"
                            cohort["median_wall_ns"] = {
                                tool: statistics.median(
                                    s["wall_ns"] for s in samples if s["tool"] == tool
                                ) for tool in ("gpmc", "rawmocs_mpfr")
                            }
                            cohort["median_peak_rss_kib"] = {
                                tool: statistics.median(
                                    s["peak_rss_kib"] for s in samples if s["tool"] == tool
                                ) for tool in ("gpmc", "rawmocs_mpfr")
                            }
                            cohort["ratio_gpmc_over_rust"] = (
                                cohort["median_wall_ns"]["gpmc"] /
                                cohort["median_wall_ns"]["rawmocs_mpfr"]
                            )
                        row["precision_cohorts"].append(cohort)
                    row["status"] = "SOURCE_QUALIFIED_WITH_BOUNDED_SOLVER_RESULTS"
        report["rows"].append(row)
        print(
            f"LONG_QU_PRS case={case} source_status={row['source_status']} "
            f"status={row['status']} variables={row.get('variables', 'CENSORED')} "
            f"precision={[(c['bits'],c['status']) for c in row.get('precision_cohorts', [])]}"
        )
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("LONG_SOURCE_PILOT_COMPLETE source_cases=", len(report["rows"]))


if __name__ == "__main__":
    main()
