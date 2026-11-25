#!/usr/bin/env python3
"""Actual solver kernel-OOM admission in an isolated cgroup-v2 service.

Run as the ordinary CI user inside a memory-limited systemd service.
The supervisor remains in the same group and records kernel event deltas.
Neither a signal alone nor an address-space allocation failure proves OOM.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    binary = a.bin.resolve(strict=True)
    out = a.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    rel, = [x.split("::", 1)[1] for x in Path("/proc/self/cgroup").read_text().splitlines() if x.startswith("0::")]
    assert rel != "/" and "rawmocs-release-oom-" in rel, rel
    group = Path("/sys/fs/cgroup") / rel.lstrip("/")
    limit = int((group / "memory.max").read_text())
    assert limit == 48 * 1024 * 1024
    assert (group / "memory.swap.max").read_text().strip() == "0"
    assert (group / "memory.oom.group").read_text().strip() == "0"

    def events():
        return {k: int(v) for k, v in (line.split() for line in (group / "memory.events.local").read_text().splitlines())}

    receipt = {"status": "IN_PROGRESS", "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
               "cgroup": rel, "memory_max": limit, "swap_max": 0, "controls": []}

    def save():
        (out / "oom-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")

    save()
    fifo = out / "kill-control.fifo"
    os.mkfifo(fifo)
    before = events()
    proc = subprocess.Popen([str(binary), "--digits", "60", str(fifo)],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            assert proc.poll() is None
            try:
                if Path(f"/proc/{proc.pid}/exe").resolve(strict=True) == binary:
                    break
            except FileNotFoundError:
                pass
            time.sleep(0.01)
        else:
            raise AssertionError("solver identity not observed")
        os.killpg(proc.pid, signal.SIGKILL)
        stdout, stderr = proc.communicate(timeout=5)
        after = events()
        receipt["controls"].append({"case": "non_oom_sigkill", "returncode": proc.returncode,
                                    "before": before, "after": after, "stdout_bytes": len(stdout)})
        save()
        assert proc.returncode == -signal.SIGKILL and not stdout
        assert after["oom_kill"] == before["oom_kill"]
        print("NON_OOM_SIGKILL_PASS", flush=True)
    finally:
        if proc.poll() is None:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.communicate(timeout=5)
        fifo.unlink()

    # Deliberately deep legal CNF, not a synthetic allocator or fault binary.
    # Many retained residual frames consume real solver memory.
    n = 4096
    cnf = out / "oom-search.cnf"
    cnf.write_text(f"p cnf {n} 1\nc t wmc\n" + " ".join(map(str, range(1, n + 1))) + " 0\n")
    before = events()
    proc = subprocess.Popen([str(binary), "--digits", "60", "--solver", "general",
                             "--timeout-ms", "30000", str(cnf)],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = proc.communicate(timeout=35)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        proc.communicate(timeout=5)
        receipt["status"] = "EXTERNAL_TIMEOUT_NOT_OOM"
        receipt["events_after_timeout"] = events()
        save()
        raise
    after = events()
    (out / "oom.stdout").write_bytes(stdout)
    (out / "oom.stderr").write_bytes(stderr)
    receipt["controls"].append({"case": "actual_general_search_oom", "returncode": proc.returncode,
                                "before": before, "after": after, "stdout_bytes": len(stdout),
                                "source_sha256": hashlib.sha256(cnf.read_bytes()).hexdigest()})
    save()
    assert proc.returncode == -signal.SIGKILL and not stdout, (proc.returncode, stderr[-500:])
    assert after["oom_kill"] > before["oom_kill"], (before, after)
    receipt["status"] = "KERNEL_OOM_AND_NEGATIVE_CONTROL_PASS"
    save()
    print("KERNEL_OOM_PASS", before["oom_kill"], after["oom_kill"], flush=True)


if __name__ == "__main__":
    main()
