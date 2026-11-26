#!/usr/bin/env python3
"""Replay a fresh candidate against the digest-pinned independent corpus.

The caller verifies the original ZIP digest. Never execute corpus programs.
This is numeric regression evidence, not a new reference or speed comparison.
"""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import subprocess

from regenerate_release import DIGITS, PIN
from release_transfer import CASES, check_count


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    for name in ("bin", "corpus", "out"):
        ap.add_argument("--" + name, type=Path, required=True)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    regen = json.loads((a.corpus / "regeneration.json").read_text())
    assert regen["status"] == "REGENERATION_COMPLETE" and regen["quprs_revision"] == PIN
    assert len(regen["sources"]) == 6 and len(regen["cohorts"]) == len(CASES)
    report = {
        "status": "IN_PROGRESS",
        "binary_sha256": digest(a.bin),
        "regeneration_sha256": digest(a.corpus / "regeneration.json"),
        "quprs_revision": PIN,
        "cohorts": [],
    }

    def save():
        (a.out / "replay.json").write_text(json.dumps(report, indent=2) + "\n")

    save()
    for label, _, _, expected, bits in CASES:
        family = label.rsplit("-", 1)[0]
        root = a.corpus / family
        cnf, = root.glob("*-gpmc.cnf")
        refpath = root / "independent-reference.json"
        ref = json.loads(refpath.read_text())
        src, = [x for x in regen["sources"] if x["family"] == family]
        cert, = [x for x in regen["cohorts"] if x["label"] == label]
        digits, target = DIGITS[bits]
        assert src["status"] == "SOURCE_EXACT" and src["cnf_sha256"] == expected
        assert digest(cnf) == ref["source_sha256"] == expected
        assert cert["status"] == "PASS" and cert["bits"] == bits and cert["target"] == target
        assert cert["reference_sha256"] == digest(refpath)
        assert ref["status"] in ("INDEPENDENT_INTERVAL_COMPLETE", "INDEPENDENT_FIXED_INTERVAL_COMPLETE")
        assert ref["bits"] == 550 and ref["width"] <= 19 and ref["work"] <= 40000000 and ref["peak"] <= 2000000
        if ref["status"] == "INDEPENDENT_FIXED_INTERVAL_COMPLETE":
            assert ref["order_policy"] in ("fill-reverse", "degree-reverse")
        proc = subprocess.run(
            ["timeout", "-k", "2s", "60s", "prlimit", "--as=3221225472", "--",
             str(a.bin.resolve()), "--digits", str(digits), str(cnf.resolve())],
            capture_output=True, text=True, timeout=65,
        )
        (a.out / (label + ".stdout")).write_text(proc.stdout)
        (a.out / (label + ".stderr")).write_text(proc.stderr)
        assert proc.returncode == 0, (label, proc.returncode, proc.stderr[-500:])
        sat, count = proc.stdout.splitlines()
        fields = count.split()
        assert sat in ("s SATISFIABLE", "s UNSATISFIABLE")
        assert len(fields) == 5 and fields[:3] == ["c", "r", "mpfr"]
        assert re.findall(r"^RAWMOCS_COMPLETE route=prejoin digits=(\d+) bits=(\d+) ",
                          proc.stderr, re.M) == [(str(digits), str(bits))]
        row = {"sat": sat == "s SATISFIABLE", "real": fields[3], "imag": fields[4]}
        upper = check_count(row, ref, target)
        assert digest(cnf) == expected and digest(refpath) == cert["reference_sha256"]
        report["cohorts"].append({
            "label": label, "bits": bits, "target": target, "source_sha256": expected,
            "reference_sha256": digest(refpath), "witness_upper": upper, "status": "PASS",
        })
        save()
        print("FAST_WITNESS_PASS", label, flush=True)
    report["status"] = "REPLAY_COMPLETE"
    save()
    print("FAST_WITNESS_COMPLETE", len(report["cohorts"]), flush=True)


if __name__ == "__main__":
    main()
