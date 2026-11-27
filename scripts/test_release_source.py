"""Offline admission controls: fast science must never authorize a release."""
import copy
import io
import json
import tarfile
import unittest

from release_source import REPO, SCIENCE, STEPS, digest, native_policy, verify_assets, verify_draft, verify_run


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
            if tuple(map(int, version.split("."))) >= (0, 2, 0):
                required = {"Select versioned native admission policy",
                            "Seven order-rotated same-precision native quadruples"}
            else:
                required = {"Seven order-rotated same-precision native triples"}
        if tuple(map(int, version.split('.'))) >= (0, 2, 1) and name.startswith("Regenerate original"):
            required = set(required) | {
                "Natural native cancellation on original source-locked CNFs",
                "R6 seven-round native transport regression against published v0.2.0",
                "Archive shared-core full science evidence",
            }
        jobs.append({"name": name, "status": "completed", "conclusion": "success",
                     "steps": [{"name": s, "conclusion": "success"} for s in required]})
    return request, run, {"total_count": len(jobs), "jobs": jobs}


class ReleaseScienceTests(unittest.TestCase):
    def test_full_gate(self):
        verify_run(*fixture(), "science")

    def test_versioned_native_policy(self):
        for version in ("0.1.0", "0.1.1"):
            self.assertEqual(native_policy(version), {
                "quadruples": False, "baseline": "ef8d1ee606bed672ee9be62d189a2b3fbcc905a0"})
        for version in ("0.2.0", "0.2.1", "0.3.0"):
            self.assertEqual(native_policy(version), {
                "quadruples": True, "baseline": "b3d02b2720c2fae5231c0ed5cf44950164a5780d"})
        self.assertEqual(native_policy("0.1.1", True), native_policy("0.2.0"))
        with self.assertRaises(ValueError):
            native_policy("0.2.0-rc.1")

    def test_v020_requires_quadruples_and_policy(self):
        verify_run(*fixture("0.2.0"), "science")
        for missing in ("Select versioned native admission policy",
                        "Seven order-rotated same-precision native quadruples"):
            req, run, jobs = fixture("0.2.0")
            for j in jobs["jobs"]:
                for step in j["steps"]:
                    if step["name"] == missing:
                        step["conclusion"] = "skipped"
            with self.subTest(missing=missing), self.assertRaises(ValueError):
                verify_run(req, run, jobs, "science")

    def test_v020_rejects_legacy_and_untrusted_receipts(self):
        req, run, jobs = fixture("0.2.0")
        for overrides in ({"event": "pull_request"},
                          {"head_branch": "candidate/v020-frontier-convergence"},
                          {"head_sha": "b" * 40}):
            with self.subTest(overrides=overrides), self.assertRaises(ValueError):
                verify_run(req, {**run, **overrides}, jobs, "science")
        _, _, old = fixture("0.1.1")
        with self.assertRaises(ValueError):
            verify_run(req, run, old, "science")

    def test_v021_requires_exact_main_shared_core_evidence(self):
        verify_run(*fixture("0.2.1"), "science")
        for missing in (
            "Natural native cancellation on original source-locked CNFs",
            "R6 seven-round native transport regression against published v0.2.0",
            "Archive shared-core full science evidence",
        ):
            for conclusion in ("skipped", "missing"):
                req, run, jobs = fixture("0.2.1")
                job = next(j for j in jobs["jobs"] if j["name"].startswith("Regenerate original"))
                if conclusion == "missing":
                    job["steps"] = [s for s in job["steps"] if s["name"] != missing]
                else:
                    next(s for s in job["steps"] if s["name"] == missing)["conclusion"] = conclusion
                with self.subTest(step=missing, conclusion=conclusion), self.assertRaises(ValueError):
                    verify_run(req, run, jobs, "science")
        req, run, jobs = fixture("0.2.1")
        for overrides in ({"head_sha": "b" * 40}, {"event": "pull_request"}):
            with self.subTest(overrides=overrides), self.assertRaises(ValueError):
                verify_run(req, {**run, **overrides}, jobs, "science")
        _, _, legacy_jobs = fixture("0.2.0")
        with self.assertRaises(ValueError):
            verify_run(req, run, legacy_jobs, "science")
        verify_run(*fixture("0.2.0"), "science")

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
    shared = tuple(map(int, version.split("."))) >= (0, 2, 1)
    manifest = f'[package]\nversion = "{version}"\n'
    if shared:
        manifest += '[lib]\nname = "rawmocs_mpfr"\n'
    files = [
        ("LICENSE", b"fixture license"),
        ("crates/rawmocs-cli/Cargo.lock", lock),
        ("crates/rawmocs-cli/Cargo.toml", manifest.encode()),
    ]
    if shared:
        files += [("crates/rawmocs-cli/src/lib.rs", b"fixture library"),
                  ("crates/rawmocs-cli/tools/external_consumer.rs", b"fixture consumer")]
    with tarfile.open(fileobj=stream, mode="w:gz", format=tarfile.PAX_FORMAT,
                      pax_headers={"comment": revision}) as tar:
        for name, data in files:
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
    if shared:
        receipt["rust_consumer_controls"] = ["file-text", "SAT-zero", "UNSAT", "MPFR-binary", "diagnostics",
                 "invalid-input", "timeout", "cancel", "reentry", "quiet-stdio", "locked-offline"]
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

    def test_v021_requires_extracted_consumer_evidence(self):
        verify_assets(*source_fixture("0.2.1"))
        for bad in (None, ["file-text"]):
            request, assets = source_fixture("0.2.1")
            receipt = json.loads(assets["source-package.json"])
            if bad is None:
                del receipt["rust_consumer_controls"]
            else:
                receipt["rust_consumer_controls"] = bad
            assets["source-package.json"] = json.dumps(receipt).encode()
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError, "missing extracted Rust consumer"):
                verify_assets(request, assets)
        verify_assets(*source_fixture("0.2.0"))

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


