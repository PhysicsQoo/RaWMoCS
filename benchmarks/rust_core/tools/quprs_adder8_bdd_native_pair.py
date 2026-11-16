#!/usr/bin/env python3
"""Original QuPRS post-RR Adder8: paired native-process algorithm research.

Algorithms consume the SAME pinned QuPRS post-RR PathSum semantics, but
different representations: ANF for BDD/bitset and weighted CNF for GPMC.
All process-launch / input parse / solve / formatting time is charged.
Source export is measured separately, NOT included in these solver timings.
No production router or GPMC implementation is changed.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
from quprs_paired_native import parse_out

QU_PRS_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
EXPECTED = {"sat": True, "real": Fraction(40960), "imag": Fraction(0)}
TOOLS = ("bdd", "gpmc", "bitset")


def digest(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def revision(repo: Path) -> str:
    return subprocess.check_output(
        ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
    ).strip()


def parse_tsv(stdout: str, label: str) -> tuple[bool, Fraction, Fraction]:
    lines = stdout.strip().splitlines()
    if len(lines) != 2:
        raise ValueError(f"{label}: expected two TSV rows, got {len(lines)}")
    keys = lines[0].split("\t")
    vals = lines[1].split("\t")
    if len(keys) != len(vals) or len(keys) != len(set(keys)):
        raise ValueError(f"{label}: malformed/duplicate TSV columns")
    record = dict(zip(keys, vals))
    expected_status = (
        "JOINT_BDD_COMPLETE" if label == "bdd"
        else "EXHAUSTIVE_ORIGINAL_ANF_COMPLETED"
    )
    if record.get("status") != expected_status:
        raise ValueError(f"{label}: unqualified outcome")
    if record.get("phase_terms") != "0" or record.get("variables") != "24":
        raise ValueError(f"{label}: not phase-free Adder8 24-var source")
    if record.get("sat") not in ("true", "false"):
        raise ValueError(f"{label}: missing explicit SAT status")
    return (
        record["sat"] == "true",
        Fraction(record["wc_real"]),
        Fraction(record["wc_imag"]),
    )


def run_native(cmd: list[str], name: str, timeout: int) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        rss = Path(tmp) / "peak-kib.txt"
        wrapped = [
            "/usr/bin/time", "-f", "%M", "-o", str(rss),
            "prlimit", "--as=2147483648", "--", *cmd
        ]
        start = time.perf_counter_ns()
        try:
            proc = subprocess.Popen(
                wrapped,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                start_new_session=True,
            )
        except OSError as exc:
            return {"arm": name, "status": "CENSORED_SPAWN", "message": str(exc)}
        try:
            out, err = proc.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.communicate()
            return {
                "arm": name, "status": "CENSORED_TIMEOUT",
                "wall_ns": time.perf_counter_ns() - start,
                "limit_seconds": timeout,
            }
        elapsed = time.perf_counter_ns() - start
        row = {
            "arm": name,
            "status": "COMPLETED" if proc.returncode == 0 else "CENSORED_EXIT",
            "wall_ns": elapsed,
            "exit_code": proc.returncode,
            "stderr_tail": err[-500:],
        }
        try:
            row["peak_rss_kib"] = int(rss.read_text().strip())
        except (OSError, ValueError):
            row["status"] = "CENSORED_RSS"
        if row["status"] == "COMPLETED":
            try:
                if name == "gpmc":
                    sat, (real, imag) = parse_out(out, "gpmc")
                else:
                    sat, real, imag = parse_tsv(out, name)
                row.update({
                    "sat": sat,
                    "real": str(real),
                    "imag": str(imag),
                })
                if (sat, real, imag) != (
                    EXPECTED["sat"], EXPECTED["real"], EXPECTED["imag"]
                ):
                    row["status"] = "CENSORED_WMC_MISMATCH"
            except (KeyError, ValueError, ZeroDivisionError) as exc:
                row["status"] = "CENSORED_OUTPUT"
                row["parse_message"] = str(exc)[:250]
        return row


def summary(rows: list[dict]) -> dict:
    def percentile(nums: list[int], quantile: float) -> float:
        ordered = sorted(nums)
        at = (len(ordered) - 1) * quantile
        lo = int(at)
        hi = min(lo + 1, len(ordered) - 1)
        return ordered[lo] + (ordered[hi] - ordered[lo]) * (at - lo)

    return {
        label: {
            "n": len(group := [x for x in rows if x["arm"] == label]),
            "wall_ms_p50": statistics.median(
                [r["wall_ns"] for r in group]
            ) / 1e6,
            "wall_ms_p90": percentile(
                [r["wall_ns"] for r in group], 0.9
            ) / 1e6,
            "rss_kib_p50": statistics.median(
                [r["peak_rss_kib"] for r in group]
            ),
            "rss_kib_p90": percentile(
                [r["peak_rss_kib"] for r in group], 0.9
            ),
        }
        for label in TOOLS
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--quprs", type=Path, required=True)
    parser.add_argument("--gpmc-repo", type=Path, required=True)
    parser.add_argument("--anf", type=Path, required=True)
    parser.add_argument("--cnf", type=Path, required=True)
    parser.add_argument("--bdd", type=Path, required=True)
    parser.add_argument("--bitset", type=Path, required=True)
    parser.add_argument("--gpmc", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--trials", type=int, default=7)
    parser.add_argument("--timeout", type=int, default=35)
    args = parser.parse_args()
    if not 3 <= args.trials <= 9 or not 2 <= args.timeout <= 90:
        raise SystemExit("requires 3..9 trials and 2..90s bounded timeout")
    if revision(args.quprs) != QU_PRS_SHA or revision(args.gpmc_repo) != GPMC_SHA:
        raise SystemExit("SOURCE_REVISION_DRIFT")
    if not args.anf.is_file() or not args.cnf.is_file():
        raise SystemExit("MISSING_PINNED_ORIGINAL_RR_SOURCE")
    if not args.anf.read_text().startswith("p anf 24 24 0\n"):
        raise SystemExit("WRONG_ORIGINAL_RR_ANF")
    if not args.cnf.read_text().startswith("c "):
        raise SystemExit("WRONG_ORIGINAL_RR_CNF")
    for file in (args.bdd, args.bitset, args.gpmc):
        if not file.is_file():
            raise SystemExit(f"MISSING_NATIVE_EXECUTABLE: {file}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw = []
    inputs = {
        "original_anf_sha256": digest(args.anf),
        "gpmc_cnf_sha256": digest(args.cnf),
        "quprs_revision": QU_PRS_SHA,
        "gpmc_revision": GPMC_SHA,
        "source_semantics": "same original post-RR PathSum, different representations",
        "precision": "phase-free integer complex WMC (exact), GPMC 30/100 decimal digits",
        "measurement": "fresh native process wall incl startup + input parse; common RR source preparation excluded and reported separately",
        "rss_limit": "GNU time observed peak KiB; prlimit virtual address space 2GiB; no hard cgroup RSS cap",
        "truth": {"sat": True, "real": "40960", "imag": "0"},
    }
    report = {
        "inputs": inputs,
        "trials_per_precision": args.trials,
        "records": raw,
        "cohorts": {},
        "status": "INCOMPLETE",
    }
    for digits in (30, 100):
        cohort = []
        for trial in range(args.trials):
            # Rotating order prevents a single arm from systematically
            # benefiting from warmed file-system/process caches.
            order = TOOLS[trial % 3 :] + TOOLS[: trial % 3]
            for name in order:
                cmd = {
                    "bdd": [str(args.bdd.resolve()), "--joint-bdd", str(args.anf.resolve())],
                    "bitset": [str(args.bitset.resolve()), str(args.anf.resolve())],
                    "gpmc": [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(args.cnf.resolve())],
                }[name]
                sample = run_native(cmd, name, args.timeout)
                sample.update({"digits": digits, "trial": trial})
                raw.append(sample)
                cohort.append(sample)
                print(
                    f"ADDER8_PAIRED arm={name} digits={digits} trial={trial} "
                    f"status={sample['status']} wall_ns={sample.get('wall_ns', 'NA')}",
                    flush=True,
                )
                # Persist failures before exiting, including censored runs.
                args.output.write_text(json.dumps(report, indent=2) + "\n")
                if sample["status"] != "COMPLETED":
                    report["cohorts"][str(digits)] = {
                        "status": "CENSORED",
                        "first_failure": sample,
                    }
                    args.output.write_text(json.dumps(report, indent=2) + "\n")
                    raise SystemExit("SOURCE_PAIRED_EXACT_ORACLE_OR_RESOURCE_FAILURE")
        report["cohorts"][str(digits)] = {
            "status": "EXACT_MATCHED_ALL_THREE",
            "statistics": summary(cohort),
        }
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    report["status"] = "EXACT_MATCHED_BOTH_PRECISIONS"
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    for digit, cohort in report["cohorts"].items():
        stats = cohort["statistics"]
        ratio = stats["gpmc"]["wall_ms_p50"] / stats["bdd"]["wall_ms_p50"]
        print(
            f"ADDER8_PAIRED_QUALIFIED digits={digit} "
            f"gpmc_p50_ms={stats['gpmc']['wall_ms_p50']:.4f} "
            f"bdd_p50_ms={stats['bdd']['wall_ms_p50']:.4f} "
            f"bitset_p50_ms={stats['bitset']['wall_ms_p50']:.4f} "
            f"gpmc_over_bdd={ratio:.4f}",
            flush=True,
        )


if __name__ == "__main__":
    main()
