"""Offline versioned source-only release controls."""
import unittest
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
if __name__ == "__main__":
    unittest.main()
