"""Materialize one publishable Rust package from the canonical source modules.
No solver arithmetic is changed. Internal source workspaces remain unpublished.
"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tomllib

root, stage = (Path(p).resolve() for p in sys.argv[1:])
assert not stage.exists()
stage.mkdir(parents=True)
(stage/"src").mkdir()
(stage/"tests").mkdir()
cli=root/"crates/rawmocs-cli"
core=root/"crates/rawmocs-core"
for d in (cli,core):
    assert tomllib.loads((d/"Cargo.toml").read_text())["package"]["publish"] is False
sources={}
def copy(src,dst,transform=lambda s:s):
    raw=src.read_bytes()
    content=transform(raw.decode()).encode()
    (stage/dst).write_bytes(content)
    sources[dst]={"source":str(src.relative_to(root)),
                  "source_sha256":hashlib.sha256(raw).hexdigest(),
                  "staged_sha256":hashlib.sha256(content).hexdigest()}
def native_root(s):
    assert s.count("mod general;")==1
    assert "rawmocs_core::" in s
    return s.replace("mod general;","mod general;\nmod reference_core;").replace("rawmocs_core::","crate::reference_core::")
copy(cli/"src/lib.rs","src/lib.rs",native_root)
copy(core/"src/lib.rs","src/reference_core.rs")
for p in ("general.rs","factor_prejoin.rs","main.rs"):
    copy(cli/"src"/p,"src/"+p,lambda s:s.replace("rawmocs_mpfr::","rawmocs::"))
for p in ("shared_core.rs","native_stdio.rs"):
    copy(cli/"tests"/p,"tests/"+p,lambda s:s.replace("rawmocs_mpfr::","rawmocs::"))
copy(root/"LICENSE","LICENSE")
copy(root/"packaging/rawmocs/Cargo.toml","Cargo.toml")
copy(root/"packaging/rawmocs/README.md","README.md")
assert not any("rawmocs_core::" in p.read_text() or "rawmocs_mpfr::" in p.read_text() for p in (stage/"src").glob("*.rs"))
meta=tomllib.loads((stage/"Cargo.toml").read_text())
assert meta["package"]["name"]=="rawmocs"
assert all(not isinstance(d,dict) or not ({"path","git"} & d.keys()) for d in meta["dependencies"].values())
print(json.dumps({"revision":subprocess.check_output(["git","-C",str(root),"rev-parse","HEAD"],text=True).strip(),"version":meta["package"]["version"],"sources":sources},indent=2))
