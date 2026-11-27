"""Versioned v0.3.0 source-only release checks; never publish linked wheels."""
import hashlib
import io
import json
from pathlib import PurePosixPath
import re
import tarfile
import tomllib
from email.parser import BytesParser

PYTHON_JOBS = {
    "python-portability (ubuntu-24.04-arm)",
    "python-portability (macos-15)",
    "python-portability (macos-15-intel)",
    "python-manylinux (ubuntu-24.04, x86_64)",
    "python-manylinux (ubuntu-24.04-arm, aarch64)",
}
RESEARCH_SKIPS = {
    "Native memory research / M1 scout or manual M0 replay",
    "M2 matched Python wheels and contracts",
    "M2 factor buffer scout / ${{ matrix.case }}",
}
PYTHON_STEPS = {
    "core": {"Verify Python package integrity rejects malformed artifacts",
             "Verify release gate rejects incomplete science receipts"},
    "First release CLI and source-locked transfer gate": {
        "Prototype Python native wheel and API contract", "Archive Python packaging evidence"},
    **{name: {"Build and test native platform wheel" if "portability" in name
              else "Build manylinux candidate and inspect dependencies"} for name in PYTHON_JOBS},
}
SDIST = "rawmocs-0.3.0.tar.gz"
PYTHON_ASSETS = {SDIST, "source-inventory.json", "package-candidate.json",
                 "rebuilt-package-candidate.json"}

def require(ok, message):
    if not ok:
        raise ValueError(message)

def digest(data):
    return hashlib.sha256(data).hexdigest()

def archive_files(blob):
    result = {}
    with tarfile.open(fileobj=io.BytesIO(blob), mode="r:gz") as t:
        names = set()
        for m in t.getmembers():
            p = PurePosixPath(m.name)
            require(m.name not in names and not p.is_absolute() and ".." not in p.parts
                    and "\\" not in m.name and (m.isfile() or m.isdir())
                    and p.parts[0] == "rawmocs-0.3.0", "unsafe source member")
            names.add(m.name)
            if m.isfile():
                name = "/".join(p.parts[1:])
                require(name and name not in result, "duplicate source path")
                result[name] = t.extractfile(m).read()
    return result

def verify_python_assets(r, assets):
    require(PYTHON_ASSETS <= assets.keys(), "missing Python assets")
    blob = assets[SDIST]
    require(digest(blob) == r["python_source_sha256"], "Python sdist digest mismatch")
    source = archive_files(assets["rawmocs-0.3.0-source.tar.gz"])
    files = archive_files(blob)
    required = {"LICENSE", "PKG-INFO", "pyproject.toml", "crates/rawmocs-cli/Cargo.toml",
                "crates/rawmocs-python/Cargo.toml", "crates/rawmocs-cli/Cargo.lock",
                "crates/rawmocs-python/Cargo.lock", "python/rawmocs/__init__.py"}
    require(required <= set(files), "missing required Python source member")
    for name, data in files.items():
        require(not name.lower().endswith((".so", ".a", ".dll", ".dylib", ".pyd", ".o",
                                            ".whl", ".exe", ".rlib"))
                and not data.startswith((b"\x7fELF", b"MZ", b"!<arch>\n",
                                         bytes.fromhex("cffaedfe"), bytes.fromhex("feedfacf"))),
                "compiled member in sdist")
        if name != "PKG-INFO":
            require(source.get(name) == data, "Python source archive mismatch: " + name)
    project = tomllib.loads(files["pyproject.toml"].decode())["project"]
    binding = tomllib.loads(files["crates/rawmocs-python/Cargo.toml"].decode())
    cli = tomllib.loads(files["crates/rawmocs-cli/Cargo.toml"].decode())
    require(project["name"] == "rawmocs" and project["version"] == "0.3.0"
            and binding["package"]["version"] == cli["package"]["version"] == "0.3.0"
            and binding["dependencies"]["rawmocs_mpfr"]["version"] == "=0.3.0"
            and project["requires-python"] == ">=3.12,<3.13", "release identity mismatch")
    m = BytesParser().parsebytes(files["PKG-INFO"])
    require(m.get_all("Name") == ["rawmocs"] and m.get_all("Version") == ["0.3.0"]
            and len(m.get_all("Requires-Python", [])) == 1
            and set(m["Requires-Python"].replace(" ", "").split(",")) == {">=3.12", "<3.13"},
            "sdist metadata mismatch")
    inv = json.loads(assets["source-inventory.json"])
    require(inv["revision"] == r["revision"] and inv["status"] == "SOURCE_INVENTORY_PASS"
            and inv["sdist"] == {"filename": SDIST, "sha256": digest(blob)}
            and inv["distribution_authorized"] is False and inv["publication_authorized"] is False,
            "source inventory identity mismatch")
    rows = inv["files"]
    require(len(rows) == len(files) and {x["path"]: x["sha256"] for x in rows}
            == {name: digest(data) for name, data in files.items()}, "source inventory mismatch")
    deps = inv["dependency_license_inventory"]
    for name, version in (("rug", "1.30.0"), ("gmp-mpfr-sys", "1.7.1")):
        require(any(x["name"] == name and x["version"] == version and x["license"] for x in deps),
                "missing native license inventory")
    for name in ("package-candidate.json", "rebuilt-package-candidate.json"):
        report = json.loads(assets[name])
        identity = report["identity"]
        require(report["revision"] == r["revision"]
                and report["status"] == "PACKAGE_CANDIDATE_INTEGRITY_PASS"
                and report["publication_authorized"] is False and report["distribution_authorized"] is False
                and identity["version"] == identity["shared_cli_version"] == "0.3.0"
                and identity["python_lock_sha256"] == digest(files["crates/rawmocs-python/Cargo.lock"])
                and identity["cli_lock_sha256"] == digest(files["crates/rawmocs-cli/Cargo.lock"])
                and re.fullmatch(r"[0-9a-f]{64}", report["wheel"]["sha256"]),
                "package receipt mismatch")
        if name == "package-candidate.json":
            require(report["sdist"] == inv["sdist"], "sdist receipt mismatch")