class ReleaseDraftTests(unittest.TestCase):
    def test_exact_ci_draft_resume_and_wrong_identity_rejected(self):
        request, assets = source_fixture("0.1.1")
        notes = "# Release notes"
        draft = {
            "id": 42, "tag_name": "v0.1.1", "draft": True, "prerelease": False,
            "target_commitish": request["revision"], "name": "RaWMoCS v0.1.1",
            "body": notes, "author": {"login": "github-actions[bot]"},
            "assets": [{"name": name, "digest": "sha256:" + digest(data),
                        "state": "uploaded"} for name, data in assets.items()],
        }
        verify_draft(draft, request, assets, None, notes)
        verify_draft(draft, request, assets, request["revision"], notes)
        for field, value in (
            ("target_commitish", "b" * 40), ("body", "other notes"),
            ("name", "other title"), ("author", {"login": "other"}),
            ("draft", False), ("assets", []), ("id", 0),
        ):
            with self.subTest(field=field), self.assertRaises(ValueError):
                verify_draft({**draft, field: value}, request, assets, None, notes)
        with self.assertRaises(ValueError):
            verify_draft(draft, request, assets, "b" * 40, notes)
        corrupted = copy.deepcopy(draft)
        corrupted["assets"][0]["digest"] = "sha256:" + "0" * 64
        with self.assertRaises(ValueError):
            verify_draft(corrupted, request, assets, None, notes)



