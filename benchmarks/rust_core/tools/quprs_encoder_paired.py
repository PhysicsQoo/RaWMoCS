#!/usr/bin/env python3
"""QuPRS encoder comparison: real QuPRS encoder CNF, independent exhaustive WMC oracle,
matched native GPMC/RaWMoCS WallTime + RSS. Does NOT test quantum fidelity.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics

from quprs_paired_native import one_call

MAX_ORACLE_VARIABLES = 12
MPFR_ABSOLUTE_TOL = Fraction(1, 10**23)
MPFR_RELATIVE_TOL = Fraction(1, 10**21)


def exact_printed_cnf(path: Path) -> tuple[int, list[tuple[int, ...]], dict[int, tuple[Fraction, Fraction]]]:
    lines = path.read_text(encoding="utf8").splitlines()
    fields = lines[0].split()
    if fields[:2] != ["p", "cnf"] or len(fields) != 4 or lines[1].strip() != "c t wmc":
        raise ValueError("not source-weighted DIMACS")
    n, declared_clauses = int(fields[2]), int(fields[3])
    if not (1 <= n <= MAX_ORACLE_VARIABLES):
        raise ValueError("source CNF requires bounded truth-table")
    weights = {}
    clauses = []
    for line in lines[2:]:
        words = line.split()
        if not words:
            continue
        if words[:3] == ["c", "p", "weight"] and len(words) == 7 and words[-1] == "0":
            lit = int(words[3])
            if not (0 < abs(lit) <= n) or lit in weights:
                raise ValueError("duplicate/out-of-bounds weight")
            weights[lit] = (Fraction(words[4]), Fraction(words[5]))
        else:
            if words[-1] != "0":
                raise ValueError("missing clause terminal")
            clause = tuple(int(t) for t in words[:-1])
            if any(not (0 < abs(lit) <= n) for lit in clause):
                raise ValueError("invalid source literal")
            clauses.append(clause)
    if len(clauses) != declared_clauses:
        raise ValueError("source clause mismatch")
    return n, clauses, weights


def independent_wmc(n: int, clauses: list[tuple[int, ...]], weights: dict[int, tuple[Fraction, Fraction]]
                    ) -> tuple[bool, tuple[Fraction, Fraction]]:
    is_sat = False
    real, imag = Fraction(0), Fraction(0)
    for mask in range(1 << n):
        def lit_true(lit: int) -> bool:
            selected = bool(mask & (1 << (abs(lit) - 1)))
            return selected if lit > 0 else not selected
        if not all(any(lit_true(lit) for lit in clause) for clause in clauses):
            continue
        is_sat = True
        acc_real, acc_imag = Fraction(1), Fraction(0)
        for v in range(1, n + 1):
            lit = v if mask & (1 << (v - 1)) else -v
            wr, wi = weights.get(lit, (Fraction(1), Fraction(0)))
            acc_real, acc_imag = acc_real * wr - acc_imag * wi, acc_real * wi + acc_imag * wr
        real += acc_real
        imag += acc_imag
    return is_sat, (real, imag)


def measure(args: argparse.Namespace) -> dict:
    manifest = json.loads(args.manifest.read_text(encoding="utf8"))
    if manifest["dataset"] != "QU_PRS_ORIGINAL_RUST_PATHSUM_WMCMANAGER_EMITTED" or len(manifest["cases"]) != 6:
        raise ValueError("incorrect original encoder source manifest")
    rows = []
    for case in manifest["cases"]:
        name = case["case"]
        path = args.corpus / case["gpmc_file"]
        if sha256(path.read_bytes()).hexdigest() != case["gpmc_cnf_sha256"]:
            raise ValueError(f"{name}: source byte digest drift")
        n, clauses, weights = exact_printed_cnf(path)
        oracle_sat, expected = independent_wmc(n, clauses, weights)
        trials = []
        for index in range(args.trials):
            for tool in ("gpmc", "rawmocs") if index % 2 == 0 else ("rawmocs", "gpmc"):
                command = [str(args.gpmc.resolve()), "-mode=1", "-prec=30", str(path.resolve())] if tool == "gpmc" else [str(args.rawmocs.resolve()), str(path.resolve())]
                sat, actual, elapsed, peak_rss = one_call(command, tool)
                limit = max(MPFR_ABSOLUTE_TOL, MPFR_RELATIVE_TOL * max([abs(v) for v in expected] + [Fraction(1)])) if tool == "gpmc" else Fraction(0)
                if sat != oracle_sat or any(abs(a - b) > limit for a, b in zip(actual, expected)):
                    raise ValueError(f"{name}: {tool} not same printed Full WMC: SAT={sat} expected_sat={oracle_sat}, observed={actual} expected={expected}, tolerance={limit}")
                trials.append({
                    "index": index, "tool": tool, "wall_ns": elapsed, "rss_kib": peak_rss,
                    "observed_real": str(actual[0]), "observed_imag": str(actual[1]),
                    "numeric_tolerance": str(limit),
                })
        rows.append({
            "case": name, "cnf_sha256": case["gpmc_cnf_sha256"],
            "variables": n, "clauses": len(clauses), "sat": oracle_sat,
            "oracle_real": str(expected[0]), "oracle_imag": str(expected[1]),
            "samples": trials,
            "wall_ns_median": {tool: statistics.median(t["wall_ns"] for t in trials if t["tool"] == tool) for tool in ("gpmc", "rawmocs")},
            "peak_rss_kib_median": {tool: statistics.median(t["rss_kib"] for t in trials if t["tool"] == tool) for tool in ("gpmc", "rawmocs")},
        })
        print(f"QU_PRS_REAL_ENCODER_PAIRED_PASS case={name} vars={n} clauses={len(clauses)} samples={len(trials)}")
    return {
        "provenance": manifest,
        "oracle": "Python independent exhaustive truth table over exact finite GPMC decimal weight tokens",
        "numeric_contract": "GPMC MPFR30 bounded decimal observation vs Rust exact printed rational; no source-phase fidelity assertion",
        "scope": "original 6 one-qubit QuPRS miter encoder cases, not representative multi-qubit performance",
        "timed_boundary": "same Linux runner subprocess launch + native CLI process + weighted CNF parse/count/output",
        "rows": rows,
    }


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--corpus", type=Path, required=True)
    p.add_argument("--manifest", type=Path, required=True)
    p.add_argument("--gpmc", type=Path, required=True)
    p.add_argument("--rawmocs", type=Path, required=True)
    p.add_argument("--trials", type=int, default=3)
    p.add_argument("--report", type=Path, required=True)
    args = p.parse_args()
    if not 1 <= args.trials <= 5:
        raise SystemExit("trials need 1..5")
    result = measure(args)
    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf8")
    print(f"QU_PRS_SOURCE_EMITTED_WMC_PARITY_COMPLETE cases={len(result['rows'])} samples={sum(len(r['samples']) for r in result['rows'])}")


if __name__ == "__main__":
    main()
