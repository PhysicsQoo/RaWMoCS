"""Verify version parity and unchanged solver source against v0.3.0.
This is a bounded packaging release candidate, not publication authorization.
"""
from pathlib import Path
import hashlib,json,subprocess,tomllib
ROOT=Path(__file__).resolve().parents[1]
BASE="05e650f46d60aed9d4fa5e6147709d12f8c3f6a9"
V="0.4.0"
def load(p): return tomllib.loads((ROOT/p).read_text())
def git(*args):return subprocess.check_output(["git","-C",str(ROOT),*args])
pub=load("packaging/rawmocs/Cargo.toml")
cli=load("crates/rawmocs-cli/Cargo.toml")
py=load("crates/rawmocs-python/Cargo.toml")
core=load("crates/rawmocs-core/Cargo.toml")
assert pub["package"]["name"]=="rawmocs" and pub["package"]["publish"]==["crates-io"]
assert pub["lib"]["name"]==pub["bin"][0]["name"]=="rawmocs"
assert all(m["package"]["version"]==V for m in (pub,cli,py))
assert load("pyproject.toml")["project"]["version"]==V
assert all(m["package"]["publish"] is False for m in (cli,py,core))
assert py["dependencies"]["rawmocs_mpfr"]["version"]=="="+V
assert all(not isinstance(d,dict) or not ({"path","git"} & d.keys()) for d in pub["dependencies"].values())
for p,names in (("crates/rawmocs-cli/Cargo.lock",["rawmocs-cli"]),("crates/rawmocs-python/Cargo.lock",["rawmocs-cli","rawmocs-python"])):
    packages={x["name"]:x for x in load(p)["package"]}
    assert all(packages[n]["version"]==V for n in names)
# Compare canonical source directory trees, not a few sampled functions.
dirs=["crates/rawmocs-core/src","crates/rawmocs-cli/src","crates/rawmocs-python/src","python/rawmocs","python/tests"]
for d in dirs:
    assert git("rev-parse",BASE+":"+d)==git("rev-parse","HEAD:"+d),d
print(json.dumps({"status":"V040_RELEASE_VERSION_SCOPE_PASS","revision":git("rev-parse","HEAD").decode().strip(),"baseline":BASE,"version":V,"unchanged_source_trees":dirs,"publication_authorized":False},indent=2))
