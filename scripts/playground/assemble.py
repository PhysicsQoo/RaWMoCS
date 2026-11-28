"""Stage only the exact scoped RaWMoCS distribution, never a comparator."""
import hashlib,json,shutil,sys,zipfile
from pathlib import Path
W=Path(sys.argv[1]);S=W/"site";P=W/"prior"
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(W/"solver.zip")=="3906df07dbfc46ee89a68270ec90521884beb0d98539c90e4089a8b7d5c7adb4"
with zipfile.ZipFile(W/"solver.zip") as z:
    names=z.namelist()
    assert len(names)==len(set(names))
    for n in names:assert not Path(n).is_absolute() and ".." not in Path(n).parts
    z.extractall(P)
receipts=[p for p in P.rglob("receipt.json") if json.loads(p.read_text()).get("status")=="W1C_SCOPED_DISTRIBUTION_CANDIDATE_PASS"]
assert len(receipts)==1
B=receipts[0].parent;r=json.loads(receipts[0].read_text())
shutil.copytree(Path(__file__).resolve().parents[2]/"playground",S)
assert set(r["distribution_sha256"])=={"rawmocs.js","rawmocs.wasm","worker.js"}
for n,h in r["distribution_sha256"].items():
    p=B/"distribution"/n;assert sha(p)==h,n;shutil.copy2(p,S/n)
assert sha(B/"source/rawmocs-0.4.0.crate")=="d3e9e6ccd4a146bac19f6bd2628038cf20ae331dd12f9dc0cbcd37a4e63e955d"
for n,h in r["source_sha256"].items():
    p=Path(n);assert not p.is_absolute() and ".." not in p.parts
    assert sha(B/"source"/p)==h,n
manifest=dict(solver="RaWMoCS",version="0.4.0",status="scoped distribution candidate",artifact_id=15259949283,distribution_sha256=r["distribution_sha256"],crate_sha256=sha(B/"source/rawmocs-0.4.0.crate"),license_inventory=r["license_inventory"],source_archive_status="retained in original qualification artifact; public redistribution not yet approved")
(S/"PROVENANCE.json").write_text(json.dumps(manifest,indent=2)+"\n")
(W/"receipt.json").write_text(json.dumps(dict(status="STANDALONE_STAGED_NOT_PUBLISHED",manifest=manifest),indent=2)+"\n")
print("STANDALONE_ARTIFACT_STAGE_PASS",flush=True)
