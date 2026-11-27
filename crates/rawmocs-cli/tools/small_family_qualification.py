#!/usr/bin/env python3
"""Qualify small original same-family cases; not a release or timing receipt.

Keep original QASM bytes, exporter and independent oracle pinned. A successful
numeric solve alone does not prove that a targeted structural branch ran.
"""
import argparse
import json
import re
import subprocess
import sys
import time
from hashlib import sha256
from pathlib import Path

from regenerate_release import PIN
from release_transfer import check_count

LEGACY = "benchmarks/MQTBench/Random_Rotation/"
NEW = "benchmarks/MQTBench/qiskit-2.6.1-opt0/Random_Rotation/"
CASES = (
    ("qpeexact8", LEGACY + "qpeexact_nativegates_ibm_qiskit_opt0_8.qasm", "01078415cf504c48fa770c5eea3fa9ccdd22fecd"),
    ("qpeinexact8", LEGACY + "qpeinexact_nativegates_ibm_qiskit_opt0_8.qasm", "a8e46601cda036d0481a2d7686bae928547563be"),
    ("qnn4", LEGACY + "qnn_nativegates_ibm_qiskit_opt0_4.qasm", "fed46a5b63719525b28be193aa0a43ab9d10c6d7"),
    ("grover4", NEW + "grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm", "464be4c1781d781a02549af6fa29ca510fe2a869"),
    ("wstate4", NEW + "wstate_nativegates_ibm_qiskit_opt0_4.qasm", "c06d0f50c9435a8c8c98f5390758173d7b950fe9"),
    ("gf2_4", "benchmarks/Feynman/gf2^4_mult.qasm", "ff3e384fd36cea71f4ca74b673cc570bac11a40f"),
)
# Observed baseline, not a permanent solver plan contract. A later algorithm
# change must explicitly requalify a changed signature, not silently skip it.
# C1 run 45974883395, artifact 15187440596 (ZIP SHA256 verified).
BASELINE = {
    "qpeexact8": ("f778c73bf8f672319ad8df8f0b5b846b16856e87afaa8ff7062c1a12b75a6b61",
        "00f15ebed71b50746e11ececde9915ec19a46af1fac203d23e27877d814b14f9", (11, 0, 1, 7, 6588, 1840, 21)),
    "qpeinexact8": ("bfebcec3d3d9940276bf5f49c22e112c857cf1ddb1c156776bea501759885b55",
        "e8cbf3f1061558060fe8f9fb551ab471d172ce650b15df4db8cef809d66355a4", (10, 0, 1, 7, 5626, 1608, 19)),
    "qnn4": ("072e617672c170d77584c7985360b76e0c9b8f8946e6d05e68cd6c0ad79f2c66",
        "3534213355e8a6afe162068a3f04c694bc5f134b463b120a2eb2f629feb10805", (18, 0, 1, 8, 14580, 2904, 22)),
    "grover4": ("43557c87d1830c4bda99b956e2e16ade646310b0b89f8a4f9dc3b11e90935db2",
        "73381a72b8b3c7f5f2c00c0ef44af75614605e45867863d6817c1e77f68fb7cf", (11, 0, 1, 6, 7066, 2454, 9)),
    "wstate4": ("ddb60973eb84c7231f14cb2d3422134161122ff14c5ad02da9794b030462d0b3",
        "6c9c881540cedc49512fda8496f3810f6ba54e4d2d98fcbc19b52da31f5d5b6e", (6, 0, 1, 5, 4664, 1710, 9)),
    "gf2_4": ("f3a787e9044b62fa616f39a2520ec0661f7233b1d9aa21404eb1ce87200e81af",
        "4224b0085d04055a0a3f54c8a8c4999c7f70a11f8da4f7ac330333b2af4e3810", (21, 0, 1, 11, 44678, 6680, 12)),
}
RUNS = ((60, 200, 30, "auto"), (130, 432, 100, "auto"),
        (60, 200, 30, "auto-frontier"), (60, 200, 30, "auto-separator"))


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def git(path, *args):
    return subprocess.check_output(["git", "-C", str(path), *args], text=True).strip()


def bounded(argv, seconds, root, name):
    start = time.monotonic_ns()
    result = subprocess.run(
        ["timeout", "-k", "2s", str(seconds), "prlimit", "--as=3221225472",
         "--", *map(str, argv)],
        capture_output=True, text=True, timeout=seconds + 5,
    )
    (root / (name + ".stdout")).write_text(result.stdout)
    (root / (name + ".stderr")).write_text(result.stderr)
    return result, time.monotonic_ns() - start


