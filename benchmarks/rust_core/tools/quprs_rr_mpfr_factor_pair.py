#!/usr/bin/env python3
"""Same post-Reduction QuPRS residual, SAME source-printed MPFR weights.

This study compares two algorithmic representations (weighted Tseitin CNF
vs native ANF/XOR/phase factors), not identical CNF bytes. Both receive the
identical original PathSum and original phase values. A separate previous
study remains the identical-CNF comparison for GPMC vs DPLL.

Results are research-only with explicit UNKNOWN/censored records.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
import os
from pathlib import Path
import re
import signal
import statistics
import subprocess
import tempfile
import time

from quprs_paired_native import parse_out

QU_PRS = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC = "89bbc89d0de918e7a5385740355df55765039230"
CASES = (
    "feynman_hwb6",
    "feynman_qft4",
    "mqt_qnn_4",
    "mqt_grover_4",
    "mqt_vqe_4",
)
PRECISIONS = {30: 100, 100: 333}
SOURCE_PATHS = {
    "feynman_hwb6": "benchmarks/Feynman/hwb6.qasm",
    "feynman_qft4": "benchmarks/Feynman/qft_4.qasm",
    "mqt_qnn_4": "benchmarks/MQTBench/qnn_nativegates_ibm_qiskit_opt0_4.qasm",
    "mqt_grover_4": "benchmarks/MQTBench/grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm",
    "mqt_vqe_4": "benchmarks/MQTBench/vqe_nativegates_ibm_qiskit_opt0_4.qasm",
}


def digest(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def check_source(path: Path, sha: str) -> None:
    actual = subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual != sha:
        raise ValueError(f"pinned source drift at {path}: {actual} != {sha}")


def run_child(cmd: list[str], label: str, timeout: int) -> dict:
    """Full subprocess wall and process-observed peak RSS, fresh each call."""
    with tempfile.TemporaryDirectory() as folder:
        peak_file = Path(folder) / "peak-rss-kib.txt"
        command = [
            "/usr/bin/time", "-f", "%M", "-o", str(peak_file),
            "prlimit", "--as=2147483648", "--", *cmd,
        ]
        start = time.perf_counter_ns()
        try:
            # Kill the whole child group on timeout, not just GNU time;
            # a timed-out GPMC subprocess must not outlive its trial.
            proc = subprocess.Popen(
                command,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                start_new_session=True,
            )
        except OSError as exc:
            return {
                "status": "CENSORED_SPAWN_FAILURE",
                "label": label,
                "diagnostic": str(exc)[:250],
            }
        try:
            stdout, stderr = proc.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.communicate()
            return {
                "status": "CENSORED_TIMEOUT",
                "label": label,
                "timeout": timeout,
                "wall_ns": time.perf_counter_ns() - start,
            }
        elapsed = time.perf_counter_ns() - start
        result = {
            "label": label,
            "status": "COMPLETED" if proc.returncode == 0 else "CENSORED_NONZERO_EXIT",
            "return_code": proc.returncode,
            "wall_ns": elapsed,
            "stderr_tail": stderr[-700:],
        }
        if peak_file.exists():
            try:
                result["peak_rss_kib"] = int(peak_file.read_text().strip())
            except ValueError:
                result["status"] = "CENSORED_INVALID_RSS"
        else:
            result["status"] = "CENSORED_NO_RSS"
        if result["status"] == "COMPLETED" and label in {"gpmc", "native_factor"}:
            try:
                sat, value = parse_out(
                    stdout, "gpmc" if label == "gpmc" else "rawmocs_mpfr"
                )
                result.update({
                    "sat": sat, "real": str(value[0]), "imag": str(value[1]),
                })
            except (ValueError, ZeroDivisionError) as exc:
                result["status"] = "CENSORED_OUTPUT_PARSE"
                result["stderr_tail"] = str(exc)
        if label == "quprs_source":
            matched = re.search(
                r"RR_PRECISION_PAIR\s+case=(\S+)\s+bits=(\d+).*?"
                r"rr_ns=(\d+)\s+cnf_encode_ns=(\d+)\s+prepare_ns=(\d+)",
                stdout,
                re.DOTALL,
            )
            if result["status"] == "COMPLETED" and not matched:
                result["status"] = "CENSORED_SOURCE_PHASE_MISSING"
            if matched:
                result.update({
                    "quprs_rr_ns": int(matched.group(3)),
                    "cnf_encode_ns": int(matched.group(4)),
                    "prepare_ns": int(matched.group(5)),
                })
        return result


def near(a: Fraction, b: Fraction, digits: int) -> bool:
    # Observation guard, not a certified WMC numerical error interval.
    delta = Fraction(1, 10 ** (digits - 6))
    return abs(a - b) <= delta * max(Fraction(1), abs(a), abs(b))


def percentiles(values: list[int]) -> dict:
    if not values:
        return {}
    return {
        "p50": statistics.median(values),
        "p90": sorted(values)[-1] if len(values) < 5
        else statistics.quantiles(values, n=10, method="inclusive")[8],
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--quprs", type=Path, required=True)
    ap.add_argument("--gpmc-source", type=Path, required=True)
    ap.add_argument("--source-executable", type=Path, required=True)
    ap.add_argument("--gpmc", type=Path, required=True)
    ap.add_argument("--factor", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--report", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=3)
    args = ap.parse_args()
    if not 2 <= args.trials <= 9:
        raise SystemExit("requires 2..9 alternating fresh-process trials")
    check_source(args.quprs, QU_PRS)
    check_source(args.gpmc_source, GPMC)
    args.corpus.mkdir(parents=True, exist_ok=True)
    rows: list[dict] = []
    any_valid = False
    for case in CASES:
        source_qasm = args.quprs / SOURCE_PATHS[case]
        source_hash = digest(source_qasm)
        for digits, bits in PRECISIONS.items():
            header = {
                "case": case, "digits": digits, "bits": bits,
                "source_qasm_sha256": source_hash,
                "source_quprs_sha": QU_PRS, "source_gpmc_sha": GPMC,
            }
            cnf = args.corpus / f"{case}-{bits}-rr-gpmc.cnf"
            factor = args.corpus / f"{case}-{bits}-rr-factor.pf"
            prepared = []
            samples = []
            for trial in range(args.trials):
                # Source exporter is run as an independent fresh process
                # for each trial, and its wall is counted symmetrically
                # for both algorithms. It constructs BOTH representations:
                # conservative common preprocessing, not optimized E2E.
                source = run_child([
                    str(args.source_executable.resolve()), case,
                    str(args.quprs.resolve()), str(args.corpus.resolve()), str(bits),
                ], "quprs_source", 20)
                source["trial"] = trial
                prepared.append(source)
                if source["status"] != "COMPLETED":
                    break
                if not cnf.exists() or not factor.exists():
                    source["status"] = "CENSORED_MISSING_EMITTED_SOURCE"
                    break
                hashes = (digest(cnf), digest(factor))
                if trial and hashes != (header["rr_gpmc_sha256"], header["rr_factor_sha256"]):
                    raise SystemExit(f"{case}@{bits}: original residual export drift")
                header["rr_gpmc_sha256"], header["rr_factor_sha256"] = hashes

                # Same residual, same MPFR phase weights, same machine, order
                # rotated. The factor engine is not fed CNF, and its
                # per-variable weight semantics come from the source PathSum.
                order = ("gpmc", "native_factor") if trial % 2 == 0 else (
                    "native_factor", "gpmc"
                )
                for name in order:
                    command = (
                        [str(args.gpmc.resolve()), "-mode=1",
                         f"-prec={digits}", str(cnf.resolve())]
                        if name == "gpmc"
                        else [str(args.factor.resolve()), str(digits),
                              str(factor.resolve())]
                    )
                    sample = run_child(command, name, 20)
                    sample["trial"] = trial
                    samples.append(sample)
            header["source_preparation"] = prepared
            header["samples"] = samples
            if len(prepared) != args.trials or len(samples) != args.trials * 2 or any(
                s["status"] != "COMPLETED" for s in prepared + samples
            ):
                header["status"] = "CENSORED_INCOMPLETE"
            else:
                grouped = {
                    arm: [s for s in samples if s["label"] == arm]
                    for arm in ("gpmc", "native_factor")
                }
                ref = grouped["gpmc"][0]
                sat = ref["sat"]
                exact_ref = (Fraction(ref["real"]), Fraction(ref["imag"]))
                coherent = all(
                    row["sat"] == sat
                    and near(Fraction(row["real"]), exact_ref[0], digits)
                    and near(Fraction(row["imag"]), exact_ref[1], digits)
                    for row in samples
                )
                # Standalone exact source oracle from prior study:
                # only the original HWB6 post-RR model was exhaustively
                # checked. This GPMC comparison must reproduce 4+0i.
                if case == "feynman_hwb6":
                    coherent &= sat and near(exact_ref[0], Fraction(4), digits) and near(
                        exact_ref[1], Fraction(0), digits
                    )
                header["status"] = "MATCHED_SOURCE_MPFR_OBSERVATION" if coherent else (
                    "CENSORED_NUMERICAL_DISAGREEMENT"
                )
                if coherent:
                    any_valid = True
                    header["sat"] = sat
                    header["reference_count"] = list(map(str, exact_ref))
                    header["solver_wall_ns"] = {
                        a: percentiles([x["wall_ns"] for x in grouped[a]])
                        for a in grouped
                    }
                    header["observed_peak_rss_kib"] = {
                        a: percentiles([x["peak_rss_kib"] for x in grouped[a]])
                        for a in grouped
                    }
                    header["common_source_wall_ns"] = percentiles(
                        [x["wall_ns"] for x in prepared]
                    )
                    header["two_process_total_wall_ns"] = {
                        a: percentiles([
                            src["wall_ns"] + sample["wall_ns"]
                            for src, sample in zip(prepared, grouped[a])
                        ])
                        for a in grouped
                    }
            print(
                f"RR_ALGORITHM_PAIR case={case} digits={digits} bits={bits} "
                f"status={header['status']} "
                f"source_cnf={header.get('rr_gpmc_sha256', 'UNAVAILABLE')[:12]}",
                flush=True,
            )
            rows.append(header)
            args.report.write_text(json.dumps({
                "source": "Original QuPRS post-RR PathSum, same phase weights",
                "precision_contract": "MPFR digits 30/100 -> bits 100/333",
                "comparison": "different factor/CNF representations of same residual",
                "time_contract": "independent fresh subprocess wall incl startup and parse; common dual-format source preprocessing separately charged, not optimized E2E",
                "rss_contract": "Linux GNU time observed maxRSS KiB; RLIMIT_AS=2GiB, not hard cgroup memory",
                "completeness": "no speed ratios for CENSORED; source phase numeric agreement only, no mathematical arbitrary-angle certificate",
                "trials": args.trials, "rows": rows,
            }, indent=2) + "\n")
    if not any_valid:
        raise SystemExit("no source-qualified paired MPFR observations")
    # Fail closed for the small original independent exact source oracle.
    if not any(
        r["case"] == "feynman_hwb6"
        and r["status"] == "MATCHED_SOURCE_MPFR_OBSERVATION"
        for r in rows
    ):
        raise SystemExit("source-pinned HWB6 independent oracle regression")
    print("RR_ALGORITHM_PAIR_STUDY_COMPLETE", flush=True)


if __name__ == "__main__":
    main()
