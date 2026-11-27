"""M1 idle-RSS reproduction; negative scientific results remain explicit."""
import json
import os
from pathlib import Path
import re
import statistics

ROOT = Path(".m1-python-evidence")
LIMIT = 32768
data = {}
signatures = []
for block in (0, 1):
    data[str(block)] = {}
    for arm in ("baseline", "candidate"):
        d = ROOT / f"block-{block}" / arm
        s = json.loads((d / "repeat-summary.json").read_text())
        rows = json.loads((d / "repeat-python.json").read_text())
        assert s["status"] == "M0_REPEAT_WITNESS_PASS" and len(rows) == 12
        assert s["cnf_sha256"] == os.environ["M0_CNF_SHA"]
        assert s["witness_sha256"] == os.environ["M0_REF_SHA"]
        signatures.append(s["signature"])
        steady = rows[2:]
        data[str(block)][arm] = dict(
            **s["summary"]["python"],
            max_postsolve_rss_kib=max(r["rss_kib"] for r in rows),
            median_postsolve_rss_kib=statistics.median(r["rss_kib"] for r in steady),
            median_postsolve_uss_kib=statistics.median(r["uss_kib"] for r in steady),
            median_free_bytes=statistics.median(r["glibc_free_bytes"] for r in steady),
            median_live_proxy_bytes=statistics.median(r["glibc_live_proxy_bytes"] for r in steady),
            median_wall_ns=statistics.median(r["wall_ns"] for r in steady))
assert all(s == signatures[0] for s in signatures)
checks = {}
for block, group in data.items():
    a, b = group["baseline"], group["candidate"]
    checks[block] = dict(
        raw_rss=b["raw_rss_growth_kib"] <= LIMIT,
        peak_hwm=b["max_cumulative_hwm_kib"] <= a["max_cumulative_hwm_kib"],
        median_wall=b["median_wall_ns"] <= 1.05*a["median_wall_ns"],
        absolute_max_postsolve=b["max_postsolve_rss_kib"] <= a["max_postsolve_rss_kib"],
        absolute_median_postsolve=b["median_postsolve_rss_kib"] <= a["median_postsolve_rss_kib"])
profiles = {}
for arm in ("baseline", "candidate"):
    d = ROOT / "block-1" / arm
    profile = (d / "heaptrack-python.txt").read_text()
    calls, = re.findall(r"^calls to allocation functions: (\d+) ", profile, re.M)
    peak, = re.findall(r"^peak heap memory consumption: (.+)$", profile, re.M)
    profiles[arm] = dict(allocation_calls=int(calls), profiled_peak_heap=peak)
passed = all(all(c.values()) for c in checks.values())
out = dict(status="M1_IDLE_SCREEN_PASS" if passed else "M1_IDLE_SCREEN_REVIEW",
           case=os.environ["CASE"], cnf_sha256=os.environ["M0_CNF_SHA"],
           witness_sha256=os.environ["M0_REF_SHA"], block0_order=os.environ["ARM_ORDER"],
           checks=checks, data=data, profiles=profiles, signature=signatures[0],
           scope="two fresh 12-call cohorts per arm; symbolized wheels; no trim or allocator tuning; not admission")
(ROOT / "idle-comparison.json").write_text(json.dumps(out, indent=2) + "\n")
print("M1_IDLE_FINAL", json.dumps(out), flush=True)
# Diagnostic verdict explicit; REVIEW is not a product pass.
