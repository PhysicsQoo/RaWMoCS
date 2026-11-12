#!/usr/bin/env python3
"""Fail-closed paired exact Full WMC and SAT control for branch arithmetic.

Original and candidate release binaries are built on one runner from the same
example source with different pinned rawmocs-core implementations.
Native wall times are descriptive; no statistical speedup is certified.
"""
import csv
import statistics
import sys
from pathlib import Path

HEADER = [
    "variant", "round", "family", "blocks", "cache_cap", "wall_ns",
    "satisfiable", "count_real", "count_imag", "nodes", "cache_entries",
    "cache_hits", "decompositions",
]
FAMILIES = ("one", "three", "cancel", "unsat")
BLOCKS = (8, 16, 32)
CAPS = (0, 10000)
VARIANTS = ("baseline", "candidate")
ROUNDS = (0, 1, 2)
EXACT = (
    "satisfiable", "count_real", "count_imag", "nodes",
    "cache_entries", "cache_hits", "decompositions",
)


def main(path: Path) -> None:
    results = {}
    with path.open(newline="", encoding="utf-8") as stream:
        rows = csv.DictReader(stream)
        if rows.fieldnames != HEADER:
            raise ValueError(f"unrecognized record fields: {rows.fieldnames}")
        for row in rows:
            key = (
                row["variant"], int(row["round"]), row["family"],
                int(row["blocks"]), int(row["cache_cap"]),
            )
            if (
                key[0] not in VARIANTS or key[1] not in ROUNDS
                or key[2] not in FAMILIES or key[3] not in BLOCKS
                or key[4] not in CAPS or key in results
            ):
                raise ValueError(f"duplicate or invalid fixture: {key}")
            if row["satisfiable"] not in ("true", "false"):
                raise ValueError(f"unknown SAT flag: {key}")
            if row["satisfiable"] != ("false" if key[2] == "unsat" else "true"):
                raise ValueError(f"SAT control violated: {key}")
            if key[2] in ("unsat", "cancel") and (
                row["count_real"] != "0" or row["count_imag"] != "0"
            ):
                raise ValueError(f"SAT-zero or UNSAT count witness violated: {key}")
            for field in ("nodes", "cache_entries", "cache_hits", "decompositions"):
                if int(row[field]) < 0:
                    raise ValueError(f"negative counter: {key}/{field}")
            if int(row["nodes"]) <= 0 or int(row["wall_ns"]) <= 0:
                raise ValueError(f"invalid solve observation: {key}")
            if key[4] == 0 and (
                int(row["cache_entries"]) or int(row["cache_hits"])
            ):
                raise ValueError(f"cache OFF control failed: {key}")
            results[key] = row

    expected = {
        (variant, round_index, family, blocks, cap)
        for variant in VARIANTS
        for round_index in ROUNDS
        for family in FAMILIES
        for blocks in BLOCKS
        for cap in CAPS
    }
    if set(results) != expected:
        raise ValueError(f"incomplete cohort: {len(expected - set(results))} missing")

    for round_index in ROUNDS:
        for family in FAMILIES:
            for blocks in BLOCKS:
                for cap in CAPS:
                    lhs = results[("baseline", round_index, family, blocks, cap)]
                    rhs = results[("candidate", round_index, family, blocks, cap)]
                    for field in EXACT:
                        if lhs[field] != rhs[field]:
                            raise ValueError(
                                f"exact baseline/candidate mismatch "
                                f"{(round_index, family, blocks, cap)}/{field}"
                            )

    for family in FAMILIES:
        for blocks in BLOCKS:
            for cap in CAPS:
                value = {}
                for variant in VARIANTS:
                    value[variant] = statistics.median(
                        int(results[(variant, r, family, blocks, cap)]["wall_ns"])
                        for r in ROUNDS
                    )
                baseline, candidate = value["baseline"], value["candidate"]
                print(
                    f"family={family} blocks={blocks} cap={cap} "
                    f"baseline_median_ns={baseline} candidate_median_ns={candidate} "
                    f"baseline_over_candidate={baseline/candidate:.4f}"
                )
    print("PAIRED_BRANCH_EXACT_SAT_WMC_SEARCH_PARITY=PASS")
    print("PAIRED_BRANCH_STATISTICAL_SPEEDUP=NOT_CLAIMED")
    print("PAIRED_BRANCH_HARD_RSS_CAP=NOT_CLAIMED")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: paired_branch_sat.py CSV")
    try:
        main(Path(sys.argv[1]))
    except (ValueError, OSError, ZeroDivisionError) as exc:
        raise SystemExit(f"PAIRED_BRANCH_VALIDATION=FAIL: {exc}") from exc
