"""Offline versioned source-only release controls."""
import unittest
import io
import json
import tarfile
from release_python_source import digest, SDIST
from release_source import verify_run
from test_release_source import fixture
from release_python_source import PYTHON_JOBS, PYTHON_STEPS, RESEARCH_SKIPS, verify_python_assets

def candidate():
    r, run, jobs = fixture("0.3.0")
    for job in jobs["jobs"]:
        job["steps"] += [{"name": x, "conclusion": "success"}
                         for x in PYTHON_STEPS.get(job["name"], set())]
    for name in sorted(PYTHON_JOBS | RESEARCH_SKIPS):
        jobs["jobs"].append({"name": name, "status": "completed",
                             "conclusion": "skipped" if name in RESEARCH_SKIPS else "success",
                             "steps": [{"name": x, "conclusion": "success"}
                                       for x in PYTHON_STEPS.get(name, set())]})
    jobs["total_count"] = len(jobs["jobs"])
    return r, run, jobs

class V030SourceGate(unittest.TestCase):
    def test_full_source(self):
        verify_run(*candidate(), "science")
    def test_legacy_without_python_rejected(self):
        with self.assertRaises(ValueError):
            verify_run(*fixture("0.3.0"), "science")
    def test_skipped_python_rejected(self):
        r, run, jobs = candidate()
        next(j for j in jobs["jobs"] if j["name"] in PYTHON_JOBS)["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            verify_run(r, run, jobs, "science")
    def test_wrong_revision_rejected(self):
        r, run, jobs = candidate()
        run["head_sha"] = "b" * 40
        with self.assertRaises(ValueError):
            verify_run(r, run, jobs, "science")
    def test_missing_python_step_rejected(self):
        r, run, jobs = candidate()
        next(j for j in jobs["jobs"] if j["name"] in PYTHON_JOBS)["steps"] = []
        with self.assertRaises(ValueError):
            verify_run(r, run, jobs, "science")
    def test_memory_job_not_release_gate(self):
        r, run, jobs = candidate()
        next(j for j in jobs["jobs"] if j["name"] in RESEARCH_SKIPS)["conclusion"] = "success"
        with self.assertRaises(ValueError):
            verify_run(r, run, jobs, "science")
    def test_missing_source_assets(self):
        with self.assertRaises(ValueError):
            verify_python_assets({}, {})

def pack(files):
    out = io.BytesIO()
    with tarfile.open(fileobj=out, mode="w:gz") as t:
        for name, data in files.items():
            info = tarfile.TarInfo("rawmocs-0.3.0/" + name)
            info.size = len(data)
            t.addfile(info, io.BytesIO(data))
    return out.getvalue()

def assets_fixture(extra=None):
    source = {
        "LICENSE": b"MIT",
        "pyproject.toml": b'[project]\nname = "rawmocs"\nversion = "0.3.0"\nrequires-python = ">=3.12,<3.13"\n',
        "crates/rawmocs-cli/Cargo.toml": b'[package]\nversion = "0.3.0"\n',
        "crates/rawmocs-python/Cargo.toml": b'[package]\nversion = "0.3.0"\n[dependencies.rawmocs_mpfr]\nversion = "=0.3.0"\n',
        "crates/rawmocs-cli/Cargo.lock": b"cli-lock",
        "crates/rawmocs-python/Cargo.lock": b"binding-lock",
        "python/rawmocs/__init__.py": b"source",
    }
    files = dict(source)
    files["PKG-INFO"] = b"Name: rawmocs\nVersion: 0.3.0\nRequires-Python: >=3.12,<3.13\n\n"
    if extra:
        files.update(extra)
    blob = pack(files)
    r = {"revision": "a" * 40, "python_source_sha256": digest(blob)}
    sd = {"filename": SDIST, "sha256": digest(blob)}
    inv = {"revision": r["revision"], "status": "SOURCE_INVENTORY_PASS", "sdist": sd,
           "distribution_authorized": False, "publication_authorized": False,
           "files": [{"path": k, "sha256": digest(v)} for k, v in files.items()],
           "dependency_license_inventory": [
               {"name": "rug", "version": "1.30.0", "license": "LGPL-3.0-or-later"},
               {"name": "gmp-mpfr-sys", "version": "1.7.1", "license": "LGPL-3.0-or-later"}]}
    receipt = {"revision": r["revision"], "status": "PACKAGE_CANDIDATE_INTEGRITY_PASS",
               "distribution_authorized": False, "publication_authorized": False,
               "identity": {"version": "0.3.0", "shared_cli_version": "0.3.0",
                            "python_lock_sha256": digest(source["crates/rawmocs-python/Cargo.lock"]),
                            "cli_lock_sha256": digest(source["crates/rawmocs-cli/Cargo.lock"])},
               "wheel": {"sha256": "b" * 64}, "sdist": sd}
    assets = {SDIST: blob, "rawmocs-0.3.0-source.tar.gz": pack(source),
              "source-inventory.json": json.dumps(inv).encode(),
              "package-candidate.json": json.dumps(receipt).encode(),
              "rebuilt-package-candidate.json": json.dumps(receipt).encode()}
    return r, assets

class SourceAssetTests(unittest.TestCase):
    def test_valid_sdist(self):
        verify_python_assets(*assets_fixture())
    def test_wrong_digest(self):
        r, assets = assets_fixture()
        r["python_source_sha256"] = "c" * 64
        with self.assertRaisesRegex(ValueError, "sdist digest mismatch"):
            verify_python_assets(r, assets)
    def test_compiled_member(self):
        with self.assertRaisesRegex(ValueError, "compiled member"):
            verify_python_assets(*assets_fixture({"rawmocs/native.so": b"native"}))
    def test_untracked_member(self):
        with self.assertRaisesRegex(ValueError, "source archive mismatch"):
            verify_python_assets(*assets_fixture({"untracked.txt": b"bad"}))
    def test_tampered_source(self):
        with self.assertRaisesRegex(ValueError, "source archive mismatch"):
            verify_python_assets(*assets_fixture({"LICENSE": b"changed"}))
    def test_forged_receipt(self):
        r, assets = assets_fixture()
        inv = json.loads(assets["source-inventory.json"])
        inv["revision"] = "b" * 40
        assets["source-inventory.json"] = json.dumps(inv).encode()
        with self.assertRaisesRegex(ValueError, "source inventory identity"):
            verify_python_assets(r, assets)

if __name__ == "__main__":
    unittest.main()
