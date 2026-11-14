#!/usr/bin/env python3
"""Same-machine paired GPMC C++ CLI vs RaWMoCS Rust CLI (not CodSpeed).

Research-only, *test-defined* QuPRS CNFs. Per-sample native fork/exec + parse
+ preprocessing + Full WMC. No equivalence-circuit encoder or production swap.
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

NUM = r"[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?"
COMPLEX = re.compile(rf"^({NUM})([+-](?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)i$")
TOL = Fraction(1, 10**23)


def parse_out(stdout: str, tool: str) -> tuple[bool, tuple[Fraction, Fraction]]:
    tokens = re.findall(r"^s (SATISFIABLE|UNSATISFIABLE)\s*$", stdout, re.MULTILINE)
    if len(tokens) != 1:
        raise ValueError(f"{tool}: expected exactly one SAT status, found {tokens}")
    sat = tokens[0] == "SATISFIABLE"
    if tool == "rawmocs":
        observed = re.findall(r"^c r rational (\S+) (\S+)\s*$", stdout, re.MULTILINE)
        if len(observed) != 1:
            raise ValueError("RaWMoCS missing exact rational Full WMC output")
        return sat, tuple(Fraction(item) for item in observed[0])
    if tool == "rawmocs_mpfr":
        observed = re.findall(r"^c r mpfr (\\S+) (\\S+)\\s*$", stdout, re.MULTILINE)
        if len(observed) != 1:
            raise ValueError("research Rust MPFR output missing or ambiguous")
        return sat, tuple(Fraction(item) for item in observed[0])
    if not sat:
        return sat, (Fraction(0), Fraction(0))
    observed = re.findall(r"^c s exact double prec-sci (.+?)\\s*$", stdout, re.MULTILINE)
    if len(observed) != 1:
        raise ValueError("GPMC missing complex WMC output")
    match = COMPLEX.fullmatch(observed[0].strip())
    if match is None:
        raise ValueError(f"invalid GPMC complex output: {observed[0]}")
    return sat, (Fraction(match.group(1)), Fraction(match.group(2)))


def one_call(command: list[str], tool: str) -> tuple[bool, tuple[Fraction, Fraction], int, int]:
    # Both tools get EXACTLY the same GNU time wrapper. The wall-clock span
    # includes process startup, tool initialization, CNF parse, WMC, and output.
    with tempfile.TemporaryDirectory() as folder:
        rssfile = str(Path(folder) / "rss.txt")
        start = time.perf_counter_ns()
        result = subprocess.run(
            ["/usr/bin/time", "-f", "%M", "-o", rssfile, *command],
            capture_output=True, text=True, timeout=10, check=True,
        )
        elapsed = time.perf_counter_ns() - start
        rss_kib = int(Path(rssfile).read_text().strip())
    sat, count = parse_out(result.stdout, tool)
    return sat, count, elapsed, rss_kib


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--gpmc", type=Path, required=True)
    p.add_argument("--rawmocs", type=Path, required=True)
    p.add_argument("--corpus", type=Path, required=True)
    p.add_argument("--trials", type=int, default=3)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    if not 1 <= args.trials <= 5:
        raise SystemExit("trials must be 1..5")
    info = json.loads((args.corpus / "manifest.json").read_text())
    if info["source"] != "QuPRS test-defined direct CNF (NOT PathSum encoder-emitted)":
        raise SystemExit("wrong corpus source")
    rows = []
    for record in info["cases"]:
        cnf = (args.corpus / record["file"]).resolve()
        if sha256(cnf.read_bytes()).hexdigest() != record["sha256"]:
            raise SystemExit(f"{record['case']}: mismatched original source input SHA-256")
        expected_sat = record["satisfiable"]
        exact = (Fraction(record["expected_real"]), Fraction(record["expected_imag"]))
        samples = []
        for trial in range(args.trials):
            # Alternate tool order within same case, no selected best-case wins.
            tools = ("gpmc", "rawmocs") if trial % 2 == 0 else ("rawmocs", "gpmc")
            for tool in tools:
                command = (
                    [str(args.gpmc.resolve()), "-mode=1", "-prec=30", str(cnf)]
                    if tool == "gpmc"
                    else [str(args.rawmocs.resolve()), str(cnf)]
                )
                sat, count, wall, rss = one_call(command, tool)
                if sat != expected_sat or any(abs(a - b) > (TOL if tool == "gpmc" else 0) for a, b in zip(count, exact)):
                    raise SystemExit(f"{record['case']} {tool} failed independent oracle: SAT={sat}, count={count}, expected={exact}")
                samples.append({
                    "trial": trial, "tool": tool,
                    "wall_ns": wall, "peak_rss_kib": rss,
                    "count_real": str(count[0]), "count_imag": str(count[1]),
                })
        rows.append({
            "case": record["case"], "cnf_sha256": record["sha256"],
            "satisfiable": expected_sat, "expected_real": str(exact[0]), "expected_imag": str(exact[1]),
            "samples": samples,
            "median_wall_ns": {
                tool: statistics.median(s["wall_ns"] for s in samples if s["tool"] == tool)
                for tool in ("gpmc", "rawmocs")
            },
            "median_peak_rss_kib": {
                tool: statistics.median(s["peak_rss_kib"] for s in samples if s["tool"] == tool)
                for tool in ("gpmc", "rawmocs")
            },
        })
        print(f"PAIRED_CLI_PASS case={record['case']} trials={args.trials}")
    report = {
        "source": info, "samples_per_tool_per_case": args.trials,
        "n_samples": sum(len(r["samples"]) for r in rows),
        "timed_span": "Python fork/exec GNU time wrapper + executable startup/parse/preprocess/count/stdout",
        "numerical": "GPMC MPFR30 observed vs RaWMoCS exact printed-decimal rational",
        "warning": "Only six tiny test-defined CNFs; medians not evidence of general superiority or QuPRS source-encoder performance",
        "rows": rows,
    }
    args.out.write_text(json.dumps(report, indent=2) + "\n")
    print(f"PAIRED_NATIVE_CLI_COMPLETE samples={report['n_samples']} cases={len(rows)}")


if __name__ == "__main__":
    main()
