"""Compare opt-in M1 diagnostic; never promote from one cohort."""
import json
from pathlib import Path
import re
import statistics

root = Path(".m0-evidence/m1")
data = {}
for arm in ("baseline", "candidate"):
    d = root / f"{arm}-evidence"
    summary = json.loads((d / "repeat-summary.json").read_text())
    rows = json.loads((d / "repeat-rust.json").read_text())
    assert len(rows) == 12
    profile = (d / "heaptrack-rust.txt").read_text()
    calls, = re.findall(r"^calls to allocation functions: (\d+) ", profile, re.M)
    peak, = re.findall(r"^peak heap memory consumption: (.+)$", profile, re.M)
    data[arm] = dict(signature=summary["signature"], **summary["summary"]["rust"],
                     allocation_calls=int(calls), profiled_peak_heap=peak,
                     median_wall_ns=statistics.median(r["wall_ns"] for r in rows[2:]))
a, b = (data[k] for k in ("baseline", "candidate"))
assert a["signature"] == b["signature"]
checks = dict(allocation_calls=b["allocation_calls"] < a["allocation_calls"],
              raw_rss=b["raw_rss_growth_kib"] <= 32768,
              peak_hwm=b["max_cumulative_hwm_kib"] <= a["max_cumulative_hwm_kib"],
              median_wall=b["median_wall_ns"] <= 1.05*a["median_wall_ns"])
result = dict(status="M1_SCOUT_PASS" if all(checks.values()) else "M1_SCOUT_REVIEW",
              data=data, checks=checks, scope="one Rust cohort; no promotion")
(root / "comparison.json").write_text(json.dumps(result, indent=2) + "\n")
print("M1_SCOUT_FINAL", json.dumps(result), flush=True)
# Negative results are scientific evidence, not an infrastructure failure.
