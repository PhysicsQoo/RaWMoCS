#!/usr/bin/env python3
"""QuPRS original encoder representative circuit source and native-paired study.

The generated examples call unmodified QuPRS PathSum/WmcManager. Censored cases
remain visible: unknown, timeout, unsupported, error and unequal printed WMC
are never represented as timings with a successful solver outcome.
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
from typing import Any

from quprs_encoder_validate import parse_cnf
from quprs_encoder_paired import exact_printed_cnf, independent_wmc
from quprs_paired_native import parse_out

CASES = (
    "ghz2", "ghz3", "ghz4", "graph3", "bv3", "qft2",
    "qft3", "grover2", "qaoa3", "feynman_mod5_4", "feynman_qft_4",
)
MAX_SOLVER_CNF_VARS = 4096
MAX_SOLVER_CNF_CLAUSES = 50_000
MAX_SOURCE_CNF_BYTES = 2_000_000
RELATIVE_DECIMAL_GUARD = Fraction(1, 10**16)
ABS_DECIMAL_GUARD = Fraction(1, 10**22)


def guarded_execution(command: list[str], tool: str) -> dict[str, Any]:
    with tempfile.TemporaryDirectory() as temp:
        target = str(Path(temp) / "peak_rss.txt")
        wrapper = [
            "/usr/bin/timeout", "-k", "2s", "8s",
            "prlimit", "--as=2147483648", "--",
            "/usr/bin/time", "-f", "%M", "-o", target,
            *command,
        ]
        try:
            result = subprocess.run(wrapper, capture_output=True, text=True, timeout=13)
        except subprocess.TimeoutExpired:
            return {"tool": tool, "status": "OUTER_TIMEOUT"}
        if result.returncode:
            reason = {
                124: "TIMEOUT",
                137: "KILLED",
                152: "CPU_OR_SIGNAL",
            }.get(result.returncode, "NATIVE_ERROR_OR_SOLVER_UNKNOWN")
            return {
                "tool": tool, "status": reason, "exit_code": result.returncode,
                "stderr_last_300": result.stderr[-300:],
            }
        try:
            sat, count = parse_out(result.stdout, tool)
            rss = int(Path(target).read_text().strip())
        except (ValueError, OSError) as exc:
            return {"tool": tool, "status": "INVALID_NATIVE_OUTPUT", "detail": str(exc)}
        # GNU time reports maximum resident set in KiB; elapsed WallTime
        # needs a monotonic host sample, not an empirical CPU Simulation.
        record = {
            "tool": tool, "status": "COMPLETED",
            "sat": sat, "real": str(count[0]), "imag": str(count[1]),
            "peak_rss_kib": rss, "native_stdout_sha256": sha256(result.stdout.encode()).hexdigest(),
            "native_stdout_bytes": len(result.stdout.encode("utf8")),
        }
        if tool == "rawmocs_mpfr":
            phase = re.search(
                r"^RESEARCH_MPFR_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) "
                r"output=(\d+) nodes=(\d+) cache_hits=(\d+) decompositions=(\d+) "
                r"digits=(\d+) bits=(\d+)$",
                result.stderr, flags=re.MULTILINE,
            )
            if phase:
                numbers = [int(v) for v in phase.groups()]
                record["mpfr_phase_ns"] = dict(zip(
                    ("read", "parse", "solve", "output"), numbers[:4]
                ))
                record["mpfr_stats"] = dict(zip(
                    ("nodes", "cache_hits", "decompositions"), numbers[4:7]
                ))
                record["mpfr_precision_digits"] = numbers[7]
                record["mpfr_precision_bits"] = numbers[8]
            else:
                record["mpfr_profile_status"] = "UNAVAILABLE"
        if tool == "rawmocs":
            phase = re.search(
                r"^RESEARCH_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) output=(\d+) "
                r"nodes=(\d+) cache_hits=(\d+) decompositions=(\d+)$",
                result.stderr, flags=re.MULTILINE,
            )
            if phase:
                numbers = [int(value) for value in phase.groups()]
                record["rawmocs_phase_ns"] = dict(zip(
                    ("read", "parse", "solve", "output"), numbers[:4]
                ))
                record["rawmocs_stats"] = dict(zip(
                    ("nodes", "cache_hits", "decompositions"), numbers[4:]
                ))
            else:
                record["rawmocs_phase_status"] = "UNAVAILABLE"
        return record


def timed_execution(command: list[str], tool: str) -> dict[str, Any]:
    import time
    start = time.perf_counter_ns()
    record = guarded_execution(command, tool)
    if record["status"] == "COMPLETED":
        record["wall_ns"] = time.perf_counter_ns() - start
    return record


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--gpmc", type=Path, required=True)
    parser.add_argument("--rawmocs", type=Path, required=True)
    parser.add_argument("--trials", type=int, default=2)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not 1 <= args.trials <= 4:
        raise SystemExit("bounded trials 1-4")
    commit = subprocess.check_output(
        ["git", "-C", str(args.source), "rev-parse", "HEAD"], text=True
    ).strip()
    if commit != "0b28736651b3740ed3b201edd2efdef3da541e0d":
        raise SystemExit("original QuPRS source is not pinned")
    report = {
        "source_commit": commit,
        "cohort": "research-created 2-4q GHZ/graph/BV/QFT/Grover/QAOA and 2 original Feynman source QASMs",
        "source_method": "unchanged Rust QuPRS PathSum / WmcManager",
        "no_quantum_certificate": True,
        "numeric_contract": "exact printed decimal weights versus MPFR30 approximate output",
        "resource_limit": {"child_virtual_memory_bytes": 2147483648, "child_wall_seconds": 8},
        "rows": [],
    }
    for case in CASES:
        input_path = args.corpus / f"{case}-gpmc.cnf"
        other_path = args.corpus / f"{case}-ganak.cnf"
        qasm = args.corpus / f"{case}.qasm"
        sidecar = args.corpus / f"{case}-gpmc-phases.tsv"
        sibling_sidecar = args.corpus / f"{case}-ganak-phases.tsv"
        paths = [input_path, other_path, qasm, sidecar, sibling_sidecar]
        row: dict[str, Any] = {"case": case, "source": "QuPRS original WmcManager"}
        if any(not path.is_file() for path in paths):
            row["status"] = "SOURCE_NOT_CAPTURED"
            row["missing_files"] = [p.name for p in paths if not p.is_file()]
            report["rows"].append(row)
            print(f"RESEARCH_HOLDOUT_CENSORED case={case} reason=SOURCE_NOT_CAPTURED")
            continue
        if case in ("feynman_mod5_4", "feynman_qft_4"):
            original = "mod5_4.qasm" if case == "feynman_mod5_4" else "qft_4.qasm"
            original_bytes = (args.source / "benchmarks/Feynman" / original).read_bytes()
            if qasm.read_bytes() != original_bytes:
                raise SystemExit(f"original unmodified Feynman QASM drift: {case}")
        gpmc = parse_cnf(input_path, "gpmc")
        ganak = parse_cnf(other_path, "ganak")
        if (gpmc["variables"], gpmc["clauses"], gpmc["weights"]) != (
            ganak["variables"], ganak["clauses"], ganak["weights"]
        ):
            raise SystemExit(f"source dialect mismatch for {case}")
        if sidecar.read_bytes() != sibling_sidecar.read_bytes():
            raise SystemExit(f"source phase mismatch for {case}")
        ids = {int(line.split("\t")[0]) for line in sidecar.read_text().splitlines()[1:]}
        weighted_vars = {literal for literal in gpmc["weights"] if literal > 0}
        if ids != weighted_vars:
            raise SystemExit(f"source phase IDs unmatched: {case}")
        n = gpmc["variables"]
        m = gpmc["clauses_count"]
        row.update({
            "cnf_sha256": sha256(input_path.read_bytes()).hexdigest(),
            "qasm_sha256": sha256(qasm.read_bytes()).hexdigest(),
            "phase_sha256": sha256(sidecar.read_bytes()).hexdigest(),
            "variables": n, "clauses": m, "source_phase_count": len(ids),
            "exact_printed_oracle_available": n <= 17,
            "source_kind": "pinned_Feynman_QASM" if case.startswith("feynman_") else "defined_research_motif",
        })
        if n > MAX_SOLVER_CNF_VARS or m > MAX_SOLVER_CNF_CLAUSES or input_path.stat().st_size > MAX_SOURCE_CNF_BYTES:
            row["status"] = "RESOURCE_SCOPE_REJECTED"
            report["rows"].append(row)
            print(f"RESEARCH_HOLDOUT_CENSORED case={case} reason=RESOURCE_SCOPE_REJECTED")
            continue
        oracle = None
        if n <= 17:
            n_check, clauses, weights = exact_printed_cnf(input_path)
            oracle = independent_wmc(n_check, clauses, weights)
            row["oracle_sat"] = oracle[0]
            row["oracle_real"] = str(oracle[1][0])
            row["oracle_imag"] = str(oracle[1][1])
        samples: list[dict] = []
        commands = {
            "gpmc": [
                str(args.gpmc.resolve()), "-mode=1", "-prec=30", str(input_path.resolve())
            ],
            "rawmocs": [str(args.rawmocs.resolve()), str(input_path.resolve())],
        }
        for trial in range(args.trials):
            order = ("gpmc", "rawmocs") if trial % 2 == 0 else ("rawmocs", "gpmc")
            for tool in order:
                outcome = timed_execution(commands[tool], tool)
                outcome["trial"] = trial
                samples.append(outcome)
        row["samples"] = samples
        completed = {tool: [s for s in samples if s["tool"] == tool and s["status"] == "COMPLETED"] for tool in commands}
        if any(len(records) != args.trials for records in completed.values()):
            row["status"] = "CENSORED_SOLVER_FAILURE"
        else:
            left = completed["gpmc"][0]
            right = completed["rawmocs"][0]
            agreement = left["sat"] == right["sat"]
            for side in ("real", "imag"):
                a = Fraction(left[side])
                b = Fraction(right[side])
                tolerance = max(ABS_DECIMAL_GUARD, RELATIVE_DECIMAL_GUARD * max(abs(a), abs(b)))
                if abs(a - b) > tolerance:
                    agreement = False
            for record in samples:
                if record["status"] != "COMPLETED":
                    agreement = False
                    continue
                # No trial may silently disagree with another trial from
                # its OWN solver. Never use a second-trial result as a valid
                # performance sample if exact printed observations differ.
                own = completed[record["tool"]][0]
                if record["sat"] != own["sat"] or (
                    Fraction(record["real"]), Fraction(record["imag"])
                ) != (
                    Fraction(own["real"]), Fraction(own["imag"])
                ):
                    agreement = False
                if oracle:
                    valid_tolerance = Fraction(0) if record["tool"] == "rawmocs" else max(
                        ABS_DECIMAL_GUARD,
                        RELATIVE_DECIMAL_GUARD * max(abs(v) for v in oracle[1]),
                    )
                    if record["sat"] != oracle[0] or any(
                        abs(Fraction(record[side]) - expected) > valid_tolerance
                        for side, expected in zip(("real", "imag"), oracle[1])
                    ):
                        agreement = False
            row["status"] = (
                "ORACLE_VERIFIED_PAIRED" if agreement and oracle else
                "SOLVER_AGREEMENT_ONLY" if agreement else "NUMERICAL_OR_SAT_DISAGREEMENT"
            )
            if agreement:
                row["wall_ns_median"] = {
                    tool: statistics.median(s["wall_ns"] for s in records)
                    for tool, records in completed.items()
                }
                row["peak_rss_kib_median"] = {
                    tool: statistics.median(s["peak_rss_kib"] for s in records)
                    for tool, records in completed.items()
                }
                profiled = [
                    sample["rawmocs_phase_ns"]
                    for sample in completed["rawmocs"]
                    if "rawmocs_phase_ns" in sample
                ]
                if len(profiled) == args.trials:
                    row["rawmocs_phase_ns_median"] = {
                        phase: statistics.median(record[phase] for record in profiled)
                        for phase in ("read", "parse", "solve", "output")
                    }
                else:
                    row["rawmocs_phase_status"] = "INCOMPLETE"
        print(f"RESEARCH_HOLDOUT_CASE case={case} vars={n} clauses={m} status={row['status']}")
        report["rows"].append(row)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    captured = sum(r.get("variables") is not None for r in report["rows"])
    success = sum(r.get("status") in ("ORACLE_VERIFIED_PAIRED", "SOLVER_AGREEMENT_ONLY") for r in report["rows"])
    print(f"RESEARCH_HOLDOUT_COMPLETE attempted={len(CASES)} captured={captured} paired={success}")
    if captured < 3:
        raise SystemExit("not enough real representative native QuPRS emitted CNFs")


if __name__ == "__main__":
    main()
