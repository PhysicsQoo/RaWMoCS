#!/usr/bin/env python3
"""Short original-QPE8 release CLI/GPMC Simulation; not a long-case timing claim."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time

from release_native_paired import parse, GPMC_PIN
from release_transfer import check_count
from regenerate_release import PIN

ROOT = Path(".release-simulation")
CASE = "extended_qpe8_prejoin_focus"
REL = "benchmarks/MQTBench/Random_Rotation/qpeexact_nativegates_ibm_qiskit_opt0_8.qasm"
QASM_BLOB = "01078415cf504c48fa770c5eea3fa9ccdd22fecd"
EXPECTED = "00f15ebed71b50746e11ececde9915ec19a46af1fac203d23e27877d814b14f9"
ARMS = ("gpmc", "rawmocs")
DIGITS, BITS, TARGET = 60, 200, 30


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def git(path, *args):
    return subprocess.check_output(["git", "-C", str(path), *args], text=True).strip()


def checked(cmd, seconds):
    return subprocess.run(["timeout", "-k", "2s", str(seconds), "prlimit",
                           "--as=3221225472", "--", *map(str, cmd)],
                          capture_output=True, text=True, timeout=seconds + 5)


def inspect(arm, stdout, stderr, reference):
    row = parse(stdout, arm)
    row["witness_upper"] = check_count(row, reference, TARGET)
    if arm == "rawmocs":
        assert re.findall(r"^RAWMOCS_COMPLETE route=prejoin digits=(\d+) bits=(\d+) ", stderr, re.M) == [(str(DIGITS), str(BITS))]
        trace, = re.findall(r"^PREJOIN_COMPLETE residual=(\d+) cuts=(\d+) leaves=(\d+) width=(\d+) work=(\d+) peak=(\d+) joins=(\d+) plan_ns=(\d+)$", stderr, re.M)
        row["plan_signature"] = list(map(int, trace[:7]))
    return row


def save(report):
    (ROOT / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")


def prepare():
    q = Path(".upstream/quprs")
    g = Path(".upstream/gpmc")
    assert git(q, "rev-parse", "HEAD") == PIN
    assert git(g, "rev-parse", "HEAD") == GPMC_PIN
    assert git(q, "rev-parse", f"HEAD:{REL}") == QASM_BLOB
    ROOT.mkdir(parents=True, exist_ok=True)
    cnf = ROOT / (CASE + "-gpmc.cnf")
    result = checked([q / "rust/target/release/examples/quprs_long_tail_source_bridge",
                      CASE, q.resolve(), ROOT.resolve(), REL], 30)
    assert result.returncode == 0, result.stderr
    assert (ROOT / (CASE + ".qasm")).read_bytes() == (q / REL).read_bytes()
    assert digest(cnf) == EXPECTED
    oracle = Path(__file__).resolve().parent / "repro/original_fixed_interval.py"
    result = checked([sys.executable, oracle, cnf, EXPECTED], 45)
    assert result.returncode == 0, result.stderr
    reference = json.loads(result.stdout)
    assert reference["source_sha256"] == EXPECTED and reference["bits"] == 550
    assert reference["width"] <= 19 and reference["work"] <= 40000000 and reference["peak"] <= 2000000
    reference["status"] = "INDEPENDENT_FIXED_INTERVAL_COMPLETE"
    assert reference["order_policy"] in ("fill-reverse", "degree-reverse")
    binaries = {"gpmc": g / "build/gpmc", "rawmocs": Path("crates/rawmocs-cli/target/release/rawmocs")}
    cmds = {"gpmc": [str(binaries["gpmc"]), "-mode=1", "-prec=60", str(cnf)],
            "rawmocs": [str(binaries["rawmocs"]), "--digits", "60", str(cnf)]}
    report = {"status": "NATIVE_PREFLIGHT_PENDING", "checkout": git(Path("."), "rev-parse", "HEAD"),
              "source_sha256": EXPECTED, "qasm_blob": QASM_BLOB, "quprs": PIN, "gpmc": GPMC_PIN,
              "working_digits": DIGITS, "bits": BITS, "target_digits": TARGET,
              "reference": reference, "binary_sha256": {arm: digest(p) for arm, p in binaries.items()},
              "native": {}, "scope": "short original QPE8 only; not long-case wall time"}
    save(report)
    config = ["options:", '  max-time: "5s"', "benchmarks:"]
    for arm in ARMS:
        start = time.perf_counter_ns()
        result = checked(cmds[arm], 1)
        elapsed = time.perf_counter_ns() - start
        (ROOT / (arm + ".native.stdout")).write_text(result.stdout)
        (ROOT / (arm + ".native.stderr")).write_text(result.stderr)
        assert result.returncode == 0 and 0 < elapsed <= 1_000_000_000, (arm, elapsed, result.stderr[-1000:])
        report["native"][arm] = inspect(arm, result.stdout, result.stderr, reference)
        report["native"][arm]["wall_ns"] = elapsed
        save(report)
        wrapper = ROOT / (arm + ".sh")
        stdout = ROOT / (arm + ".simulation.stdout")
        stderr = ROOT / (arm + ".simulation.stderr")
        stdout.unlink(missing_ok=True)
        stderr.unlink(missing_ok=True)
        wrapper.write_text("#!/bin/sh\nexec " + shlex.join(cmds[arm]) +
                           " >" + shlex.quote(str(stdout)) + " 2>" + shlex.quote(str(stderr)) + "\n")
        wrapper.chmod(0o755)
        config.extend([f'  - name: "release_original/qpe8/{arm}_mpfr200"', f'    exec: "./{wrapper}"'])
    Path("codspeed-release.generated.yml").write_text("\n".join(config) + "\n")
    report["status"] = "NATIVE_PREFLIGHT_PASS"
    save(report)
    print("RELEASE_SIMULATION_PREFLIGHT_PASS", flush=True)


def verify():
    report = json.loads((ROOT / "qualification.json").read_text())
    assert report["status"] == "NATIVE_PREFLIGHT_PASS"
    assert report["checkout"] == git(Path("."), "rev-parse", "HEAD")
    assert digest(ROOT / (CASE + "-gpmc.cnf")) == EXPECTED
    assert digest(Path(".upstream/gpmc/build/gpmc")) == report["binary_sha256"]["gpmc"]
    assert digest(Path("crates/rawmocs-cli/target/release/rawmocs")) == report["binary_sha256"]["rawmocs"]
    report["status"] = "SIMULATION_OUTPUT_INCONCLUSIVE"
    report["simulation"] = {}
    try:
        for arm in ARMS:
            row = inspect(arm, (ROOT / (arm + ".simulation.stdout")).read_text(),
                          (ROOT / (arm + ".simulation.stderr")).read_text(), report["reference"])
            report["simulation"][arm] = row
            assert all(row[k] == report["native"][arm][k] for k in row)
        report["status"] = "SIMULATION_OUTPUT_ROUTE_PASS"
        print("RELEASE_SIMULATION_OUTPUT_ROUTE_PASS", flush=True)
    finally:
        save(report)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("stage", choices=("prepare", "verify"))
    (prepare if ap.parse_args().stage == "prepare" else verify)()
