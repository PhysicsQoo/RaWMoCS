"""Ordinary CI routing only; never authorizes a release.

Classify exact Git diff records, without trusting a PR title or file extension.
An unknown or malformed change takes the conservative path. Full native
performance remains explicitly requested, not a default pull request job.
"""
import argparse
import json
import os
import re
import subprocess

SHA = re.compile(r"[0-9a-f]{40}")
DOCS = {"README.md", "LICENSE", "docs/python-api.md", "docs/python-package-release.md",
        "docs/ci-parallel-release.md", "docs/ci-test-suite-rationalization.md",
        "releases/README.md"}
PACKAGE = {"pyproject.toml", "scripts/python_package_candidate.py",
           "scripts/python_source_inventory.py", "scripts/test_python_package_candidate.py"}
BINDING = {"python/rawmocs/__init__.py", "python/tests/test_native_api.py",
           "python/tests/test_native_boundary.py", "crates/rawmocs-python/src/lib.rs",
           "crates/rawmocs-python/Cargo.toml", "crates/rawmocs-python/Cargo.lock"}

def classify(raw):
    records = raw.split(b"\0")
    if records.pop() != b"" or len(records) % 2:
        raise ValueError("malformed raw diff")
    paths = {}
    for header, filename in zip(records[::2], records[1::2]):
        m = re.fullmatch(rb":([0-7]{6}) ([0-7]{6}) ([0-9a-f]{40}) ([0-9a-f]{40}) ([AMDT])", header)
        if not m:
            raise ValueError("unsupported raw diff record")
        before, after, _, _, status = m.groups()
        path = filename.decode("utf-8", "strict")
        if not path or path in paths:
            raise ValueError("duplicate or empty diff path")
        regular = after == b"100644" and (
            (status == b"M" and before == b"100644") or
            (status == b"A" and before == b"000000"))
        if not regular:
            category = "CORE_OR_UNKNOWN"
        elif path in DOCS or re.fullmatch(r"releases/v\d+\.\d+\.\d+\.md", path):
            category = "DOCS"
        elif path in PACKAGE:
            category = "PACKAGE"
        elif path in BINDING:
            category = "BINDING"
        else:
            category = "CORE_OR_UNKNOWN"
        paths[path] = category
    categories = set(paths.values())
    scope = next((x for x in ("CORE_OR_UNKNOWN", "BINDING", "PACKAGE", "DOCS") if x in categories), "DOCS")
    return {"scope": scope, "paths": paths,
            "python_contract": bool(categories & {"PACKAGE", "BINDING", "CORE_OR_UNKNOWN"}),
            "full_science_required_before_release": scope in ("BINDING", "CORE_OR_UNKNOWN"),
            "release_authorized": False}

def git(*args):
    return subprocess.check_output(["git", *args])

def inspect(base, revision):
    if not SHA.fullmatch(base or "") or not SHA.fullmatch(revision or ""):
        raise ValueError("exact base and revision required")
    for sha in (base, revision):
        if git("rev-parse", "--verify", sha + "^{commit}").decode().strip() != sha:
            raise ValueError("identity mismatch")
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, revision]).returncode:
        raise ValueError("base not ancestor")
    raw = git("diff", "--raw", "--no-renames", "--no-abbrev", "-z", base, revision, "--")
    return {**classify(raw), "base": base, "revision": revision}

def wasm_recipe_changed(report):
    # Two clean cross-builds are a release/recipe gate, not routine solver CI.
    # An unreadable diff is conservative: require the recipe check.
    return bool(report.get("reason")) or any(
        p.startswith(("scripts/wasm/",".github/actions/wasm-candidate/")) or p in (
            "scripts/stage_single_rust_crate.py", ".github/workflows/rust-science.yml",
        ) for p in report["paths"]
    )

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--base", default="")
    p.add_argument("--revision", required=True)
    p.add_argument("--out", required=True)
    a = p.parse_args()
    try:
        report = inspect(a.base, a.revision)
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        report = {"scope": "CORE_OR_UNKNOWN", "paths": {}, "base": a.base,
                  "revision": a.revision, "reason": str(exc),
                  "python_contract": True, "full_science_required_before_release": True,
                  "release_authorized": False}
    with open(a.out, "w") as f:
        json.dump(report, f, indent=2, sort_keys=True)
    print(json.dumps(report, sort_keys=True))
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as f:
            print("scope=" + report["scope"], file=f)
            print("python_contract=" + str(report["python_contract"]).lower(), file=f)
            print("wasm_contract=" + str(wasm_recipe_changed(report)).lower(), file=f)

if __name__ == "__main__":
    main()
