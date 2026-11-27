"""Per-family Python admission scout, preserving the original RSS/time limits."""
import json
import os
from pathlib import Path
import statistics

root = Path(".m1-python-evidence")
data = {}
for arm in ("baseline", "candidate"):
    d = root / arm
    s = json.loads((d / "repeat-summary.json").read_text())
    rows = json.loads((d / "repeat-python.json").read_text())
    assert s["status"] == "M0_REPEAT_WITNESS_PASS" and len(rows) == 12
    assert s["cnf_sha256"] == os.environ["M0_CNF_SHA"]
    assert s["witness_sha256"] == os.environ["M0_REF_SHA"]
    data[arm] = dict(signature=s["signature"], **s["summary"]["python"],
                     max_postsolve_rss_kib=max(r["rss_kib"] for r in rows),
                     median_postsolve_rss_kib=statistics.median(r["rss_kib"] for r in rows[2:]),
                     median_wall_ns=statistics.median(r["wall_ns"] for r in rows[2:]))
a, b = (data[k] for k in ("baseline", "candidate"))
assert a["signature"] == b["signature"]
checks = dict(raw_rss=b["raw_rss_growth_kib"] <= 32768,
              peak_hwm=b["max_cumulative_hwm_kib"] <= a["max_cumulative_hwm_kib"],
              median_wall=b["median_wall_ns"] <= 1.05*a["median_wall_ns"])
out = dict(status="M1_PYTHON_SCOUT_PASS" if all(checks.values()) else "M1_PYTHON_SCOUT_REVIEW",
           case=os.environ["CASE"], cnf_sha256=os.environ["M0_CNF_SHA"],
           witness_sha256=os.environ["M0_REF_SHA"], order=os.environ["ARM_ORDER"],
           checks=checks, data=data, scope="one paired 12-call cohort per arm; no promotion")
(root / "comparison.json").write_text(json.dumps(out, indent=2) + "\n")
print("M1_PYTHON_FINAL", json.dumps(out), flush=True)
assert all(checks.values()), "candidate resource/performance screen requires review"
