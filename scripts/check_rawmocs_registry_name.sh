#!/usr/bin/env bash
# Read only. A missing name is an observation, not a reservation.
set -euo pipefail
out="${1:?evidence directory required}"
mkdir -p "$out"
date -u +%FT%TZ > "$out/checked-at.txt"
for item in api index; do
  if [ "$item" = api ]; then url=https://crates.io/api/v1/crates/rawmocs; else url=https://index.crates.io/ra/wm/rawmocs; fi
  rc=0
  status="$(curl -sS -L --max-time 20 --retry 1 -A 'RaWMoCS registry name check (https://github.com/PhysicsQoo/RaWMoCS)' -o "$out/$item.body" -w '%{http_code}' "$url")" || rc=$?
  printf '%s\n' "$status" > "$out/$item.http"
  printf '%s\n' "$rc" > "$out/$item.exit"
done
python3 - "$out" <<'PY'
from pathlib import Path
import json,sys
p=Path(sys.argv[1])
a,i=(int((p/(k+".http")).read_text()) for k in ("api","index"))
ok=all((p/(k+".exit")).read_text().strip()=="0" for k in ("api","index"))
state="AVAILABLE_OBSERVED_NOT_RESERVED" if ok and a==i==404 else ("REGISTERED_OR_INDEXED" if 200 in (a,i) else "INCONCLUSIVE")
r={"name":"rawmocs","api_http":a,"index_http":i,"transport_ok":ok,"status":state}
(p/"registry-name.json").write_text(json.dumps(r,indent=2)+"\n")
print(json.dumps(r))
assert state=="AVAILABLE_OBSERVED_NOT_RESERVED",state
PY
