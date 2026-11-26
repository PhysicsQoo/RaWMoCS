#!/usr/bin/env python3
"""Independent QFT frontier/order scout. No production changes or cap increases."""
import argparse
from fractions import Fraction as Q
from hashlib import sha256
from itertools import combinations, product
import json
from pathlib import Path
import random
from time import monotonic

import original_exact_prejoin as oracle
from original_fixed_interval import BITS, S, ZERO as I_ZERO, add as iadd


def indices(mask):
    while mask:
        bit = mask & -mask
        yield bit.bit_length() - 1
        mask ^= bit


def search(raw, expected, local_width):
    start = monotonic()
    report = dict(status="IN_PROGRESS", source_sha256=expected, local_width=local_width,
                  bits=BITS, caps=dict(width=oracle.MAX_WIDTH, work=oracle.MAX_WORK,
                                       live=oracle.MAX_LIVE), rows=[])
    try:
        fs, nodes, scalar, local = oracle.local_reduce(
            raw, expected, start + 30, diagnostic=True,
            local_width=local_width, incremental=True)
    except ValueError as exc:
        if str(exc) not in ("LOCAL_DEADLINE", "LOCAL_RESOURCE", "LOCAL_EXACT_BITS", "LOCAL_LIVE"):
            raise
        report.update(status="LOCAL_REJECTED", reason=str(exc))
        return report
    adj, comps = oracle.residual_components(fs, nodes)
    report.update(local=local, residual=len(nodes), factors=len(fs),
                  components=sorted(map(len, comps), reverse=True))
    ids = sorted(nodes)
    index = {v: i for i, v in enumerate(ids)}
    masks = [sum(1 << index[u] for u in adj[v]) for v in ids]
    full = (1 << len(ids)) - 1
    high = sorted(nodes, key=lambda v: (-len(adj[v]), v))[:12]
    report["cut_pool"] = high
    deadline = monotonic() + 45

    def order(cuts, penalty, seed):
        active = full
        for v in cuts:
            active &= ~(1 << index[v])
        g = [m & active for m in masks]
        ranks = list(range(len(ids)))
        random.Random(seed).shuffle(ranks)
        width = cells = 0
        seq = []
        while active:
            if monotonic() >= deadline:
                return None
            def score(i):
                nb = g[i]
                degree = nb.bit_count()
                fill = 0 if penalty is None else sum(
                    (nb & ~g[j]).bit_count() - 1 for j in indices(nb)) // 2
                return (degree if penalty is None else fill + penalty * degree,
                        degree, ranks[i])
            i = min(indices(active), key=score)
            nb = g[i]
            width = max(width, nb.bit_count())
            cells += 1 << nb.bit_count()
            seq.append(ids[i])
            active &= ~(1 << i)
            for j in indices(nb):
                g[j] = (g[j] | nb) & ~((1 << i) | (1 << j))
            g[i] = 0
        return width, cells, tuple(cuts), tuple(seq)

    shortlist = []
    for n in range(min(4, len(high)) + 1):
        cuts = list(combinations(high, n))
        random.Random(2026 + n).shuffle(cuts)
        selected = cuts[:24]
        best = []
        trials = 0
        for cut in selected:
            if monotonic() >= deadline:
                break
            for penalty in (0, 1, 4, None):
                for seed in (0, 1, 2, 3):
                    x = order(cut, penalty, seed)
                    if x is None:
                        break
                    trials += 1
                    best.append(x)
                if monotonic() >= deadline:
                    break
        ranked = sorted(set(best))
        # Retain distinct cutsets to avoid repeated equivalent candidates.
        unique = {}
        for x in ranked:
            unique.setdefault(x[2], x)
        top = list(unique.values())[:4]
        shortlist.extend(x for x in top if x[0] <= oracle.MAX_WIDTH)
        report["rows"].append(dict(cuts=n, trials=trials, planned=len(selected)*16,
            best_width=ranked[0][0] if ranked else None,
            best_cells=ranked[0][1] if ranked else None,
            candidates=[dict(width=x[0], cells=x[1], cutset=x[2], order=x[3]) for x in top]))
        if monotonic() >= deadline:
            break
    plan_deadline = monotonic() + 30
    report["replay"] = []
    # Test lower conditioning count first, then estimated graph cost.
    for width, cells, cuts, seq in sorted(shortlist, key=lambda x: (len(x[2]),x[0],x[1],x[2],x[3])):
        if monotonic() >= plan_deadline:
            break
        row = dict(cutset=cuts, order=seq, graph_width=width, branches=0,
                   work=local["work"], peak=local["peak"], width=0)
        report["replay"].append(row)
        branches = []
        try:
            for values in product((False, True), repeat=len(cuts)):
                branch, sc = oracle.restrict(fs, scalar, cuts, values)
                p = oracle.plan(branch, nodes-set(cuts), (None, False),
                                local["retained_cells"], plan_deadline, fixed_order=seq)
                if p is None:
                    break
                row["branches"] += 1
                row["work"] += p["work"] + local["retained_cells"]
                row["peak"] = max(row["peak"], p["peak"])
                row["width"] = max(row["width"], p["width"])
                if row["work"] > oracle.MAX_WORK or row["peak"] > oracle.MAX_LIVE:
                    break
                branches.append((values,p))
        except ValueError as exc:
            if str(exc) != "PLAN_DEADLINE":
                raise
            row["status"] = "PLAN_DEADLINE"
            break
        if len(branches) != 1 << len(cuts):
            row["status"] = "BRANCH_OR_AGGREGATE_CAP_REJECTED"
            continue
        row["status"] = "FEASIBLE"
        report["selected"] = row
        # Keep structural proof even if interval execution is censored.
        report["status"] = "BOUNDED_PLAN_FOUND"
        execution_deadline = monotonic() + 90
        result = None
        try:
            for values, p in branches:
                branch, sc = oracle.restrict(fs, scalar, cuts, values)
                result = iadd(result, oracle.execute(branch, sc, p, execution_deadline))
        except ValueError as exc:
            if str(exc) not in ("JOIN_DEADLINE", "SUM_DEADLINE", "EXECUTE_DEADLINE"):
                raise
            report["execution_stop"] = str(exc)
            break
        sat = result is not None
        if result is None:
            result = I_ZERO
        report.update(status="INDEPENDENT_INTERVAL_COMPLETE", sat=sat,
            real=[str(Q(x, S)) for x in result[:2]],
            imag=[str(Q(x, S)) for x in result[2:]],
            width=row["width"], work=row["work"], peak=row["peak"])
        break
    if report["status"] == "IN_PROGRESS":
        report["status"] = "NO_ADMITTED_OBSERVED_PLAN"
    report["wall_s"] = monotonic() - start
    return report


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cnf", type=Path)
    ap.add_argument("sha256")
    ap.add_argument("--local-width", type=int, choices=(4,5,6), required=True)
    a = ap.parse_args()
    raw = a.cnf.read_bytes()
    assert sha256(raw).hexdigest() == a.sha256
    print(json.dumps(search(raw, a.sha256, a.local_width)), flush=True)
