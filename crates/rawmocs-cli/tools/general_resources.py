#!/usr/bin/env python3
"""Linux process lifecycle and deep general-route release admission.

External deadline/cancel controls block the actual executable in input open.
They test supervisor-observed process lifecycle, not active-search cancellation
latency or kernel OOM attribution. No failure is interpreted as a WMC value.
"""
import argparse
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    binary = args.bin.resolve()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    report = {"status": "IN_PROGRESS", "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "controls": []}

    def record(row):
        report["controls"].append(row)
        (out / "general-resources.json").write_text(json.dumps(report, indent=2) + "\n")

    # Only one nonzero-weight model: variable 1 true, all others false.
    # Nevertheless Boolean SAT and all branches must be counted, and the
    # positive-before-negative chronological search has >512 live frames.
    n = 600
    cnf = out / "deep.cnf"
    cnf.write_text(
        f"p cnf {n} 1\nc t wmc\n"
        + "".join(f"c p weight {v} 0 0 0\n" for v in range(2, n + 1))
        + " ".join(map(str, range(1, n + 1))) + " 0\n"
    )
    result = subprocess.run(
        ["prlimit", "--as=3221225472", "--", str(binary), "--digits", "60",
         "--timeout-ms", "20000", str(cnf)],
        capture_output=True, text=True, timeout=30,
    )
    (out / "deep.stdout").write_text(result.stdout)
    (out / "deep.stderr").write_text(result.stderr)
    assert result.returncode == 0, result.stderr
    lines = result.stdout.splitlines()
    assert len(lines) == 2 and lines[0] == "s SATISFIABLE"
    fields = lines[1].split()
    assert len(fields) == 5 and fields[:3] == ["c", "r", "mpfr"]
    assert Fraction(fields[3]) == 1 and Fraction(fields[4]) == 0
    frames, = re.findall(r"^RAWMOCS_GENERAL_COMPLETE max_frames=(\d+)$", result.stderr, re.M)
    assert int(frames) > 512
    assert "RAWMOCS_COMPLETE route=general" in result.stderr
    record({"case": "deep-600", "max_frames": int(frames), "count": ["1", "0"], "status": "PASS"})
    print("DEEP_GENERAL_PASS", frames, flush=True)

    for mode in ("EXTERNAL_TIMEOUT", "CANCELLED"):
        fifo = out / (mode.lower() + ".fifo")
        os.mkfifo(fifo)
        proc = subprocess.Popen(
            ["prlimit", "--as=3221225472", "--", str(binary), "--digits", "60", str(fifo)],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
        )
        try:
            # Wait until the real executable replaced prlimit. No writer
            # opens the FIFO; source read cannot complete before termination.
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                assert proc.poll() is None, "unexpected premature exit"
                try:
                    if Path(f"/proc/{proc.pid}/exe").resolve(strict=True) == binary:
                        break
                except FileNotFoundError:
                    pass
                time.sleep(0.01)
            else:
                raise AssertionError("executable identity not observed")
            if mode == "EXTERNAL_TIMEOUT":
                try:
                    proc.communicate(timeout=0.2)
                except subprocess.TimeoutExpired:
                    pass
                else:
                    raise AssertionError("external deadline did not expire")
            os.killpg(proc.pid, signal.SIGTERM)
            stdout, stderr = proc.communicate(timeout=5)
            assert proc.returncode == -signal.SIGTERM and not stdout
            (out / (mode.lower() + ".stderr")).write_bytes(stderr)
            record({"case": mode, "status": "PASS", "returncode": proc.returncode,
                    "stdout_bytes": len(stdout), "evidence": "supervisor action; input-open blocked"})
            print("GENERAL_LIFECYCLE_PASS", mode, flush=True)
        finally:
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.communicate(timeout=5)
            fifo.unlink()
    report["status"] = "DEEP_AND_LIFECYCLE_COMPLETE"
    report["oom_status"] = "NOT_RUN"
    (out / "general-resources.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