def qualify(args, label, rel, blob, row, save):
    root = args.out / label
    root.mkdir()
    assert git(args.quprs, "rev-parse", "HEAD:" + rel) == blob
    case = "extended_v022_" + label
    export, elapsed = bounded([args.exporter, case, args.quprs, root, rel], 20, root, "export")
    row["export_ns"] = elapsed
    assert export.returncode == 0, ("export", export.returncode)
    qasm, cnf = root / (case + ".qasm"), root / (case + "-gpmc.cnf")
    assert qasm.read_bytes() == (args.quprs / rel).read_bytes()
    source = digest(cnf)
    row.update(qasm_sha256=digest(qasm), cnf_sha256=source, cnf_bytes=cnf.stat().st_size)
    expected_qasm, expected_cnf, expected_plan = BASELINE[label]
    assert row["qasm_sha256"] == expected_qasm and source == expected_cnf, "qualified source drift"
    save()
    oracle = Path(__file__).resolve().parent / "repro" / "original_fixed_interval.py"
    result, elapsed = bounded([sys.executable, oracle, cnf, source], 25, root, "oracle")
    row["oracle_ns"] = elapsed
    assert result.returncode == 0, ("oracle", result.returncode)
    ref = json.loads(result.stdout)
    assert ref["source_sha256"] == source and ref["bits"] == 550
    assert ref["order_policy"] in ("fill-reverse", "degree-reverse")
    assert ref["width"] <= 19 and ref["work"] <= 40000000 and ref["peak"] <= 2000000
    (root / "independent-reference.json").write_text(json.dumps(ref, indent=2) + "\n")
    row.update(reference_sha256=digest(root / "independent-reference.json"),
               variables=ref["variables"], clauses=ref["clauses"],
               oracle_width=ref["width"], oracle_work=ref["work"], oracle_peak=ref["peak"])
    save()
    for digits, bits, target, mode in RUNS:
        trial = {"digits": digits, "bits": bits, "target_digits": target, "mode": mode,
                 "status": "IN_PROGRESS"}
        row["trials"].append(trial)
        name = f"{mode}-{bits}"
        result, elapsed = bounded([args.bin, "--digits", digits, "--solver", mode,
                                   "--timeout-ms", "5000", cnf], 8, root, name)
        trial.update(wall_ns=elapsed, exit_code=result.returncode)
        try:
            assert result.returncode == 0, ("solver", result.returncode)
            lines = result.stdout.splitlines()
            assert len(lines) == 2 and lines[0] in ("s SATISFIABLE", "s UNSATISFIABLE")
            fields = lines[1].split()
            assert len(fields) == 5 and fields[:3] == ["c", "r", "mpfr"]
            (route, reported_digits, reported_bits), = re.findall(
                r"^RAWMOCS_COMPLETE route=([^ ]+) digits=(\d+) bits=(\d+) ", result.stderr, re.M)
            assert (int(reported_digits), int(reported_bits)) == (digits, bits)
            count = {"sat": lines[0] == "s SATISFIABLE", "real": fields[3], "imag": fields[4]}
            upper = check_count(count, ref, target)
            assert digest(cnf) == source
            traces = [line for line in result.stderr.splitlines()
                      if line.startswith(("PREJOIN_", "FRONTIER_", "SEPARATOR_", "GENERAL_"))]
            plan, = re.findall(
                r"^PREJOIN_COMPLETE residual=(\d+) cuts=(\d+) leaves=(\d+) width=(\d+) work=(\d+) peak=(\d+) joins=(\d+) plan_ns=\d+$",
                result.stderr, re.M)
            plan = tuple(map(int, plan))
            assert route == "prejoin" and plan == expected_plan, "C1 baseline route/plan changed"
            assert count["sat"], "C1 SAT support changed"
            trial.update(status="PASS", route=route, count=count, witness_upper=upper,
                         plan_signature=plan, traces=traces)
        except (AssertionError, ValueError) as exc:
            trial.update(status="NOT_ADMITTED", error=str(exc))
        save()
    row["status"] = "NUMERIC_PASS" if all(t["status"] == "PASS" for t in row["trials"]) else "NOT_ADMITTED"
    print("SMALL_FAMILY_RESULT", label, row["status"],
          [(t["mode"], t["bits"], t.get("route"), t["status"]) for t in row["trials"]], flush=True)


def main():
    p = argparse.ArgumentParser()
    for name in ("quprs", "exporter", "bin", "out"):
        p.add_argument("--" + name, type=Path, required=True)
    a = p.parse_args()
    for name in ("quprs", "exporter", "bin", "out"):
        setattr(a, name, getattr(a, name).resolve())
    assert git(a.quprs, "rev-parse", "HEAD") == PIN
    a.out.mkdir(parents=True, exist_ok=False)
    report = {"status": "IN_PROGRESS", "quprs_revision": PIN, "checkout": git(Path("."), "rev-parse", "HEAD"),
              "binary_sha256": digest(a.bin), "exporter_sha256": digest(a.exporter),
              "scope": "small-family qualification only; no historical release gate replacement",
              "cases": []}

    def save():
        (a.out / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")

    save()
    for label, rel, blob in CASES:
        row = {"label": label, "source_path": rel, "qasm_blob": blob, "status": "IN_PROGRESS", "trials": []}
        report["cases"].append(row)
        save()
        try:
            qualify(a, label, rel, blob, row, save)
        except (AssertionError, ValueError, subprocess.SubprocessError, OSError) as exc:
            row.update(status="NOT_ADMITTED", error=str(exc))
            print("SMALL_FAMILY_RESULT", label, row["status"], str(exc), flush=True)
        save()
    report["status"] = "NUMERIC_PASS" if all(r["status"] == "NUMERIC_PASS" for r in report["cases"]) else "NOT_ADMITTED"
    save()
    print("SMALL_FAMILY_QUALIFICATION", report["status"],
          sum(r["status"] == "NUMERIC_PASS" for r in report["cases"]), len(CASES), flush=True)
    if report["status"] != "NUMERIC_PASS":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
