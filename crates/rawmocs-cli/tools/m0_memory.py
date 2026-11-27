"""Source-locked M0 repeated and Heaptrack arms; no QuPRS imports."""
import argparse
import ctypes
from fractions import Fraction
import gc
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time

from release_transfer import check_count

CNF_SHA = "6fb69626195bee5b89ed3503ae8b1b4801d9267e7bd5ff3684af152afcd885e1"
REF_SHA = "e59e2741a69dbb13b4f6d0b44562ac689541c4bcf7c43da476006ec8f1b87512"
ROOT = Path(os.environ.get("M0_EVIDENCE_DIR", ".m0-evidence")).resolve()
BIN = Path(os.environ.get("M0_RUST_BIN", "crates/rawmocs-cli/target/release/examples/m0_repeat")).resolve()
FIELDS = ("sat", "real", "imag", "route", "bits", "real_sig", "real_exp",
          "real_negative_zero", "imag_sig", "imag_exp", "imag_negative_zero",
          "wall_ns", "glibc_inuse_bytes", "glibc_mmap_bytes",
          "glibc_free_bytes", "glibc_arena_bytes")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def worker(path):
    import rawmocs

    class MallInfo2(ctypes.Structure):
        _fields_ = [(k, ctypes.c_size_t) for k in
                    ("arena", "ordblks", "smblks", "hblks", "hblkhd", "usmblks",
                     "fsmblks", "uordblks", "fordblks", "keepcost")]

    libc = ctypes.CDLL("libc.so.6")
    libc.mallinfo2.restype = MallInfo2
    libc.mallinfo2.argtypes = []
    for line in sys.stdin:
        assert line.strip() == "solve"
        start = time.perf_counter_ns()
        n = rawmocs.solve_file(path, digits=130, timeout_ms=120000)
        a, b = n.real_binary, n.imag_binary
        fields = (str(n.sat).lower(), n.real, n.imag, n.route, n.precision_bits,
                  a.significand, a.exponent2, str(a.negative_zero).lower(),
                  b.significand, b.exponent2, str(b.negative_zero).lower())
        del n, a, b
        elapsed = time.perf_counter_ns() - start
        gc.collect()
        m = libc.mallinfo2()
        print("M0_ROW", *fields, elapsed, m.uordblks, m.hblkhd, m.fordblks, m.arena,
              sep="\t", flush=True)


def proc_memory(pid):
    status = Path(f"/proc/{pid}/status").read_text().splitlines()
    vals = {k: int(v.split()[0]) for k, v in
            (line.split(":", 1) for line in status if line.startswith(("VmRSS:", "VmHWM:")))}
    rollup = Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines()
    private = {k: int(v.split()[0]) for k, v in
               (line.split(":", 1) for line in rollup if line.startswith(("Private_Clean:", "Private_Dirty:")))}
    return dict(rss_kib=vals["VmRSS"], cumulative_hwm_kib=vals["VmHWM"],
                uss_kib=sum(private.values()))


def parse(line, ref):
    prefix, *values = line.rstrip("\n").split("\t")
    assert prefix == "M0_ROW" and len(values) == len(FIELDS), line
    row = dict(zip(FIELDS, values))
    assert row["sat"] in ("true", "false")
    row["sat"] = row["sat"] == "true"
    for k in ("bits", "real_exp", "imag_exp", "wall_ns", "glibc_inuse_bytes",
              "glibc_mmap_bytes", "glibc_free_bytes", "glibc_arena_bytes"):
        row[k] = int(row[k])
    assert row["bits"] == 432 and row["route"] == "prejoin"
    row["witness_upper"] = check_count(row, ref, 100)
    row["glibc_live_proxy_bytes"] = row["glibc_inuse_bytes"] + row["glibc_mmap_bytes"]
    return row


def signature(row):
    return {k: row[k] for k in FIELDS[:11]}


