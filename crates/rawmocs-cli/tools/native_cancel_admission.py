#!/usr/bin/env python3
"""Bounded public-API cancellation trials using unchanged admitted source CNFs."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import subprocess

from release_transfer import CASES, check_count


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    for name in ("corpus", "probe", "out"):
        ap.add_argument("--" + name, type=Path, required=True)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    report = {"status": "IN_PROGRESS", "probe_sha256": digest(a.probe), "cases": []}

    def save():
        (a.out / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    save()
    for label, mode in (("qpeinexact22-432", "auto"), ("grover6-432", "auto-frontier")):
        _, _, _, expected, bits = next(row for row in CASES if row[0] == label)
        assert bits == 432
        root = a.corpus / label.rsplit("-", 1)[0]
        cnf, = root.glob("*-gpmc.cnf")
        ref = json.loads((root / "independent-reference.json").read_text())
        assert digest(cnf) == expected == ref["source_sha256"]
        proc = subprocess.run(
            ["timeout", "-k", "2s", "60s", "prlimit", "--as=3221225472", "--",
             str(a.probe.resolve()), str(cnf.resolve()), mode],
            capture_output=True, text=True, timeout=65,
        )
        (a.out / (label + ".stdout")).write_text(proc.stdout)
        (a.out / (label + ".stderr")).write_text(proc.stderr)
        assert proc.returncode == 0 and not proc.stderr, (label, proc.returncode, proc.stderr)
        lines = proc.stdout.splitlines()
        assert len(lines) == 6 and lines[4] == "s SATISFIABLE"
        baseline = int(lines[0].removeprefix("BASELINE_NS "))
        latencies = []
        for trial, line in enumerate(lines[1:4]):
            match = re.fullmatch(r"NATURAL_CANCEL_PASS trial=(\d+) latency_ns=(\d+)", line)
            assert match and int(match[1]) == trial
            latencies.append(int(match[2]))
        fields = lines[5].split()
        assert fields[:3] == ["c", "r", "mpfr"] and len(fields) == 5
        upper = check_count({"sat": True, "real": fields[3], "imag": fields[4]}, ref, 100)
        assert digest(cnf) == expected
        report["cases"].append({
            "label": label, "mode": mode, "source_sha256": expected, "bits": bits,
            "baseline_ns": baseline, "cancel_after_ms": 100, "latency_ns": latencies,
            "witness_upper": upper, "status": "PASS",
        })
        save()
        print("NATURAL_CANCEL_ADMISSION_PASS", label, latencies, flush=True)
    report["status"] = "PASS"
    save()


if __name__ == "__main__":
    main()
