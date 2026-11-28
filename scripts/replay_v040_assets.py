"""Replay the exact publication-side asset checks without credentials or writes."""
import subprocess,sys
from pathlib import Path
from release_source import verify_assets,digest
from release_python_source import verify_python_assets
from release_v040 import CRATE,SOURCE,SDIST,V,verify_crate,files

mode,folder=sys.argv[1:]
p=Path(folder)
root=Path(__file__).resolve().parents[1]
rev=subprocess.check_output(["git","-C",str(root),"rev-parse","HEAD"],text=True).strip()
r={"version":V,"revision":rev}
if mode=="source":
    a={k:(p/"cli-source"/k).read_bytes() for k in (SOURCE,"SHA256SUMS","source-package.json")}
    r["source_sha256"]=digest(a[SOURCE])
    verify_assets(r,a)
    a.update({k:(p/k).read_bytes() for k in (SDIST,"source-inventory.json","package-candidate.json","rebuilt-package-candidate.json")})
    r["python_source_sha256"]=digest(a[SDIST])
    verify_python_assets(r,a)
    print("V040_PUBLISHER_SOURCE_ASSET_REPLAY_PASS")
elif mode=="crate":
    import json
    a={x.name:x.read_bytes() for x in p.iterdir() if x.is_file()}
    r["crate_sha256"]=digest(a[CRATE])
    receipt=json.loads(a["source-map.json"])
    canonical={m["source"]:(root/m["source"]).read_bytes() for m in receipt["sources"].values()}
    f=verify_crate(r,a,canonical)
    assert f["Cargo.lock"]==(root/"packaging/rawmocs/Cargo.lock").read_bytes()
    print("V040_PUBLISHER_CRATE_ASSET_REPLAY_PASS")
else:raise ValueError("unknown mode")
