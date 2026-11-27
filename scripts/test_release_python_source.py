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


class ParallelTransportGate(unittest.TestCase):
    def candidate(self):
        r, run, jobs = candidate()
        source = next(j for j in jobs["jobs"] if j["name"] ==
                      "Regenerate original CNF and independent interval without artifacts")
        moved = {"R6 seven-round native transport regression against published v0.2.0",
                 "Archive shared-core full science evidence"}
        transport = [x for x in source["steps"] if x["name"] in moved]
        source["steps"] = [x for x in source["steps"] if x["name"] not in moved]
        source["steps"] += [{"name": n, "conclusion": "success"} for n in
                            ("Archive cancellation evidence for parallel transport",
                             "Archive regeneration evidence")]
        transport.append({"name": "Prepare locked native dependencies", "conclusion": "success"})
        jobs["jobs"].append({"name": "Release shared-core native transport (parallel)",
                             "status": "completed", "conclusion": "success", "steps": transport})
        jobs["total_count"] = len(jobs["jobs"])
        return r, run, jobs

    def test_parallel_full(self):
        verify_run(*self.candidate(), "science")

    def test_missing_transport_job(self):
        r, run, jobs = self.candidate()
        jobs["jobs"].pop()
        jobs["total_count"] -= 1
        with self.assertRaises(ValueError):
            verify_run(r, run, jobs, "science")

    def test_failed_or_skipped_transport(self):
        for conclusion in ("failure", "skipped"):
            r, run, jobs = self.candidate()
            jobs["jobs"][-1]["conclusion"] = conclusion
            with self.assertRaises(ValueError):
                verify_run(r, run, jobs, "science")

    def test_missing_transport_step(self):
        r, run, jobs = self.candidate()
        jobs["jobs"][-1]["steps"].pop(0)
        with self.assertRaises(ValueError):
            verify_run(r, run, jobs, "science")

    def test_missing_cancellation_archive(self):
        r, run, jobs = self.candidate()
        source = next(j for j in jobs["jobs"] if j["name"] ==
                      "Regenerate original CNF and independent interval without artifacts")
        source["steps"] = [x for x in source["steps"] if x["name"] !=
                           "Archive cancellation evidence for parallel transport"]
        with self.assertRaises(ValueError):
            verify_run(r, run, jobs, "science")



class CIOnlyV030Gate(unittest.TestCase):
    def test_scope(self):
        from release_ci_only_v030 import check_diff
        def row(path, status="M", before="100644", after="100644"):
            return (f":{before} {after} {'a'*40} {'b'*40} {status}\0{path}\0").encode()
        self.assertEqual(check_diff(row(".github/workflows/rust-science.yml")),
                         [".github/workflows/rust-science.yml"])
        for p in ("crates/rawmocs-cli/src/lib.rs", "crates/rawmocs-cli/Cargo.lock",
                  "crates/rawmocs-python/src/lib.rs", "pyproject.toml",
                  "crates/rawmocs-cli/tools/release_native_paired.py", "releases/request.json"):
            with self.subTest(path=p), self.assertRaises(ValueError):
                check_diff(row(p))
        for raw in (b"\0", row(".github/workflows/rust-science.yml", "T", "100644", "120000"),
                    row(".github/workflows/rust-science.yml")*2):
            with self.assertRaises(ValueError):
                check_diff(raw)

    def bounded(self, kind):
        from release_ci_only_v030 import SCIENCE, CODSPEED, SKIPS, REQUIRED
        r, data, _ = fixture("0.3.0")
        r["qualification"] = "v030-ci-only"
        r["codspeed_run"] = 123
        data.update(event="push", head_commit={"message":"candidate [release-package]"},
                    path=".github/workflows/" + ("rust-science.yml" if kind == "science" else "codspeed.yml"))
        passed, skipped = (SCIENCE, SKIPS) if kind == "science" else (CODSPEED, set())
        jobs = []
        for name in sorted(passed | skipped):
            needed = REQUIRED.get(name, set())
            if name.startswith("portability ("):
                needed = {"Run cargo test --workspace"}
            jobs.append({"name":name,"status":"completed",
                         "conclusion":"skipped" if name in skipped else "success",
                         "steps":[{"name":x,"conclusion":"success"} for x in needed]})
        return r, data, {"total_count":len(jobs),"jobs":jobs}, kind

    def test_bounded(self):
        from release_ci_only_v030 import verify_bounded_run
        for kind in ("science", "codspeed"):
            verify_bounded_run(*self.bounded(kind))

    def test_wrong_intent_or_revision(self):
        from release_ci_only_v030 import verify_bounded_run
        for bad in ({"event":"pull_request"},{"head_sha":"b"*40},{"head_commit":{"message":"routine"}}):
            r, data, jobs, kind = self.bounded("science")
            data.update(bad)
            with self.assertRaises(ValueError):
                verify_bounded_run(r,data,jobs,kind)

    def test_missing_source_install(self):
        from release_ci_only_v030 import verify_bounded_run
        r,data,jobs,kind=self.bounded("science")
        job=next(j for j in jobs["jobs"] if j["name"]=="First release CLI and source-locked transfer gate")
        job["steps"]=[s for s in job["steps"] if s["name"]!="Source archive and extracted-tree installation admission"]
        with self.assertRaises(ValueError):
            verify_bounded_run(r,data,jobs,kind)

    def test_skipped_light(self):
        from release_ci_only_v030 import verify_bounded_run, ROUTINE
        r,data,jobs,kind=self.bounded("science")
        next(j for j in jobs["jobs"] if j["name"]==ROUTINE)["conclusion"]="skipped"
        with self.assertRaises(ValueError):
            verify_bounded_run(r,data,jobs,kind)

    def test_wrong_version(self):
        from release_ci_only_v030 import verify_bounded_run
        r,data,jobs,kind=self.bounded("science")
        r["version"]="0.3.1"
        with self.assertRaises(ValueError):
            verify_bounded_run(r,data,jobs,kind)


    def test_combined_codspeed_both_targets_required(self):
        from release_ci_only_v030 import verify_bounded_run, COMBINED
        for missing in ("CodSpeed unified Simulation (68 identities plus QPE8 pair)",
                        "Verify simulated output, precision and plan signature",
                        "Compose one CodSpeed config for Rust harness and QPE8 pair"):
            r, data, jobs, kind = self.bounded("codspeed")
            job = next(j for j in jobs["jobs"] if j["name"] == COMBINED)
            job["steps"] = [s for s in job["steps"] if s["name"] != missing]
            with self.subTest(step=missing), self.assertRaises(ValueError):
                verify_bounded_run(r, data, jobs, kind)

    def test_combined_codspeed_not_skippable(self):
        from release_ci_only_v030 import verify_bounded_run, COMBINED
        r, data, jobs, kind = self.bounded("codspeed")
        next(j for j in jobs["jobs"] if j["name"] == COMBINED)["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            verify_bounded_run(r, data, jobs, kind)

    def test_full_policy_combined_codspeed(self):
        r, data, jobs, kind = self.bounded("codspeed")
        verify_run(r, data, jobs, kind)

if __name__ == "__main__":
    unittest.main()
