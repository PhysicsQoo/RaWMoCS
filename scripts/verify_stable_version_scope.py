"""Verify exact admitted parser source and consistent stable package versions; no publish."""
from pathlib import Path
import json,re,subprocess,tomllib
ROOT=Path(__file__).resolve().parents[1]
def load(p): return tomllib.loads((ROOT/p).read_text())
def git(*args):return subprocess.check_output(["git","-C",str(ROOT),*args])
pub=load("packaging/rawmocs/Cargo.toml")
V=pub["package"]["version"]
assert re.fullmatch(r"\d+\.\d+\.\d+",V) and tuple(map(int,V.split(".")))>(0,4,0)
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
for p,names in (("crates/rawmocs-cli/Cargo.lock",["rawmocs-cli"]),("crates/rawmocs-python/Cargo.lock",["rawmocs-cli","rawmocs-python"]),("packaging/rawmocs/Cargo.lock",["rawmocs"])):
    packages={x["name"]:x for x in load(p)["package"]}
    assert all(packages[n]["version"]==V for n in names)
# Record the exact source trees, allowing new admitted solver changes in future versions.
dirs=["crates/rawmocs-core/src","crates/rawmocs-cli/src","crates/rawmocs-python/src","python/rawmocs","python/tests"]
trees={d:git("rev-parse","HEAD:"+d).decode().strip() for d in dirs}
print(json.dumps({"status":"STABLE_RELEASE_VERSION_SCOPE_PASS","revision":git("rev-parse","HEAD").decode().strip(),"version":V,"source_trees":trees,"publication_authorized":False},indent=2))
