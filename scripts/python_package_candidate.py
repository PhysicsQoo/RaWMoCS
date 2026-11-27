"""Candidate packaging evidence only; never grants redistribution or publication."""
import argparse
import base64
import csv
from email.parser import BytesParser
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
PY_LOCK = "crates/rawmocs-python/Cargo.lock"
CLI_LOCK = "crates/rawmocs-cli/Cargo.lock"


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return tomllib.loads((ROOT / path).read_text())


def identity():
    project = load("pyproject.toml")["project"]
    binding = load("crates/rawmocs-python/Cargo.toml")
    cli = load("crates/rawmocs-cli/Cargo.toml")["package"]
    py_version = binding["package"]["version"].replace("-alpha.", "a")
    require(project["name"] == "rawmocs" and project["version"] == py_version,
            "Python/Rust binding version mismatch")
    require(binding["dependencies"]["rawmocs_mpfr"]["version"] == "=" + cli["version"],
            "binding must pin shared CLI version")
    for lock, expected in ((PY_LOCK, {"rawmocs-python": binding["package"]["version"],
                                     "rawmocs-cli": cli["version"]}),
                           (CLI_LOCK, {"rawmocs-cli": cli["version"]})):
        packages = {p["name"]: p for p in load(lock)["package"]}
        for name, version in expected.items():
            require(packages[name]["version"] == version, "lock version mismatch: " + name)
    return {"name": project["name"], "version": py_version,
            "requires_python": project["requires-python"],
            "shared_cli_version": cli["version"],
            "python_lock_sha256": sha((ROOT / PY_LOCK).read_bytes()),
            "cli_lock_sha256": sha((ROOT / CLI_LOCK).read_bytes())}


def safe_names(names):
    require(len(names) == len(set(names)), "duplicate archive member")
    for name in names:
        path = PurePosixPath(name)
        require(not path.is_absolute() and ".." not in path.parts and "\\" not in name,
                "unsafe archive member")


def metadata(data, ident):
    fields = BytesParser().parsebytes(data)
    for field, value in (("Name", ident["name"]), ("Version", ident["version"])):
        require(fields.get_all(field) == [value], "metadata mismatch: " + field)
    actual = fields.get_all("Requires-Python")
    require(actual is not None and len(actual) == 1 and
            set(actual[0].replace(" ", "").split(",")) ==
            set(ident["requires_python"].replace(" ", "").split(",")),
            "Requires-Python mismatch")


def audit_wheel(path, ident, platform):
    require(path.name.startswith("rawmocs-" + ident["version"] + "-cp312-cp312-")
            and path.suffix == ".whl", "unexpected wheel filename or ABI")
    require(re.fullmatch(platform, path.stem.split("-")[-1]) is not None,
            "unexpected wheel platform")
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        safe_names(names)
        files = {n for n in names if not n.endswith("/")}
        dist = "rawmocs-" + ident["version"] + ".dist-info/"
        meta, wheel, record = (dist + n for n in ("METADATA", "WHEEL", "RECORD"))
        require({meta, wheel, record, "rawmocs/__init__.py"} <= files,
                "missing package members")
        metadata(z.read(meta), ident)
        headers = BytesParser().parsebytes(z.read(wheel))
        require(headers.get("Root-Is-Purelib", "").lower() == "false",
                "native wheel marked pure")
        tags = headers.get_all("Tag", [])
        require(tags == ["cp312-cp312-" + path.stem.split("-")[-1]],
                "wheel tag mismatch")
        native = sorted(n for n in files if n.startswith("rawmocs/_native") and
                        n.endswith((".so", ".pyd")))
        require(len(native) == 1, "expected one native extension")
        rows = list(csv.reader(io.StringIO(z.read(record).decode("utf-8"))))
        require(all(len(row) == 3 for row in rows), "malformed RECORD")
        require(len(rows) == len(files) and {row[0] for row in rows} == files,
                "RECORD file coverage mismatch")
        for name, hash_field, size in rows:
            if name == record:
                require(not hash_field and not size, "RECORD self hash")
                continue
            data = z.read(name)
            h = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).decode().rstrip("=")
            require(hash_field == "sha256=" + h and size == str(len(data)),
                    "RECORD integrity mismatch: " + name)
        licenses = sorted(n for n in files if "license" in n.lower() or "copying" in n.lower())
        require(licenses, "no license file in wheel")
    return {"filename": path.name, "sha256": sha(path.read_bytes()), "tags": tags,
            "native_extension": native[0], "license_members": licenses}


def audit_sdist(path, ident):
    prefix = "rawmocs-" + ident["version"] + "/"
    require(path.name == prefix[:-1] + ".tar.gz", "unexpected sdist filename")
    with tarfile.open(path, "r:gz") as tar:
        members = tar.getmembers()
        safe_names([m.name for m in members])
        require(all(m.name == prefix[:-1] or m.name.startswith(prefix) for m in members),
                "unexpected sdist root")
        require(all(m.isfile() or m.isdir() for m in members), "unexpected sdist member type")
        def read(name):
            member = tar.getmember(prefix + name)
            require(member.isfile(), "not regular: " + name)
            return tar.extractfile(member).read()
        metadata(read("PKG-INFO"), ident)
        for name in ("pyproject.toml", PY_LOCK, CLI_LOCK,
                     "crates/rawmocs-cli/Cargo.toml", "crates/rawmocs-python/Cargo.toml",
                     "crates/rawmocs-core/Cargo.toml", "python/rawmocs/__init__.py", "LICENSE"):
            require(read(name) == (ROOT / name).read_bytes(), "sdist source mismatch: " + name)
    return {"filename": path.name, "sha256": sha(path.read_bytes())}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--wheel", type=Path, required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--sdist", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    ident = identity()
    receipt = {"schema": 1, "status": "PACKAGE_CANDIDATE_INTEGRITY_PASS",
               "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                                   text=True).strip(),
               "identity": ident, "wheel": audit_wheel(args.wheel, ident, args.platform),
               "distribution_authorized": False,
               "publication_authorized": False}
    if args.sdist:
        receipt["sdist"] = audit_sdist(args.sdist, ident)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(receipt["status"], receipt["revision"])


if __name__ == "__main__":
    main()
