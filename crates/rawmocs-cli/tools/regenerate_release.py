#!/usr/bin/env python3
"""Rebuild admitted original CNFs and independent witnesses without artifacts."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import subprocess
import sys

from release_transfer import CASES, check_count

PIN = "0b28736651b3740ed3b201edd2efdef3da541e0d"
# The historical scout IDs encode the original relative path SHA256 prefix.
# Resolve against the pinned Git tree, never a new generated or transpiled QASM.
SOURCE = {
    "qpeinexact22": ("extended_mqt_qpeinexact_22_6befed5d0733", "6befed5d0733"),
    "qpeexact22": ("extended_mqt_qpeexact_22_e26adf85ca78", "e26adf85ca78"),
    "qnn12": ("extended_mqt_qnn_12_6d079756c70f", "6d079756c70f"),
    "grover6": ("extended_mqt_grover6_prejoin_qiskit261", "benchmarks/MQTBench/qiskit-2.6.1-opt0/Random_Rotation/grover-noancilla_nativegates_ibm_qiskit_opt0_6.qasm"),
    "wstate16": ("extended_mqt_wstate16_prejoin_family", "benchmarks/MQTBench/qiskit-2.6.1-opt0/Random_Rotation/wstate_nativegates_ibm_qiskit_opt0_16.qasm"),
    "gf2-7": ("extended_gf2_7_prejoin_scout", "benchmarks/Feynman/gf2^7_mult.qasm"),
}
DIGITS = {200: (60, 30), 333: (100, 100), 432: (130, 100)}


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def bounded(argv, seconds):
    return subprocess.run(["timeout", "-k", "2s", str(seconds), "prlimit", "--as=3221225472", "--", *map(str, argv)],
                          capture_output=True, text=True, timeout=seconds + 5)


def main():
    ap = argparse.ArgumentParser()
    for name in ("quprs", "exporter", "bin", "out"):
        ap.add_argument("--" + name, type=Path, required=True)
    a = ap.parse_args()
    q = a.quprs.resolve()
    out = a.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    assert subprocess.check_output(["git", "-C", str(q), "rev-parse", "HEAD"], text=True).strip() == PIN
    tree = subprocess.check_output(["git", "-C", str(q), "ls-tree", "-r", "--name-only", "HEAD"], text=True).splitlines()
    oracle = Path(__file__).resolve().parent / "repro"
    report = {"status": "IN_PROGRESS", "quprs_revision": PIN,
              "binary_sha256": digest(a.bin), "sources": [], "cohorts": []}

    def save():
        (out / "regeneration.json").write_text(json.dumps(report, indent=2) + "\n")

    save()
    built = {}
    for label, _, _, expected, bits in CASES:
        family = label.rsplit("-", 1)[0]
        case, selector = SOURCE[family]
        if family not in built:
            if selector.startswith("benchmarks/"):
                rel = selector
                assert rel in tree
            else:
                rel, = [p for p in tree if p.startswith("benchmarks/MQTBench/") and p.endswith(".qasm")
                        and sha256(p.encode()).hexdigest()[:12] == selector]
            root = out / family
            root.mkdir()
            exported = bounded([a.exporter.resolve(), case, q, root, rel], 30)
            (root / "export.stdout").write_text(exported.stdout)
            (root / "export.stderr").write_text(exported.stderr)
            assert exported.returncode == 0, (family, exported.stderr[-500:])
            cnf = root / (case + "-gpmc.cnf")
            qasm = root / (case + ".qasm")
            assert qasm.read_bytes() == (q / rel).read_bytes()
            assert digest(cnf) == expected, (family, digest(cnf), expected)
            report["sources"].append({"family": family, "path": rel, "qasm_sha256": digest(qasm),
                                      "cnf_sha256": expected, "status": "SOURCE_EXACT"})
            save()
            print("REGENERATED_SOURCE_PASS", family, rel, expected, flush=True)
            ref_run = bounded([sys.executable, oracle / "original_exact_prejoin.py", cnf, expected], 190)
            (root / "exact-oracle.stdout").write_text(ref_run.stdout)
            (root / "exact-oracle.stderr").write_text(ref_run.stderr)
            if ref_run.returncode == 0:
                ref = json.loads(ref_run.stdout)
                assert ref["status"] == "INDEPENDENT_INTERVAL_COMPLETE"
            else:
                ref_run = bounded([sys.executable, oracle / "original_fixed_interval.py", cnf, expected], 190)
                (root / "fixed-oracle.stdout").write_text(ref_run.stdout)
                (root / "fixed-oracle.stderr").write_text(ref_run.stderr)
                assert ref_run.returncode == 0, (family, ref_run.stderr[-500:])
                ref = json.loads(ref_run.stdout)
                ref["status"] = "INDEPENDENT_FIXED_INTERVAL_COMPLETE"
                assert ref["order_policy"] in ("fill-reverse", "degree-reverse")
            assert ref["source_sha256"] == expected and ref["bits"] == 550
            assert ref["width"] <= 19 and ref["work"] <= 40000000 and ref["peak"] <= 2000000
            (root / "independent-reference.json").write_text(json.dumps(ref, indent=2) + "\n")
            built[family] = (root, cnf, ref)
        root, cnf, ref = built[family]
        digits, target = DIGITS[bits]
        run = bounded([a.bin.resolve(), "--digits", digits, cnf], 60)
        (root / (label + ".stdout")).write_text(run.stdout)
        (root / (label + ".stderr")).write_text(run.stderr)
        assert run.returncode == 0, (label, run.stderr[-500:])
        lines = run.stdout.splitlines()
        assert len(lines) == 2 and lines[0] in ("s SATISFIABLE", "s UNSATISFIABLE")
        fields = lines[1].split()
        assert len(fields) == 5 and fields[:3] == ["c", "r", "mpfr"]
        row = {"sat": lines[0] == "s SATISFIABLE", "real": fields[3], "imag": fields[4]}
        assert digest(cnf) == expected
        bound = check_count(row, ref, target)
        report["cohorts"].append({"label": label, "bits": bits, "target": target, "witness_upper": bound,
                                  "reference_sha256": digest(root / "independent-reference.json"), "status": "PASS"})
        save()
        print("REGENERATED_WITNESS_PASS", label, flush=True)
    assert len(report["sources"]) == 6 and len(report["cohorts"]) == 7
    report["status"] = "REGENERATION_COMPLETE"
    save()


if __name__ == "__main__":
    main()
