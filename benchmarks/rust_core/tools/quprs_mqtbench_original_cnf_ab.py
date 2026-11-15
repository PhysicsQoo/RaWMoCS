#!/usr/bin/env python3
"""Original MQT Bench QASM -> unchanged QuPRS WMC CNF -> paired native solvers.

These are QuPRS-encoded CNFs derived from existing committed MQT Bench QASM;
they are NOT represented as MQT-published CNFs. Each source, encoder, dialect,
precision, native resource bound, and censored outcome remains visible.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
import json
from pathlib import Path
import statistics
import subprocess

from quprs_encoder_validate import parse_cnf
from quprs_long_feynman_pilot import native, sha_file

QU_PRS_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
ORIGINAL_MQT = {
    "mqt_wstate_4": "wstate_nativegates_ibm_qiskit_opt0_4.qasm",
    "mqt_vqe_4": "vqe_nativegates_ibm_qiskit_opt0_4.qasm",
    "mqt_qnn_4": "qnn_nativegates_ibm_qiskit_opt0_4.qasm",
    "mqt_grover_4": "grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm",
    "mqt_qwalk_3": "qwalk-noancilla_nativegates_ibm_qiskit_opt0_3.qasm",
    "mqt_qaoa_7": "qaoa_nativegates_ibm_qiskit_opt0_7.qasm",
    "mqt_qpeexact_8": "qpeexact_nativegates_ibm_qiskit_opt0_8.qasm",
}
DIGITS_TO_BITS = {30: 100, 100: 333}
ARMS = ("gpmc", "packed", "fast", "scratch")
MAX_SOURCE_BYTES = 2_000_000
MAX_ROTATION_ELIGIBLE_WALL_NS = 10_000_000_000  # Do not multiply slow native processes
TRIALS = 4


def invoke(case: str, digits: int, cnf: Path, budget: int, gpmc: Path, mpfr: Path, arm: str) -> dict:
    if arm == "gpmc":
        command = [str(gpmc.resolve()), "-mode=1", f"-prec={digits}", str(cnf.resolve())]
        tool = "gpmc"
    else:
        command = [str(mpfr.resolve()), "--node-budget=1000000", "--cache-limit=50000",
                   "--indexed-unit-prop-csr-after-8192",
                   "--packed-residual-key-fast" if arm in ("fast", "scratch") else "--packed-residual-key",
                   "--fast-assign"]
        if arm == "scratch":
            command.append("--reuse-frequency-scratch")
        command += ["--hashbrown-borrowed", str(digits), str(cnf.resolve())]
        tool = "rawmocs_mpfr"
    result = native(command, tool)
    result["arm"] = arm
    return result


def complex_relative_witness(reference: dict, candidate: dict, digits: int) -> dict:
    """Exact rational complex-parity witness; no binary float tolerance.

    A tiny imaginary component must not be judged against an unrelated
    absolute threshold when the same complex result has a much larger
    real component. Apply the same decimal-relative guard to both
    components, scaled by the maximum magnitude of EITHER component of
    the complete complex output. Preserve absolute deltas in evidence.
    """
    gr, gi = Fraction(reference["real"]), Fraction(reference["imag"])
    cr, ci = Fraction(candidate["real"]), Fraction(candidate["imag"])
    scale = max(Fraction(1), abs(gr), abs(gi), abs(cr), abs(ci))
    delta_real = abs(gr - cr)
    delta_imag = abs(gi - ci)
    relative = max(delta_real, delta_imag) / scale
    guard = Fraction(1, 10 ** (digits - 6))
    return {
        "pass": relative <= guard,
        "delta_real": str(delta_real),
        "delta_imag": str(delta_imag),
        "complex_component_scale": str(scale),
        "max_scaled_component_delta": str(relative),
        "relative_guard": f"1e-{digits - 6}",
    }


def certify(samples: list[dict], bits: int, digits: int) -> str:
    if any(s["status"] != "COMPLETED" for s in samples):
        return "CENSORED_NATIVE_OR_RESOURCE_LIMIT"
    gpmc = [s for s in samples if s["arm"] == "gpmc"]
    rust = [s for s in samples if s["arm"] != "gpmc"]
    if any(s.get("mpfr_precision_bits") != bits for s in rust):
        return "FAIL_MPFR_PRECISION"
    if len({s["sat"] for s in samples}) != 1:
        return "FAIL_SAT"
    if len({s["stdout_sha256"] for s in rust}) != 1:
        return "FAIL_RUST_FULL_COMPLEX_OUTPUT"
    if any(s.get("solve_ns") is None or s.get("engine_drop_ns") is None for s in rust):
        return "FAIL_PHASE_TIMING"
    # Comparisons are at exactly the same 50k capacity and original CNF.
    stats = {arm: {(s.get("dpll_nodes"), s.get("cache_hits"), s.get("component_splits"))
                   for s in rust if s["arm"] == arm} for arm in ("packed", "fast", "scratch")}
    if any(len(x) != 1 or None in next(iter(x)) for x in stats.values()):
        return "FAIL_DETERMINISTIC_SEARCH"
    if stats["packed"] != stats["fast"] or stats["fast"] != stats["scratch"]:
        return "FAIL_SAME_CAP_SEARCH_TOPOLOGY"
    if not all(complex_relative_witness(gpmc[0], s, digits)["pass"] for s in samples):
        return "FAIL_EQUAL_MPFR_GPMC_NUMERIC"
    return "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY"


def main() -> None:
    ap = argparse.ArgumentParser()
    for name in ("source", "gpmc-source", "corpus", "gpmc", "mpfr", "report"):
        ap.add_argument("--" + name, required=True, type=Path)
    args = ap.parse_args()
    if subprocess.check_output(["git", "-C", str(args.source), "rev-parse", "HEAD"], text=True).strip() != QU_PRS_SHA:
        raise SystemExit("UNPINNED_ORIGINAL_QU_PRS_SOURCE")
    if subprocess.check_output(["git", "-C", str(args.gpmc_source), "rev-parse", "HEAD"], text=True).strip() != GPMC_SHA:
        raise SystemExit("UNPINNED_ORIGINAL_GPMC_SOURCE")
    statuses = {}
    for line in (args.corpus / "source-status.tsv").read_text(encoding="utf8").splitlines():
        words = line.split("\t")
        if len(words) != 2 or words[0] not in ORIGINAL_MQT or words[0] in statuses:
            raise SystemExit("INVALID_MQT_SOURCE_STATUS_MANIFEST")
        statuses[words[0]] = words[1]
    if set(statuses) != set(ORIGINAL_MQT):
        raise SystemExit("MISSING_MQT_SOURCE_ATTEMPT")
    report = {
        "dataset": "ORIGINAL_MQT_BENCH_QASM_QU_PRS_EMITTED_WEIGHTED_CNF",
        "not_mqt_published_cnf": True,
        "quprs_source_sha": QU_PRS_SHA,
        "gpmc_source_sha": GPMC_SHA,
        "bits": DIGITS_TO_BITS,
        "arms": list(ARMS),
        "cache_capacity": 50_000,
        "node_budget": 1_000_000,
        "native_bounds": "30s / 2GiB individually per process",
        "statistical_design": "4 fresh native trials per arm in complete balanced rotation when eligible; otherwise censored",
        "oracle_caveat": "Original circuit and encoder provenance plus solver numeric agreement, NOT independent Full WMC certification",
        "cases": [],
    }
    complete = 0
    for case, basename in ORIGINAL_MQT.items():
        src = args.source / "benchmarks" / "MQTBench" / basename
        item = {"case": case, "source_relpath": str(src.relative_to(args.source)),
                "source_qasm_sha256": sha_file(src),
                "export_status": statuses[case], "results": []}
        if statuses[case] != "EXPORTED":
            item["status"] = "CENSORED_SOURCE_EXPORT"
            report["cases"].append(item)
            print(f"MQT_CNF_CENSORED case={case} status={item['status']}")
            continue
        copied = args.corpus / f"{case}.qasm"
        gfile = args.corpus / f"{case}-gpmc.cnf"
        kfile = args.corpus / f"{case}-ganak.cnf"
        gp = args.corpus / f"{case}-gpmc-phases.tsv"
        kp = args.corpus / f"{case}-ganak-phases.tsv"
        if not all(p.is_file() and p.stat().st_size <= MAX_SOURCE_BYTES
                   for p in (copied, gfile, kfile, gp, kp)):
            item["status"] = "CENSORED_MISSING_OR_OVERSIZE_OUTPUT"
            report["cases"].append(item)
            continue
        if src.read_bytes() != copied.read_bytes():
            raise SystemExit(f"MQT original QASM source modified: {case}")
        try:
            a, b = parse_cnf(gfile, "gpmc"), parse_cnf(kfile, "ganak")
        except ValueError as exc:
            item["status"] = "CENSORED_UNSUPPORTED_EMITTED_CNF"
            item["detail"] = str(exc)
            report["cases"].append(item)
            continue
        if ((a["variables"], a["clauses"], a["weights"])
                != (b["variables"], b["clauses"], b["weights"])
                or gp.read_bytes() != kp.read_bytes()):
            raise SystemExit(f"MQT dual-dialect/phase parity violated: {case}")
        item.update({
            "variables": a["variables"], "clauses": a["clauses_count"],
            "cnf_gpmc_sha256": sha_file(gfile),
            "cnf_ganak_sha256": sha_file(kfile),
            "phase_sidecar_sha256": sha_file(gp),
        })
        for digits, bits in DIGITS_TO_BITS.items():
            samples = []
            for trial in range(TRIALS):
                for arm in ARMS[trial:] + ARMS[:trial]:
                    row = invoke(case, digits, gfile, 1_000_000, args.gpmc, args.mpfr, arm)
                    row["trial"] = trial
                    samples.append(row)
                if trial == 0:
                    # Do not spend bounded CI minutes repeating censored
                    # or extremely slow native runs. Keep the raw probes.
                    if (any(s["status"] != "COMPLETED" for s in samples)
                            or any(s["wall_ns"] > MAX_ROTATION_ELIGIBLE_WALL_NS for s in samples)):
                        break
            status = certify(samples, bits, digits)
            trial_count = len(samples) // len(ARMS)
            if status == "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY" and trial_count < TRIALS:
                status = "COMPLETED_SINGLE_TRIAL_NO_PERFORMANCE_RANK"
            result = {"digits": digits, "bits": bits, "status": status,
                      "completed_rotations": trial_count, "samples": samples}
            if all(s["status"] == "COMPLETED" for s in samples):
                gpmc_reference = next(s for s in samples if s["arm"] == "gpmc")
                witnesses = [
                    complex_relative_witness(gpmc_reference, s, digits)
                    for s in samples if s["arm"] != "gpmc"
                ]
                result["worst_complex_numeric_witness"] = max(
                    witnesses, key=lambda x: Fraction(x["max_scaled_component_delta"])
                )
            if status == "MATCHED_MPFR_SOURCE_AGREEMENT_ONLY":
                complete += 1
                result["median_wall_ns"] = {
                    arm: statistics.median(x["wall_ns"] for x in samples if x["arm"] == arm)
                    for arm in ARMS
                }
                result["median_peak_rss_kib"] = {
                    arm: statistics.median(x["peak_rss_kib"] for x in samples if x["arm"] == arm)
                    for arm in ARMS
                }
                result["median_solve_ns"] = {
                    arm: statistics.median(x["solve_ns"] for x in samples if x["arm"] == arm)
                    for arm in ("packed", "fast", "scratch")
                }
                result["paired_fast_solve_speedup"] = statistics.median(
                    next(x["solve_ns"] for x in samples if x["arm"] == "packed" and x["trial"] == t)
                    / next(x["solve_ns"] for x in samples if x["arm"] == "fast" and x["trial"] == t)
                    for t in range(TRIALS)
                )
                result["paired_scratch_solve_speedup"] = statistics.median(
                    next(x["solve_ns"] for x in samples if x["arm"] == "fast" and x["trial"] == t)
                    / next(x["solve_ns"] for x in samples if x["arm"] == "scratch" and x["trial"] == t)
                    for t in range(TRIALS)
                )
                result["paired_scratch_wall_speedup"] = statistics.median(
                    next(x["wall_ns"] for x in samples if x["arm"] == "fast" and x["trial"] == t)
                    / next(x["wall_ns"] for x in samples if x["arm"] == "scratch" and x["trial"] == t)
                    for t in range(TRIALS)
                )
                print(
                    f"MQT_CNF_PAIRED case={case} bits={bits} vars={a['variables']} clauses={a['clauses_count']} "
                    + " ".join(f"{arm}_wall_ms={result['median_wall_ns'][arm]/1e6:.3f}" for arm in ARMS)
                    + " " + " ".join(f"{arm}_rss_kib={result['median_peak_rss_kib'][arm]}" for arm in ARMS)
                    + f" packed_solve_ms={result['median_solve_ns']['packed']/1e6:.3f}"
                    + f" fast_solve_ms={result['median_solve_ns']['fast']/1e6:.3f}"
                    + f" scratch_solve_ms={result['median_solve_ns']['scratch']/1e6:.3f}"
                    + f" paired_fast_solve={result['paired_fast_solve_speedup']:.4f}"
                    + f" paired_scratch_solve={result['paired_scratch_solve_speedup']:.4f}"
                    + f" paired_scratch_wall={result['paired_scratch_wall_speedup']:.4f}"
                )
            else:
                print(f"MQT_CNF_CENSORED case={case} bits={bits} status={status} rotations={trial_count}")
            item["results"].append(result)
        item["status"] = "ENCODED_CNF_WITH_PER_PRECISION_OUTCOMES"
        report["cases"].append(item)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if complete == 0:
        raise SystemExit("NO_COMPLETE_MQT_NATIVE_4_ARM_COMPARISON: no speed claim; inspect archived source/solver evidence")
    fatal = [r for c in report["cases"] for r in c["results"] if r["status"].startswith("FAIL_")]
    if fatal:
        raise SystemExit("MQT NUMERIC / SEARCH PARITY FAILURE: no speed claim")
    print(f"MQT_CNF_REPORT complete_precision_cases={complete} total_source_cases={len(ORIGINAL_MQT)}")


if __name__ == "__main__":
    main()
