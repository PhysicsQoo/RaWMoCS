#!/usr/bin/env python3
"""Fail-closed analysis of predeclared, same-process Gaussian addition trials.

Native wall time includes allocation and process noise. This is descriptive
research, not a statistical speedup or end-to-end solver certification.
"""
import csv
import statistics
import sys
from pathlib import Path

SCENARIOS = (
    "left_zero", "right_zero", "both_zero", "dense",
    "left_zero_large", "dense_large",
)
IMPLEMENTATIONS = ("reference", "candidate")
ROUNDS = range(7)
ITERATIONS = 4096
HEADER = ["scenario", "round", "implementation", "iterations", "total_wall_ns"]


def main(path: Path) -> None:
    with path.open(newline="", encoding="utf-8") as stream:
        reader = csv.DictReader(stream)
        if reader.fieldnames != HEADER:
            raise ValueError(f"unexpected CSV schema: {reader.fieldnames}")
        rows: dict[tuple[str, int, str], int] = {}
        for row in reader:
            key = (row["scenario"], int(row["round"]), row["implementation"])
            if (
                key[0] not in SCENARIOS
                or key[1] not in ROUNDS
                or key[2] not in IMPLEMENTATIONS
                or key in rows
            ):
                raise ValueError(f"unknown or duplicate observation: {key}")
            if int(row["iterations"]) != ITERATIONS:
                raise ValueError(f"wrong fixed iteration count: {key}")
            elapsed = int(row["total_wall_ns"])
            if elapsed <= 0:
                raise ValueError(f"nonpositive duration: {key}")
            rows[key] = elapsed
    expected = {
        (scenario, round_index, impl)
        for scenario in SCENARIOS
        for round_index in ROUNDS
        for impl in IMPLEMENTATIONS
    }
    if set(rows) != expected:
        raise ValueError(f"incomplete paired cohort: missing {sorted(expected - set(rows))}")

    for scenario in SCENARIOS:
        ref = statistics.median(
            rows[(scenario, round_index, "reference")] / ITERATIONS
            for round_index in ROUNDS
        )
        candidate = statistics.median(
            rows[(scenario, round_index, "candidate")] / ITERATIONS
            for round_index in ROUNDS
        )
        print(
            f"scenario={scenario} reference_median_ns_per_op={ref:.2f} "
            f"candidate_median_ns_per_op={candidate:.2f} "
            f"reference_over_candidate={ref / candidate:.4f}"
        )
    print("PAIRED_GAUSSIAN_ARITHMETIC_ORACLE=PASS")
    print("PAIRED_GAUSSIAN_STATISTICAL_SPEEDUP=NOT_CLAIMED")
    print("PAIRED_GAUSSIAN_MEMORY_COST=NOT_MEASURED")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: paired_gaussian_add.py CSV_FILE")
    try:
        main(Path(sys.argv[1]))
    except (ValueError, OSError, ZeroDivisionError) as exc:
        raise SystemExit(f"PAIRED_GAUSSIAN_VALIDATION=FAIL: {exc}") from exc
