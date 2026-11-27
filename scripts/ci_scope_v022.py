"""Shadow-only, fail-closed scope classifier for v0.2.2 CI rationalization.

The reference is the test-only candidate with successful Science/CodSpeed
receipts, not an independently published version. Classification is not
release admission: the existing release validator still requires full science.
"""
import argparse
import json
import re
import subprocess
import sys
import unittest

QUALIFIED = "6dc7affac2003f445fa9f896aafee76b969752e3"
PUBLISHED = "b6904f9b68f71e1a791c0607d087e7c1156b622e"
CI_PATHS = frozenset({
    ".github/workflows/rust-science.yml",
    "scripts/ci_scope_v022.py",
    "scripts/release_source.py",
    "scripts/test_release_source.py",
    "docs/ci-test-suite-rationalization.md",
    "releases/v0.2.2.md",
})
VERSION_PATHS = frozenset({
    "crates/rawmocs-cli/Cargo.toml",
    "crates/rawmocs-cli/Cargo.lock",
})
SHA = re.compile(r"[0-9a-f]{40}")


def git(*args):
    return subprocess.check_output(["git", *args])


def version_only(old, new):
    needle = b'name = "rawmocs-cli"\nversion = "0.2.1"\n'
    replacement = b'name = "rawmocs-cli"\nversion = "0.2.2"\n'
    return old.count(needle) == 1 and new == old.replace(needle, replacement, 1)


def classify(raw, blob):
    """NUL-delimited raw git diff; no filename glob or self-reported scope."""
    records = raw.split(b"\0")
    if records.pop() != b"" or len(records) % 2:
        raise ValueError("malformed diff")
    reasons = []
    paths = set()
    for header, filename in zip(records[::2], records[1::2]):
        match = re.fullmatch(rb":([0-7]{6}) ([0-7]{6}) ([0-9a-f]{40}) ([0-9a-f]{40}) ([AMDT])", header)
        if not match:
            raise ValueError("unsupported raw diff record")
        before, after, old, new, status = match.groups()
        path = filename.decode("utf-8", errors="strict")
        if not path or path in paths:
            raise ValueError("duplicate or empty diff path")
        paths.add(path)
        regular = after == b"100644" and (
            (status == b"M" and before == b"100644") or
            (status == b"A" and before == b"000000")
        )
        if regular and path in CI_PATHS:
            continue
        if (regular and status == b"M" and path in VERSION_PATHS
                and version_only(blob(old.decode()), blob(new.decode()))):
            continue
        reasons.append(path)
    return {"scope": "FULL_SCIENCE" if reasons else "CI_ONLY_CANDIDATE",
            "changed_paths": sorted(paths), "full_science_reasons": sorted(reasons)}


def inspect(revision):
    if not SHA.fullmatch(revision):
        raise ValueError("exact candidate SHA required")
    for commit in (QUALIFIED, PUBLISHED, revision):
        if git("rev-parse", "--verify", commit + "^{commit}").decode().strip() != commit:
            raise ValueError("commit identity mismatch")
    diff = git("diff", "--raw", "--no-renames", "--no-abbrev", "-z",
               QUALIFIED, revision, "--")
    report = classify(diff, lambda sha: git("cat-file", "blob", sha))
    return {**report, "revision": revision, "qualified_reference": QUALIFIED,
            "published_reference": PUBLISHED, "release_authorized": False}


class ScopeTests(unittest.TestCase):
    def record(self, path, status="M", oldmode="100644", newmode="100644"):
        return (f":{oldmode} {newmode} {'a'*40} {'b'*40} {status}\0{path}\0").encode()

    def test_allowlist_exact_paths(self):
        for path in CI_PATHS:
            self.assertEqual(classify(self.record(path), lambda _: b"")["scope"],
                             "CI_ONLY_CANDIDATE")
        for path in ("crates/rawmocs-cli/src/general.rs",
                     "crates/rawmocs-cli/tests/shared_core.rs",
                     "crates/rawmocs-cli/tools/small_family_qualification.py",
                     ".github/workflows/codspeed.yml", "unknown.md",
                     "scripts/ci_scope_v022.py.extra", "releases/request.json"):
            with self.subTest(path=path):
                self.assertEqual(classify(self.record(path), lambda _: b"")["scope"],
                                 "FULL_SCIENCE")

    def test_version_only(self):
        old = b'[package]\nname = "rawmocs-cli"\nversion = "0.2.1"\n'
        new = old.replace(b"0.2.1", b"0.2.2")
        for path in VERSION_PATHS:
            for candidate, expected in ((new, "CI_ONLY_CANDIDATE"),
                                         (new+b'edition = "2024"\n', "FULL_SCIENCE"),
                                         (old, "FULL_SCIENCE")):
                with self.subTest(path=path, candidate=candidate):
                    result = classify(self.record(path), lambda sha: old if sha == "a"*40 else candidate)
                    self.assertEqual(result["scope"], expected)

    def test_modes_and_mixed_changes(self):
        path = "scripts/ci_scope_v022.py"
        for status, before, after in (("D", "100644", "000000"),
                                      ("T", "100644", "120000"),
                                      ("M", "100644", "100755")):
            self.assertEqual(classify(self.record(path, status, before, after),
                                      lambda _: b"")["scope"], "FULL_SCIENCE")
        result = classify(self.record(path)+self.record("Cargo.toml"), lambda _: b"")
        self.assertEqual(result["full_science_reasons"], ["Cargo.toml"])

    def test_malformed_and_duplicate(self):
        row = self.record("scripts/ci_scope_v022.py")
        for bad in (row[:-1], row+row, b"bad\0file\0"):
            with self.assertRaises(ValueError):
                classify(bad, lambda _: b"")
        with self.assertRaises(ValueError):
            inspect("main")

    def test_empty_diff_does_not_authorize_release(self):
        self.assertEqual(classify(b"", lambda _: b"")["scope"], "CI_ONLY_CANDIDATE")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--revision")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(ScopeTests)
        if not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful():
            sys.exit(1)
    if args.revision:
        print(json.dumps(inspect(args.revision), sort_keys=True))
    elif not args.self_test:
        parser.error("provide --self-test or --revision")


if __name__ == "__main__":
    main()
