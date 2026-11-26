"""Offline admission controls: fast science must never authorize a release."""
import copy
import unittest

from release_source import REPO, SCIENCE, STEPS, verify_run


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


if __name__ == "__main__":
    unittest.main()
