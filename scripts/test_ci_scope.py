import unittest
from pathlib import Path
from ci_scope import classify

def row(path, status="M", old="100644", new="100644"):
    return (f":{old} {new} {'a'*40} {'b'*40} {status}\0{path}\0").encode()

class ScopeTests(unittest.TestCase):
    def test_docs(self):
        r = classify(row("README.md"))
        self.assertEqual(r["scope"], "DOCS")
        self.assertFalse(r["python_contract"])
    def test_package(self):
        r = classify(row("pyproject.toml"))
        self.assertEqual(r["scope"], "PACKAGE")
        self.assertTrue(r["python_contract"])
        self.assertFalse(r["release_authorized"])
    def test_binding(self):
        r = classify(row("crates/rawmocs-python/src/lib.rs"))
        self.assertEqual(r["scope"], "BINDING")
        self.assertTrue(r["full_science_required_before_release"])
    def test_core_and_unknown(self):
        for p in ("Cargo.toml", "crates/rawmocs-cli/Cargo.lock", "crates/rawmocs-cli/src/lib.rs",
                  "scripts/release_source.py", "docs/unknown.md", "python/unknown.py",
                  ".github/workflows/rust-science.yml", "releases/request.json"):
            self.assertEqual(classify(row(p))["scope"], "CORE_OR_UNKNOWN")
    def test_mixed(self):
        r = classify(row("pyproject.toml")+row("crates/rawmocs-cli/src/main.rs"))
        self.assertEqual(r["scope"], "CORE_OR_UNKNOWN")
    def test_modes(self):
        for old, new, status in (("100644","100755","M"), ("100644","120000","T"),
                                 ("100644","000000","D")):
            self.assertEqual(classify(row("pyproject.toml", status, old, new))["scope"], "CORE_OR_UNKNOWN")
    def test_malformed(self):
        for raw in (row("README.md")[:-1], row("README.md")*2, b"bad\0x\0"):
            with self.assertRaises(ValueError):
                classify(raw)
    def test_empty_no_authority(self):
        self.assertFalse(classify(b"")["release_authorized"])


class ManualLongCI(unittest.TestCase):
    root = Path(__file__).resolve().parents[1] / ".github" / "workflows"

    def job_guard(self, workflow, job):
        lines = (self.root / workflow).read_text().splitlines()
        start = lines.index("  " + job + ":") + 1
        end = next((i for i in range(start, len(lines))
                    if lines[i].startswith("  ") and not lines[i].startswith("    ")
                    and lines[i].endswith(":")), len(lines))
        return next(l.strip() for l in lines[start:end] if l.startswith("    if: "))

    def test_science_long_jobs_dispatch_only(self):
        for name in ("release-native-transport", "release-native-paired",
                     "python-portability", "python-manylinux"):
            self.assertEqual(self.job_guard("rust-science.yml", name),
                             "if: github.event_name == 'workflow_dispatch' && inputs.suite == 'full'")

    def test_no_push_title_full_intent(self):
        for filename in ("rust-science.yml", "codspeed.yml"):
            source = (self.root / filename).read_text()
            for marker in ("[release-science]", "[parallel-full]", "[full-science]", "[performance]"):
                self.assertNotIn(marker, source)
            if filename == "rust-science.yml":
                self.assertIn("default: fast", source)

    def test_combined_codspeed_auto(self):
        source = (self.root / "codspeed.yml").read_text()
        self.assertNotIn("inputs.suite", source)
        self.assertIn("  pull_request:", source)
        self.assertIn("  push:", source)
        self.assertIn("  workflow_dispatch:", source)
        self.assertIn("github.event.pull_request.head.repo.full_name == github.repository", source)
        self.assertEqual(source.count("uses: CodSpeedHQ/action@v5"), 1)
        self.assertIn("entrypoint:", source)
        self.assertIn("cargo codspeed run --bench solver --bench quprs_direct_cnf", source)
        self.assertIn("Verify simulated output, precision and plan signature", source)
        self.assertNotIn("continue-on-error: true", source)

    def test_manual_separate_concurrency(self):
        for filename, suffix in (("rust-science.yml", "inputs.suite || 'routine'"),
                                 ("codspeed.yml", "'manual' || 'routine'")):
            source = (self.root / filename).read_text()
            group = next(l for l in source.splitlines() if l.startswith("  group: "))
            self.assertIn("github.event_name == 'workflow_dispatch' && "+suffix, group)

if __name__ == "__main__":
    unittest.main()
