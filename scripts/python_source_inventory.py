"""Inventory source distribution and locked dependency licenses, not legal clearance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
from python_package_candidate import ROOT, identity, audit_sdist, require

def inventory(path, metadata):
    ident = identity()
    receipt = audit_sdist(path, ident)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    require(revision == os.environ.get("GITHUB_SHA"), "source revision mismatch")
    prefix = "rawmocs-" + ident["version"] + "/"
    tracked = set(subprocess.check_output(["git", "ls-tree", "-r", "--name-only", "-z", revision], cwd=ROOT).decode().split("\0"))
    files = []
    with tarfile.open(path, "r:gz") as tar:
        for member in tar.getmembers():
            if not member.isfile():
                continue
            name = member.name[len(prefix):]
            data = tar.extractfile(member).read()
            require(not name.lower().endswith((".so", ".a", ".dll", ".dylib", ".pyd", ".o", ".whl", ".exe", ".rlib")),
                    "linked or compiled member: " + name)
            require(not data.startswith((b"\x7fELF", b"MZ", b"!<arch>\n", bytes.fromhex("cffaedfe"), bytes.fromhex("feedfacf"))),
                    "binary member: " + name)
            if name != "PKG-INFO":
                require(name in tracked, "untracked sdist member: " + name)
                original = subprocess.check_output(["git", "show", revision + ":" + name], cwd=ROOT)
                require(original == data, "sdist member not identical to source: " + name)
            files.append({"path": name, "sha256": hashlib.sha256(data).hexdigest()})
    packages = metadata["packages"]
    dependencies = sorted([{"name": p["name"], "version": p["version"],
                            "source": p["source"], "license": p["license"],
                            "license_file": p["license_file"]}
                           for p in packages], key=lambda p: (p["name"], p["version"]))
    for name, version in (("rug", "1.30.0"), ("gmp-mpfr-sys", "1.7.1")):
        require(any(p["name"] == name and p["version"] == version and p["license"] for p in dependencies),
                "missing expected native dependency license: " + name)
    return {"status": "SOURCE_INVENTORY_PASS", "revision": revision,
            "sdist": receipt, "files": files, "dependency_license_inventory": dependencies,
            "distribution_authorized": False, "publication_authorized": False}

if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--sdist", type=Path, required=True)
    p.add_argument("--metadata", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args()
    result = inventory(a.sdist, json.loads(a.metadata.read_text()))
    a.out.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(result["status"], result["revision"], len(result["files"]))
