"""Offline negative controls for Python packaging; no solver or native build."""
import base64
import csv
import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import python_package_candidate as gate


class PackageGateTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.ident = {"name": "rawmocs", "version": "0.3.0a1",
                      "requires_python": ">=3.10"}
        self.dist = "rawmocs-0.3.0a1.dist-info/"
        self.meta = b"Name: rawmocs\nVersion: 0.3.0a1\nRequires-Python: >=3.10\n\n"
        self.wheel = self.root / "rawmocs-0.3.0a1-cp312-cp312-linux_x86_64.whl"
        self.platform = "linux_x86_64"
        self.members = {
            "rawmocs/__init__.py": b"python",
            "rawmocs/_native.cpython-312-x86_64-linux-gnu.so": b"native",
            self.dist + "METADATA": self.meta,
            self.dist + "WHEEL": b"Root-Is-Purelib: false\nTag: cp312-cp312-linux_x86_64\n",
            self.dist + "licenses/LICENSE": b"license",
        }

    def write_wheel(self, *, omit_record_row=False, mutate=False):
        record = self.dist + "RECORD"
        rows = io.StringIO()
        writer = csv.writer(rows)
        for name, data in self.members.items():
            if omit_record_row and name.endswith("LICENSE"):
                continue
            h = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).decode().rstrip("=")
            writer.writerow([name, "sha256=" + h, len(data)])
        writer.writerow([record, "", ""])
        with zipfile.ZipFile(self.wheel, "w") as z:
            for name, data in self.members.items():
                z.writestr(name, data + b"x" if mutate and name.endswith(".so") else data)
            z.writestr(record, rows.getvalue())

    def test_valid_wheel(self):
        self.write_wheel()
        self.assertEqual(gate.audit_wheel(self.wheel, self.ident, self.platform)["filename"],
                         self.wheel.name)

    def test_wrong_version(self):
        self.members[self.dist + "METADATA"] = self.meta.replace(b"0.3.0a1", b"0.3.0")
        self.write_wheel()
        with self.assertRaisesRegex(ValueError, "metadata mismatch"):
            gate.audit_wheel(self.wheel, self.ident, self.platform)

    def test_wrong_platform(self):
        self.write_wheel()
        with self.assertRaisesRegex(ValueError, "unexpected wheel platform"):
            gate.audit_wheel(self.wheel, self.ident, "linux_aarch64")

    def test_missing_license(self):
        del self.members[self.dist + "licenses/LICENSE"]
        self.write_wheel()
        with self.assertRaisesRegex(ValueError, "no license"):
            gate.audit_wheel(self.wheel, self.ident, self.platform)

    def test_record_missing_member(self):
        self.write_wheel(omit_record_row=True)
        with self.assertRaisesRegex(ValueError, "RECORD file coverage"):
            gate.audit_wheel(self.wheel, self.ident, self.platform)

    def test_record_bad_digest(self):
        self.write_wheel(mutate=True)
        with self.assertRaisesRegex(ValueError, "RECORD integrity"):
            gate.audit_wheel(self.wheel, self.ident, self.platform)

    def test_archive_path(self):
        self.members["../escape"] = b"bad"
        self.write_wheel()
        with self.assertRaisesRegex(ValueError, "unsafe archive member"):
            gate.audit_wheel(self.wheel, self.ident, self.platform)

    def write_sdist(self, *, missing=False, corrupt=False):
        source = self.root / "source"
        paths = ("pyproject.toml", gate.PY_LOCK, gate.CLI_LOCK,
                 "crates/rawmocs-cli/Cargo.toml", "crates/rawmocs-python/Cargo.toml",
                 "crates/rawmocs-core/Cargo.toml", "python/rawmocs/__init__.py", "LICENSE")
        members = {"PKG-INFO": self.meta}
        for name in paths:
            p = source / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(name.encode())
            if not (missing and name == "LICENSE"):
                members[name] = p.read_bytes() + (b"x" if corrupt and name == gate.PY_LOCK else b"")
        path = self.root / "rawmocs-0.3.0a1.tar.gz"
        with tarfile.open(path, "w:gz") as t:
            for name, data in members.items():
                info = tarfile.TarInfo("rawmocs-0.3.0a1/" + name)
                info.size = len(data)
                t.addfile(info, io.BytesIO(data))
        return source, path

    def test_sdist_valid(self):
        source, path = self.write_sdist()
        with patch.object(gate, "ROOT", source):
            self.assertEqual(gate.audit_sdist(path, self.ident)["filename"], path.name)

    def test_sdist_missing_license(self):
        source, path = self.write_sdist(missing=True)
        with patch.object(gate, "ROOT", source), self.assertRaisesRegex(ValueError, "missing sdist member"):
            gate.audit_sdist(path, self.ident)

    def test_sdist_wrong_lock(self):
        source, path = self.write_sdist(corrupt=True)
        with patch.object(gate, "ROOT", source), self.assertRaisesRegex(ValueError, "sdist source mismatch"):
            gate.audit_sdist(path, self.ident)


if __name__ == "__main__":
    unittest.main()