class RationalizedCandidateTests(unittest.TestCase):
    def fixture(self):
        from ci_scope_v022 import PUBLISHED, QUALIFIED
        from release_source import REPO, V022_SCIENCE, V022_SKIPPED_JOB, V022_STEPS
        revision = "a" * 40
        request = {"version": "0.2.2", "revision": revision, "science_run": 123}
        run_data = {
            "id": 123, "repository": {"full_name": REPO},
            "head_repository": {"full_name": REPO},
            "path": ".github/workflows/rust-science.yml",
            "head_branch": "main", "head_sha": revision,
            "event": "workflow_dispatch", "status": "completed",
            "conclusion": "success",
        }
        jobs = [
            {"name": name, "status": "completed", "conclusion": "success",
             "steps": [{"name": step, "conclusion": "success"}
                       for step in sorted(V022_STEPS[name])]}
            for name in sorted(V022_SCIENCE)
        ]
        jobs.append({"name": V022_SKIPPED_JOB, "status": "completed",
                     "conclusion": "skipped", "steps": []})
        scope = {"revision": revision, "qualified_reference": QUALIFIED,
                 "published_reference": PUBLISHED, "scope": "CI_ONLY_CANDIDATE",
                 "full_science_reasons": [], "release_authorized": False}
        return request, run_data, {"total_count": len(jobs), "jobs": jobs}, scope

    def test_candidate_contract_is_not_legacy_release_authorization(self):
        from release_source import verify_run, verify_v022_rationalized_run
        request, run_data, jobs, scope = self.fixture()
        verify_v022_rationalized_run(request, run_data, jobs, scope)
        with self.assertRaises(ValueError):
            verify_run(request, run_data, jobs, "science")

    def test_wrong_version_scope_or_revision(self):
        from release_source import verify_v022_rationalized_run
        for key, value in (
            ("revision", "b" * 40), ("qualified_reference", "b" * 40),
            ("published_reference", "b" * 40), ("scope", "FULL_SCIENCE"),
            ("full_science_reasons", ["src/lib.rs"]), ("release_authorized", True),
        ):
            with self.subTest(key=key):
                request, run_data, jobs, scope = self.fixture()
                scope[key] = value
                with self.assertRaises(ValueError):
                    verify_v022_rationalized_run(request, run_data, jobs, scope)
        for version in ("0.2.1", "0.2.3", "0.3.0"):
            request, run_data, jobs, scope = self.fixture()
            request["version"] = version
            with self.assertRaises(ValueError):
                verify_v022_rationalized_run(request, run_data, jobs, scope)

    def test_wrong_run_identity_or_intent(self):
        from release_source import verify_v022_rationalized_run
        for key, value in (
            ("id", 124), ("head_sha", "b" * 40),
            ("head_branch", "research/v022-ci-rationalization"),
            ("event", "pull_request"), ("event", "push"),
            ("head_repository", {"full_name": "other/repo"}),
            ("path", ".github/workflows/other.yml"),
            ("status", "in_progress"), ("conclusion", "failure"),
        ):
            with self.subTest(key=key, value=value):
                request, run_data, jobs, scope = self.fixture()
                run_data[key] = value
                with self.assertRaises(ValueError):
                    verify_v022_rationalized_run(request, run_data, jobs, scope)

    def test_required_steps_cannot_be_skipped_or_missing(self):
        from release_source import V022_STEPS, verify_v022_rationalized_run
        for name, required in V022_STEPS.items():
            for step in required:
                for conclusion in ("skipped", "failure", "missing"):
                    with self.subTest(name=name, step=step, conclusion=conclusion):
                        request, run_data, jobs, scope = self.fixture()
                        job = next(j for j in jobs["jobs"] if j["name"] == name)
                        if conclusion == "missing":
                            job["steps"] = [s for s in job["steps"] if s["name"] != step]
                        else:
                            next(s for s in job["steps"] if s["name"] == step)["conclusion"] = conclusion
                        with self.assertRaises(ValueError):
                            verify_v022_rationalized_run(request, run_data, jobs, scope)

    def test_duplicate_critical_step_rejected_but_setup_may_repeat(self):
        from release_source import V022_SMALL_JOB, verify_v022_rationalized_run
        request, run_data, jobs, scope = self.fixture()
        job = next(j for j in jobs["jobs"] if j["name"] == V022_SMALL_JOB)
        job["steps"] += [{"name": "Run actions/checkout@v4", "conclusion": "success"}] * 2
        verify_v022_rationalized_run(request, run_data, jobs, scope)
        job["steps"].append(copy.deepcopy(job["steps"][0]))
        with self.assertRaises(ValueError):
            verify_v022_rationalized_run(request, run_data, jobs, scope)

    def test_wrong_job_set_or_status(self):
        from release_source import V022_SMALL_JOB, V022_SKIPPED_JOB, verify_v022_rationalized_run
        for name, conclusion in ((V022_SMALL_JOB, "skipped"), (V022_SKIPPED_JOB, "failure"),
                                 (V022_SKIPPED_JOB, "success"), ("core", "cancelled")):
            request, run_data, jobs, scope = self.fixture()
            next(j for j in jobs["jobs"] if j["name"] == name)["conclusion"] = conclusion
            with self.assertRaises(ValueError):
                verify_v022_rationalized_run(request, run_data, jobs, scope)
        request, run_data, jobs, scope = self.fixture()
        jobs["jobs"].pop()
        jobs["total_count"] -= 1
        with self.assertRaises(ValueError):
            verify_v022_rationalized_run(request, run_data, jobs, scope)


if __name__ == "__main__":
    unittest.main()
