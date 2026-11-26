"""Offline admission controls: fast science must never authorize a release."""
import copy
import io
import json
import tarfile
import unittest

from release_source import REPO, SCIENCE, STEPS, digest, verify_assets, verify_run


def fixture(version="0.1.1"):
    revision = "a" * 40
    request = {"version": version, "revision": revision, "science_run": 123}
    run = {
        "id": 123, "repository": {"full_name": REPO}, "head_repository": {"full_name": REPO},
        "path": ".github/workflows/rust-science.yml", "head_branch": "main",
        "head_sha": revision, "event": "workflow_dispatch", "status": "completed",
        "conclusion": "success",
    }
    jobs = []
    for name in sorted(SCIENCE):
        required = STEPS.get(name, ())
        if version != "0.1.0" and name.startswith("Release matched native GPMC ("):
            required = {"Seven order-rotated same-precision native triples"}
        jobs.append({"name": name, "status": "completed", "conclusion": "success",
                     "steps": [{"name": s, "conclusion": "success"} for s in required]})
    return request, run, {"total_count": len(jobs), "jobs": jobs}


class ReleaseScienceTests(unittest.TestCase):
    def test_full_gate(self):
        verify_run(*fixture(), "science")

    def test_published_legacy_gate(self):
        verify_run(*fixture("0.1.0"), "science")

    def test_push_not_release_admission(self):
        req, run, jobs = fixture()
        run["event"] = "push"
        with self.assertRaises(ValueError):
            verify_run(req, run, jobs, "science")

    def test_explicit_release_push_full_gate(self):
        req, run, jobs = fixture()
        run["event"] = "push"
        run["head_commit"] = {"message": "Prepare next release [release-science]"}
        verify_run(req, run, jobs, "science")

    def test_explicit_release_push_fast_still_rejected(self):
        req, run, jobs = fixture()
        run["event"] = "push"
        run["head_commit"] = {"message": "Prepare next release [release-science]"}
        for j in jobs["jobs"]:
            if j["name"].startswith("Release matched native GPMC ("):
                j["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            verify_run(req, run, jobs, "science")

    def test_manual_fast_not_release_admission(self):
        req, run, jobs = fixture()
        for j in jobs["jobs"]:
            if j["name"].startswith("Regenerate original"):
                j["name"] = "Replay seven source-locked independent witnesses"
            elif j["name"].startswith("Release matched native GPMC ("):
                j["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            verify_run(req, run, jobs, "science")

    def test_missing_required_full_evidence(self):
        for missing in (
            "Source archive and extracted-tree installation admission",
            "Regenerate six source CNFs and verify seven cohorts",
            "Seven order-rotated same-precision native triples",
        ):
            req, run, jobs = copy.deepcopy(fixture())
            for j in jobs["jobs"]:
                for step in j["steps"]:
                    if step["name"] == missing:
                        step["conclusion"] = "skipped"
            with self.subTest(missing=missing), self.assertRaises(ValueError):
                verify_run(req, run, jobs, "science")

    def test_old_pair_not_new_release_admission(self):
        req, run, jobs = fixture()
        for j in jobs["jobs"]:
            for step in j["steps"]:
                if step["name"] == "Seven order-rotated same-precision native triples":
                    step["name"] = "Seven order-rotated same-precision native pairs"
        with self.assertRaises(ValueError):
            verify_run(req, run, jobs, "science")


def source_fixture(version):
    revision = "a" * 40
    archive = f"rawmocs-{version}-source.tar.gz"
    prefix = f"rawmocs-{version}/"
    lock = b"version = 4\n"
    stream = io.BytesIO()
    with tarfile.open(fileobj=stream, mode="w:gz", format=tarfile.PAX_FORMAT,
                      pax_headers={"comment": revision}) as tar:
        for name, data in (
            ("LICENSE", b"fixture license"),
            ("crates/rawmocs-cli/Cargo.lock", lock),
            ("crates/rawmocs-cli/Cargo.toml",
             f'[package]\nversion = "{version}"\n'.encode()),
        ):
            member = tarfile.TarInfo(prefix + name)
            member.size = len(data)
            tar.addfile(member, io.BytesIO(data))
    data = stream.getvalue()
    sha = digest(data)
    receipt = {
        "status": "SOURCE_PACKAGE_INSTALL_PASS", "revision": revision,
        "version": version, "target": "x86_64-unknown-linux-gnu",
        "source_archive": archive, "source_sha256": sha,
        "cargo_lock_sha256": digest(lock), "deterministic_archive": True,
        "redistributed_binary": False, "offline_build": False,
        "controls": ["version", "help", "SAT-zero", "UNSAT", "general", "timeout"],
    }
    if version != "0.1.0":
        receipt["large_general_controls"] = ["analytic", "default-cap", "parse-deadline"]
    return {"version": version, "revision": revision, "source_sha256": sha}, {
        archive: data,
        "SHA256SUMS": f"{sha}  {archive}\n".encode(),
        "source-package.json": json.dumps(receipt).encode(),
    }


class ReleaseSourceTests(unittest.TestCase):
    def test_legacy_and_new_receipt(self):
        for version in ("0.1.0", "0.1.1"):
            with self.subTest(version=version):
                verify_assets(*source_fixture(version))

    def test_new_receipt_requires_large_route_controls(self):
        request, assets = source_fixture("0.1.1")
        receipt = json.loads(assets["source-package.json"])
        del receipt["large_general_controls"]
        assets["source-package.json"] = json.dumps(receipt).encode()
        with self.assertRaisesRegex(ValueError, "missing installed large General evidence"):
            verify_assets(request, assets)

    def test_new_receipt_rejects_wrong_version(self):
        request, assets = source_fixture("0.1.1")
        receipt = json.loads(assets["source-package.json"])
        receipt["version"] = "0.1.0"
        assets["source-package.json"] = json.dumps(receipt).encode()
        with self.assertRaisesRegex(ValueError, "receipt mismatch: version"):
            verify_assets(request, assets)


if __name__ == "__main__":
    unittest.main()