def launch(arm, path, ref, profile=False):
    cmd = [str(BIN), str(path)] if arm == "rust" else [
        sys.executable, str(Path(__file__).resolve()), "worker", str(path)]
    label = ("profile-" if profile else "repeat-") + arm
    if profile:
        cmd = ["heaptrack", "-o", str(ROOT / label), *cmd]
    rows = []
    with (ROOT / (label + ".stderr")).open("w") as err:
        proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=err, text=True, bufsize=1, start_new_session=True)
        try:
            # The profiler may use a wrapper PID; its RSS is not compared.
            for i in range(1 if profile else 12):
                proc.stdin.write("solve\n")
                proc.stdin.flush()
                end = time.monotonic() + (160 if profile else 30)
                while True:
                    remaining = end - time.monotonic()
                    if remaining <= 0:
                        raise TimeoutError(label)
                    if not select.select([proc.stdout], [], [], min(1, remaining))[0]:
                        continue
                    line = proc.stdout.readline()
                    assert line, f"{label} exited before receipt"
                    if line.startswith("M0_ROW\t"):
                        break
                    # Heaptrack may print an informational banner.
                    with (ROOT / (label + ".banner")).open("a") as banner:
                        banner.write(line)
                row = parse(line, ref)
                if not profile:
                    row.update(proc_memory(proc.pid))
                row["iteration"] = i + 1
                rows.append(row)
                (ROOT / (label + ".json")).write_text(json.dumps(rows, indent=2) + "\n")
                print("M0_ROW_RECEIPT", label, i+1, row.get("rss_kib"), row["glibc_live_proxy_bytes"], flush=True)
            proc.stdin.close()
            assert proc.wait(timeout=60 if profile else 5) == 0, label
        finally:
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait(timeout=5)
            proc.stdout.close()
    return rows


def main(stage):
    ROOT.mkdir(exist_ok=True)
    path, = Path(".m0-input").rglob("*-gpmc.cnf")
    witness, = Path(".m0-input").rglob("prejoin-reference.json")
    receipt, = Path(".m0-input").rglob("prejoin-native-pilot.json")
    assert sha(path) == CNF_SHA and sha(witness) == REF_SHA
    ref = json.loads(witness.read_text())
    prior = json.loads(receipt.read_text())
    assert ref["source_sha256"] == prior["source_sha256"] == CNF_SHA
    assert prior["reference_sha256"] == REF_SHA and prior["status"] == "REPEAT_COMPLETE"
    assert ref["bits"] == 550 and ref["status"] in (
        "INDEPENDENT_INTERVAL_COMPLETE", "INDEPENDENT_FIXED_INTERVAL_COMPLETE")
    assert ref["width"] <= 19 and ref["work"] <= 40000000 and ref["peak"] <= 2000000
    cohort, = [c for c in prior["cohorts"] if c["bits"] == 432]
    assert cohort["status"] == "SEVEN_MATCHED_PAIRS_PASS" and len(cohort["trials"]) == 7
    assert cohort["working_digits"] == 130 and cohort["target_digits"] == 100
    assert all(t["rows"][a]["status"] == "COMPLETED" and t["rows"][a]["witness"]["pass"]
               for t in cohort["trials"] for a in ("gpmc", "prejoin"))
    if stage in ("repeat", "repeat-rust"):
        arms = ("rust",) if stage == "repeat-rust" else ("rust", "python")
        rows = {a: launch(a, path, ref) for a in arms}
        expected = signature(rows["rust"][0])
        assert all(signature(row) == expected for rs in rows.values() for row in rs)
        summary = {}
        for a, rs in rows.items():
            def growth(k):
                return max(0, max(r[k] for r in rs[2:]) - max(r[k] for r in rs[:2]))
            summary[a] = dict(raw_rss_growth_kib=growth("rss_kib"),
                              uss_growth_kib=growth("uss_kib"),
                              live_proxy_growth_bytes=growth("glibc_live_proxy_bytes"),
                              max_free_bytes=max(r["glibc_free_bytes"] for r in rs),
                              max_cumulative_hwm_kib=max(r["cumulative_hwm_kib"] for r in rs))
        out = dict(status="M0_REPEAT_WITNESS_PASS", signature=expected, summary=summary,
                   cnf_sha256=CNF_SHA, witness_sha256=REF_SHA,
                   scope="diagnosis only; RSS review remains open; glibc live proxy not a leak proof")
        (ROOT / "repeat-summary.json").write_text(json.dumps(out, indent=2) + "\n")
        print("M0_REPEAT_FINAL", json.dumps(summary), flush=True)
    else:
        prior = json.loads((ROOT / "repeat-summary.json").read_text())
        arms = ("rust",) if stage == "profile-rust" else ("rust", "python")
        rows = {a: launch(a, path, ref, profile=True) for a in arms}
        assert all(signature(row) == prior["signature"] for rs in rows.values() for row in rs)
        print("M0_PROFILE_WITNESS_PASS", flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("stage", choices=("repeat", "profile", "repeat-rust", "profile-rust", "worker"))
    p.add_argument("path", nargs="?", type=Path)
    a = p.parse_args()
    if a.stage == "worker":
        worker(a.path)
    else:
        main(a.stage)
