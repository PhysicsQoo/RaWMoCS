import unittest
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

if __name__ == "__main__":
    unittest.main()
