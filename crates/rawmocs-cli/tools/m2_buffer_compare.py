"""M2 exploratory three-arm resource screen; no promotion on one cohort."""
import json
import os
from pathlib import Path
import statistics

root = Path(".m1-python-evidence")
data = {}
for arm in ("baseline", "candidate", "m2"):
    d = root / arm
    s = json.loads((d / "repeat-summary.json").read_text())
    rows = json.loads((d / "repeat-python.json").read_text())
    assert s["status"] == "M0_REPEAT_WITNESS_PASS" and len(rows) == 12
    assert s["cnf_sha256"] == os.environ["M0_CNF_SHA"]
    assert s["witness_sha256"] == os.environ["M0_REF_SHA"]
    steady = rows[2:]
    data[arm] = dict(signature=s["signature"], **s["summary"]["python"],
                     max_postsolve_rss_kib=max(r["rss_kib"] for r in rows),
                     median_postsolve_rss_kib=statistics.median(r["rss_kib"] for r in steady),
                     median_free_bytes=statistics.median(r["glibc_free_bytes"] for r in steady),
                     median_live_proxy_bytes=statistics.median(r["glibc_live_proxy_bytes"] for r in steady),
                     median_wall_ns=statistics.median(r["wall_ns"] for r in steady))
a, b, c = (data[k] for k in ("baseline", "candidate", "m2"))
assert a["signature"] == b["signature"] == c["signature"]
def screens(x, y):
    return dict(raw_rss=y["raw_rss_growth_kib"] <= 32768,
                peak_hwm=y["max_cumulative_hwm_kib"] <= x["max_cumulative_hwm_kib"],
                median_wall=y["median_wall_ns"] <= 1.05*x["median_wall_ns"],
                absolute_max_postsolve=y["max_postsolve_rss_kib"] <= x["max_postsolve_rss_kib"],
                absolute_median_postsolve=y["median_postsolve_rss_kib"] <= x["median_postsolve_rss_kib"])
checks = dict(m1_vs_baseline=screens(a,b), m2_vs_baseline=screens(a,c), m2_vs_m1=screens(b,c))
passed = all(checks["m2_vs_baseline"].values()) and all(checks["m2_vs_m1"].values())
out = dict(status="M2_BUFFER_SCOUT_PASS" if passed else "M2_BUFFER_SCOUT_REVIEW",
           case=os.environ["CASE"], cnf_sha256=os.environ["M0_CNF_SHA"],
           witness_sha256=os.environ["M0_REF_SHA"], order=os.environ["ARM_ORDER"],
           data=data, checks=checks,
           scope="one fresh 12-call cohort per arm; no promotion; allocator unchanged")
(root / "m2-comparison.json").write_text(json.dumps(out, indent=2) + "\n")
print("M2_BUFFER_FINAL", json.dumps(out), flush=True)
# Scientific REVIEW is not infrastructure failure or release admission.
