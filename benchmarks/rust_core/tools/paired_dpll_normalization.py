#!/usr/bin/env python3
"""Fail-closed comparison of pinned baseline and candidate cycle-suite CSVs.

Each row is independently exact-oracled and process-isolated by cache_profile.
This analysis is descriptive: one shared CI runner is not a statistical speedup
certificate, and VmHWM is process peak RSS rather than a cache byte cap.
"""
import csv
import statistics
import sys
from pathlib import Path

HEADER = [
    "vertices", "variables", "trial", "policy", "solve_wall_ns",
    "process_peak_rss_kib", "nodes", "cache_entries", "cache_hits",
    "decompositions", "key_clauses", "key_literals",
]
SHAPES = {(v, t, p) for v in (6, 10, 14) for t in range(3) for p in ("off", "on")}
EXACT = ("variables", "nodes", "cache_entries", "cache_hits",
         "decompositions", "key_clauses", "key_literals")


def load(path: Path):
    with path.open(newline="", encoding="utf-8") as stream:
        reader = csv.DictReader(stream)
        if reader.fieldnames != HEADER:
            raise ValueError(f"unexpected CSV schema in {path.name}")
        rows = {}
        for row in reader:
            key = (int(row["vertices"]), int(row["trial"]), row["policy"])
            if key not in SHAPES or key in rows:
                raise ValueError(f"unexpected or repeated case in {path.name}: {key}")
            for column in HEADER:
                if column != "policy":
                    value = int(row[column])
                    if value < 0:
                        raise ValueError(f"negative counter in {path.name}: {column}")
                    row[column] = value
            if row["variables"] != key[0] + 1:
                raise ValueError(f"wrong width in {path.name}: {key}")
            if row["solve_wall_ns"] <= 0 or row["process_peak_rss_kib"] <= 0:
                raise ValueError(f"nonpositive observation in {path.name}: {key}")
            if row["nodes"] <= 0 or (key[2] == "off" and
                                    (row["cache_entries"] or row["cache_hits"])):
                raise ValueError(f"invalid cache control in {path.name}: {key}")
            rows[key] = row
        if set(rows) != SHAPES:
            raise ValueError(f"incomplete fixture cohort in {path.name}")
    return rows


def main(folder: Path):
    data = {}
    for round_index in range(3):
        baseline = load(folder / f"baseline-{round_index}.csv")
        candidate = load(folder / f"candidate-{round_index}.csv")
        for key in sorted(SHAPES):
            for column in EXACT:
                if baseline[key][column] != candidate[key][column]:
                    raise ValueError(
                        f"deterministic search mismatch at round {round_index} "
                        f"{key} {column}"
                    )
        data[("baseline", round_index)] = baseline
        data[("candidate", round_index)] = candidate

    with (folder / "summary.csv").open("w", newline="", encoding="utf-8") as stream:
        writer = csv.writer(stream)
        writer.writerow(["vertices", "policy", "baseline_median_ns",
                         "candidate_median_ns", "baseline_over_candidate",
                         "baseline_median_vm_hwm_kib", "candidate_median_vm_hwm_kib"])
        for vertices in (6, 10, 14):
            for policy in ("off", "on"):
                values = {}
                for kind in ("baseline", "candidate"):
                    entries = [
                        data[(kind, round_index)][(vertices, trial, policy)]
                        for round_index in range(3) for trial in range(3)
                    ]
                    values[kind] = (
                        statistics.median(r["solve_wall_ns"] for r in entries),
                        statistics.median(r["process_peak_rss_kib"] for r in entries),
                    )
                a, b = values["baseline"], values["candidate"]
                writer.writerow([vertices, policy, a[0], b[0],
                                 f"{a[0] / b[0]:.4f}", a[1], b[1]])
                print(
                    f"cycle={vertices} policy={policy}: "
                    f"median baseline={a[0]}ns candidate={b[0]}ns "
                    f"base/candidate={a[0] / b[0]:.4f}; "
                    f"VmHWM={a[1]}/{b[1]}KiB"
                )
    print("PAIRED_DPLL_EXACT_SEARCH_PARITY=PASS")
    print("PAIRED_DPLL_HARD_RSS_CAP=NOT_CLAIMED")
    print("PAIRED_DPLL_STATISTICAL_SPEEDUP=NOT_CLAIMED")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: paired_dpll_normalization.py ARTIFACT_DIR")
    try:
        main(Path(sys.argv[1]))
    except (ValueError, OSError, ZeroDivisionError) as exc:
        raise SystemExit(f"PAIRED_DPLL_VALIDATION=FAIL: {exc}") from exc
