#!/usr/bin/env python3
"""Source-pinned long-tail discovery, then strictly bounded original-CNF comparison.

This is a scout, NOT evidence that a large QASM is a hard WMC instance.
Every 100-bit original C++ GPMC and Rust screen is actually timed; >=10s
or a censored 30s screen is required before a candidate enters the pilot.
No solver or source changes, no synthetic CNF, no inferred timeout result.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "rust_core" / "tools"))
from quprs_cnf_factor_minfill_ab import build_command
from quprs_mqtbench_original_cnf_ab import complex_relative_witness
from quprs_paired_native import parse_out
from quprs_encoder_validate import parse_cnf

QU_PRS_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
GPMC_SHA = "89bbc89d0de918e7a5385740355df55765039230"
BITS = {30: 100, 100: 333}
ARMS = ("gpmc", "inline_full")
EXPORTED_CASES = 16
MIN_LONG_SECONDS = 10
EXPORT_S = 35
SCREEN_S = 30
PAIR_S = 120
PILOT_TRIALS = 2
MAX_PAIRED_LONG_CASES = 3
# Research-only large, but finite DPLL limit. This does NOT alter the
# production solver or the standalone research executable default.
# 30/120s and 3GiB native-process limits remain the binding controls.
LONG_TAIL_NODE_BUDGET = "--node-budget=1000000000"
AS_BYTES = 3_221_225_472
FS_BYTES = 16_777_216
CNF_MAX_BYTES = 12_000_000

def sha(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()

def git_sha(path: Path) -> str:
    return subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True
    ).strip()

def source_index(content: str) -> dict[str, str]:
    index = {}
    for line in content.splitlines():
        row = line.split("\t")
        if len(row) != 2:
            raise ValueError("INVALID_SOURCE_INDEX")
        name, rel = row
        path = Path(rel)
        if (not re.fullmatch(r"(feynman|mqt)_[a-z0-9_]+", name)
                or name in index or path.is_absolute() or ".." in path.parts
                or not rel.endswith(".qasm")
                or not (rel.startswith("benchmarks/Feynman/")
                        or rel.startswith("benchmarks/MQTBench/"))):
            raise ValueError("UNTRUSTED_OR_DUPLICATE_ORIGINAL_SOURCE")
        index[name] = rel
    if len(index) != EXPORTED_CASES:
        raise ValueError("UNEXPECTED_ORIGINAL_SOURCE_COUNT")
    return index

def command(args: argparse.Namespace, name: str, cnf: Path, digits: int, arm: str) -> list[str]:
    if arm not in ARMS or digits not in BITS:
        raise ValueError("INVALID_ARM_OR_PRECISION")
    # Reuse checked same-original-CNF flags. The research-only high node
    # budget is explicit, finite, and restricted to the native Rust arm.
    # GPMC and Rust still share the SAME full-process time/AS limits.
    cmd = build_command(args, arm, digits, cnf, name)
    if arm == "gpmc":
        if cmd[1:3] != ["-mode=1", f"-prec={digits}"]:
            raise ValueError("GPMC_NATIVE_CONTROL_DRIFT")
        if any(arg.startswith("--node-budget=") for arg in cmd):
            raise ValueError("GPMC_NONORIGINAL_NODE_FLAG")
    else:
        if any(arg.startswith("--node-budget=") for arg in cmd):
            raise ValueError("UNEXPECTED_PREEXISTING_RUST_NODE_CAP")
        cmd.insert(1, LONG_TAIL_NODE_BUDGET)
        if cmd[1] != LONG_TAIL_NODE_BUDGET or not all(x in cmd for x in
               ("--weighted-equivalence", "--trail-count",
                "--trail-inline-weight-relabel", "--hashbrown-borrowed")):
            raise ValueError("RUST_NATIVE_CONTROL_DRIFT")
    return cmd

def frontier_command(args: argparse.Namespace, name: str, cnf: Path, digits: int) -> list[str]:
    cmd = command(args, name, cnf, digits, "inline_full")
    position = cmd.index("--hashbrown-borrowed")
    cmd[position:position] = ["--factor-variable-elimination", "--factor-ve-indexed-fused", "--factor-ve-indexed-frontier"]
    return cmd


def classify_native_failure(returncode: int, stderr: str) -> str:
    # CLI cap is not a solver time measurement. Never label as OOM/timeout.
    if returncode == 124:
        return "CENSORED_TIME"
    if returncode == 2 and "RESEARCH_MPFR_FAIL_CLOSED: node limit" in stderr:
        return "CENSORED_NODE_BUDGET"
    if returncode == 2 and "RESEARCH_MPFR_FAIL_CLOSED: depth limit" in stderr:
        return "CENSORED_DEPTH_BUDGET"
    return "CENSORED_EXIT_OR_RESOURCE"


def native(cmd: list[str], arm: str, duration: int, digits: int | None) -> dict:
    with tempfile.TemporaryDirectory() as td:
        rss = Path(td) / "rss"
        wrapped = [
            "/usr/bin/timeout", "-k", "2s", f"{duration}s",
            "prlimit", f"--as={AS_BYTES}", f"--fsize={FS_BYTES}", "--",
            "/usr/bin/time", "-f", "%M", "-o", str(rss), *cmd,
        ]
        started = time.perf_counter_ns()
        try:
            process = subprocess.run(
                wrapped, capture_output=True, text=True, timeout=duration + 7
            )
        except subprocess.TimeoutExpired:
            return {"arm": arm, "status": "CENSORED_OUTER_TIMEOUT",
                    "wall_ns_lower_bound": duration * 1_000_000_000}
        except OSError as error:
            return {"arm": arm, "status": "CENSORED_LAUNCH_ERROR",
                    "error": str(error)[:250]}
        elapsed = time.perf_counter_ns() - started
        row = {"arm": arm, "wall_ns": elapsed,
               "exit_code": process.returncode,
               "status": ("COMPLETED" if process.returncode == 0 else
                          classify_native_failure(process.returncode, process.stderr)),
               "stderr_tail": process.stderr[-1000:]}
        if rss.is_file():
            try:
                row["peak_rss_kib"] = int(rss.read_text().strip())
            except ValueError:
                pass
        if process.returncode:
            return row
        if arm == "source":
            return row
        try:
            sat, (real, imag) = parse_out(
                process.stdout, "gpmc" if arm == "gpmc" else "rawmocs_mpfr"
            )
            row.update(sat=sat, real=str(real), imag=str(imag),
                       stdout_sha256=sha256(process.stdout.encode()).hexdigest())
            if arm != "gpmc":
                match = re.search(
                    r"^RESEARCH_MPFR_PHASE_NS read=(\d+) parse=(\d+) solve=(\d+) "
                    r"output=(\d+) nodes=(\d+) cache_hits=(\d+) decompositions=(\d+) "
                    r"digits=(\d+) bits=(\d+)$",
                    process.stderr, re.M,
                )
                if not match or int(match.group(8)) != digits or \
                        int(match.group(9)) != BITS[digits]:
                    raise ValueError("INVALID_OR_MISSING_MPFR_PRECISION")
                if arm in ("indexed_fused", "indexed_conditioned", "indexed_interned", "indexed_frontier"):
                    ve = re.search(r"^RESEARCH_MPFR_FACTOR_VE enabled=true completed=(true|false) eligible=(true|false) capped=(true|false) ", process.stderr, re.M)
                    if not ve:
                        raise ValueError("MISSING_INDEXED_FUSED_PATH_EVIDENCE")
                    row["factor_ve_completed"] = ve.group(1) == "true"
                    row["factor_ve_capped"] = ve.group(3) == "true"
                if arm == "indexed_frontier":
                    trace = re.search(r"^RESEARCH_MPFR_INDEXED_FRONTIER planned=(\d+) completed=(\d+) pruned=(\d+) estimated_work=(\d+) estimated_peak=(\d+) scc_merges=(\d+) arithmetic=(\d+) actual_work=(\d+) planning_ns=(\d+)$", process.stderr, re.M)
                    if not trace:
                        raise ValueError("MISSING_INDEXED_FRONTIER_PATH_EVIDENCE")
                    for i, key in enumerate(("planned","completed","pruned","estimated_work","estimated_peak","scc_merges","arithmetic","actual_work","planning_ns"), 1):
                        row["frontier_" + key] = int(trace.group(i))
                if arm == "indexed_interned":
                    trace = re.search(r"^RESEARCH_MPFR_INDEXED_INTERNED mul=(\d+) add=(\d+) nontrivial=(\d+) hits=(\d+) peak_pool=(\d+) gc=(\d+) gc_scans=(\d+) peak_live=(\d+) removed=(\d+)$", process.stderr, re.M)
                    if not trace:
                        raise ValueError("MISSING_INDEXED_INTERNED_PATH_EVIDENCE")
                    for i, key in enumerate(("mul","add","nontrivial","hits","peak_pool","gc","gc_scans","peak_live","removed"), 1):
                        row["interned_" + key] = int(trace.group(i))
                if arm == "indexed_conditioned":
                    trace = re.search(r"^RESEARCH_MPFR_INDEXED_CONDITION selected=(\d+) probes=(\d+) branches_completed=(\d+) root_work=(\d+) root_peak=(\d+) chosen_work=(\d+) chosen_peak=(\d+) planning_ns=(\d+)$", process.stderr, re.M)
                    if not trace:
                        raise ValueError("MISSING_INDEXED_CONDITION_PATH_EVIDENCE")
                    for i, key in enumerate(("selected","probes","branches_completed","root_work","root_peak","chosen_work","chosen_peak","planning_ns"), 1):
                        row["condition_" + key] = int(trace.group(i))
                row.update(precision_bits=int(match.group(9)),
                           solve_ns=int(match.group(3)),
                           nodes=int(match.group(5)),
                           cache_hits=int(match.group(6)),
                           decompositions=int(match.group(7)))
        except (ValueError, OSError) as err:
            row["status"] = "CENSORED_INVALID_NUMERIC_OUTPUT"
            row["error"] = str(err)[:300]
        return row

def long_screen(row: dict) -> bool:
    if row["status"] == "COMPLETED":
        return row["wall_ns"] >= MIN_LONG_SECONDS * 1_000_000_000
    return row["status"] in ("CENSORED_TIME", "CENSORED_OUTER_TIMEOUT")

def score_screen(rows: dict) -> tuple:
    # Precommitted long-tail selection: first genuine GPMC>=10s or GPMC
    # 30s timeout, then original source-high-cost Rust-only cases.
    # GPMC timeout is retained as censored, not interpreted as a score.
    gpmc = rows["gpmc"]
    gpmc_long = long_screen(gpmc)
    completed = all(x["status"] == "COMPLETED" for x in rows.values())
    seconds = max(
        (v.get("wall_ns", v.get("wall_ns_lower_bound", 0)) / 1e9)
        for v in rows.values()
    )
    gpmc_seconds = gpmc.get("wall_ns", gpmc.get("wall_ns_lower_bound", 0)) / 1e9
    return (int(gpmc_long), int(completed), gpmc_seconds, seconds)

def persist(args: argparse.Namespace, report: dict) -> None:
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

def self_test() -> None:
    fake = argparse.Namespace(gpmc=Path("/original/gpmc"),
                              mpfr=Path("/original/rust"))
    for digits, bits in BITS.items():
        assert bits == (100 if digits == 30 else 333)
        for arm in ARMS:
            cmd = command(fake, "mqt_qnn_12",
                          Path("/original-weighted.cnf"), digits, arm)
            assert ("--hashbrown-borrowed" in cmd) == (arm != "gpmc")
            assert ("--factor-variable-elimination" in cmd) is False
            assert cmd.count(LONG_TAIL_NODE_BUDGET) == (arm != "gpmc")
            assert not any(x.startswith("--node-budget=") for x in
                           (cmd[1:] if arm == "gpmc" else cmd[2:]))
    assert long_screen({"status": "COMPLETED", "wall_ns": 10_000_000_000})
    assert not long_screen({"status": "COMPLETED", "wall_ns": 9_999_999_999})
    assert long_screen({"status": "CENSORED_TIME"})
    assert not long_screen({"status": "CENSORED_EXIT_OR_RESOURCE"})
    assert not long_screen({"status": "CENSORED_NODE_BUDGET"})
    assert classify_native_failure(2, "RESEARCH_MPFR_FAIL_CLOSED: node limit") == "CENSORED_NODE_BUDGET"
    assert classify_native_failure(124, "") == "CENSORED_TIME"
    assert classify_native_failure(2, "allocation failed") == "CENSORED_EXIT_OR_RESOURCE"
    assert score_screen({
        "gpmc": {"status": "CENSORED_TIME", "wall_ns": 30_000_000_000},
        "inline_full": {"status": "COMPLETED", "wall_ns": 1_000_000_000},
    }) > score_screen({
        "gpmc": {"status": "COMPLETED", "wall_ns": 1_000_000_000},
        "inline_full": {"status": "COMPLETED", "wall_ns": 20_000_000_000},
    })
    sample = "\n".join(
        f"feynman_test_{i}\tbenchmarks/Feynman/a{i}.qasm" for i in range(EXPORTED_CASES)
    )
    assert len(source_index(sample)) == EXPORTED_CASES
    try:
        source_index(sample + "\nmqt_bad\t../private.qasm")
        raise AssertionError("index rejects untrusted path")
    except ValueError:
        pass
    print("ORIGINAL_LONGTAIL_SOURCE_PRECISION_ARM_PREFLIGHT_PASS", flush=True)

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--stage", choices=("screen", "pilot"), default="screen")
    ap.add_argument("--pilot-case", type=str)
    ap.add_argument("--frontier-pilot", action="store_true")
    for field in ("quprs", "gpmc-source", "exporter", "gpmc", "mpfr", "corpus", "report"):
        ap.add_argument("--" + field, type=Path)
    args = ap.parse_args()
    self_test()
    if args.self_test:
        return
    if args.frontier_pilot and (args.stage != "pilot" or args.pilot_case != "feynman_gf2_8_mult"):
        ap.error("--frontier-pilot requires the focused GF2^8 original-source pilot")
    required = ("quprs", "gpmc_source", "gpmc", "mpfr", "corpus", "report")
    if args.stage == "screen":
        required += ("exporter",)
        if args.pilot_case:
            ap.error("--pilot-case belongs to --stage pilot")
    elif not args.pilot_case:
        ap.error("--stage pilot requires --pilot-case")
    for field in required:
        if getattr(args, field) is None:
            ap.error("missing --" + field.replace("_", "-"))
    if git_sha(args.quprs) != QU_PRS_SHA or git_sha(args.gpmc_source) != GPMC_SHA:
        raise SystemExit("ORIGINAL_QU_PRS_GPMC_SOURCE_LOCK_DRIFT")
    args.corpus.mkdir(parents=True, exist_ok=True)
    numeric_disagreement = False
    if args.stage == "pilot":
        # The independently completed screen artifact is the contract.
        # No source regeneration, no silent case substitution.
        if not args.report.is_file():
            raise SystemExit("MISSING_SCREEN_REPORT")
        report = json.loads(args.report.read_text(encoding="utf-8"))
        if (report.get("status") != "LONGTAIL_SCREEN_COMPLETE"
                or report.get("source_revisions") !=
                {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA}
                or report.get("precisions") !=
                {"GPMC_decimal_30": 100, "GPMC_decimal_100": 333}
                or report.get("limits", {}).get("rust_node_budget_research") != 1_000_000_000):
            raise SystemExit("INVALID_SCREEN_OR_PRECISION_SOURCE_LOCK")
        indexed = source_index(
            (args.corpus / "source-index.tsv").read_text(encoding="utf-8")
        )
        records = report.get("cases", [])
        if len(records) != EXPORTED_CASES or {r["case"] for r in records} != set(indexed):
            raise SystemExit("MISSING_SOURCE_CASES")
        selected_ids = report.get("selected", [])
        if args.pilot_case not in selected_ids or len(selected_ids) != MAX_PAIRED_LONG_CASES:
            raise SystemExit("PILOT_CASE_NOT_SOURCE_QUALIFIED")
        for record in records:
            name = record["case"]
            source = args.quprs / indexed[name]
            if (record["original_qasm"] != indexed[name]
                    or sha(source) != record["source_qasm_sha256"]):
                raise SystemExit(f"ORIGINAL_SOURCE_SHA_MISMATCH {name}")
            if record["source_status"] == "SOURCE_EXACT":
                cnf = args.corpus / f"{name}-gpmc.cnf"
                copied = args.corpus / f"{name}.qasm"
                if (not cnf.is_file() or not copied.is_file()
                        or sha(cnf) != record["cnf_sha256"]
                        or copied.read_bytes() != source.read_bytes()
                        or set(record["screen"]) != set(ARMS)
                        or not record.get("cnf_variables")
                        or not record.get("cnf_clauses")):
                    raise SystemExit(f"ORIGINAL_WEIGHTED_CNF_SHA_MISMATCH {name}")
        selected = [r for r in records if r["case"] == args.pilot_case]
        args.report = args.corpus / f"long-tail-pilot-{args.pilot_case}.json"
        report["pilot_case"] = args.pilot_case
        report["status"] = "PILOT_IN_PROGRESS"
        persist(args, report)
    else:
        original = subprocess.check_output(
            [str(args.exporter.resolve()), "--list-sources"],
            text=True, timeout=10,
        )
        index = source_index(original)
        (args.corpus / "source-index.tsv").write_text(original, encoding="utf-8")
        report = {
            "status": "IN_PROGRESS", "study": "Long-tail ORIGINAL QASM -> QuPRS Weighted CNF",
            "do_not_label_oversized_circuit_as_long_wmc": True,
            "precisions": {"GPMC_decimal_30": 100, "GPMC_decimal_100": 333},
            "source_revisions": {"quprs": QU_PRS_SHA, "gpmc": GPMC_SHA},
            "source_provenance": "unchanged committed Feynman/MQTBench QASM; pinned QuPRS WmcManager; not MQT-published CNFs",
            "not_independently_certified": "WMC solver agreement is not an independent proof",
            "selection": {"screen_each_arm_seconds": SCREEN_S,
                          "min_screen_wall_seconds": MIN_LONG_SECONDS,
                          "max_paired_cases": MAX_PAIRED_LONG_CASES,
                          "pilot_trials": PILOT_TRIALS,
                          "case_rank": "GPMC-measured hard first, then Rust-hard; no winner selection",
                          "exclude_microbenchmarks": True},
            "limits": {"export_wall_s": EXPORT_S, "screen_wall_s": SCREEN_S,
                       "pilot_wall_s": PAIR_S, "rust_node_budget_research": 1_000_000_000,
                       "default_research_node_budget_unchanged": 100_000,
                       "AS_bytes_each_process": AS_BYTES,
                       "max_source_cnf_bytes": CNF_MAX_BYTES},
            "cases": [], "selected": [],
        }
        for case, rel in index.items():
            src = args.quprs / rel
            item = {"case": case, "original_qasm": rel, "family": case.split("_")[0],
                    "source_qasm_sha256": sha(src), "source_bytes": src.stat().st_size,
                    "source_status": "IN_PROGRESS", "screen": {},
                    "pilot": []}
            report["cases"].append(item)
            persist(args, report)
            proc = native(
                [str(args.exporter.resolve()), case, str(args.quprs.resolve()),
                 str(args.corpus.resolve())], "source", EXPORT_S, None,
            )
            (args.corpus / f"{case}.export.log").write_text(
                proc.get("stderr_tail", "") + "\n", encoding="utf-8"
            )
            item["export"] = proc
            copied = args.corpus / f"{case}.qasm"
            cnf = args.corpus / f"{case}-gpmc.cnf"
            if proc["status"] != "COMPLETED" or not copied.is_file() or not cnf.is_file():
                item["source_status"] = "CENSORED_SOURCE_EXPORT"
            elif copied.read_bytes() != src.read_bytes():
                item["source_status"] = "FATAL_SOURCE_QASM_DRIFT"
                persist(args, report)
                raise SystemExit(f"ORIGINAL_QASM_BYTES_CHANGED {case}")
            elif not 0 < cnf.stat().st_size <= CNF_MAX_BYTES:
                item["source_status"] = "CENSORED_CNF_BYTES"
            else:
                try:
                    parsed = parse_cnf(cnf, "gpmc")
                    item["source_status"] = "SOURCE_EXACT"
                    item["cnf_sha256"] = sha(cnf)
                    item["cnf_bytes"] = cnf.stat().st_size
                    item["cnf_variables"] = parsed["variables"]
                    item["cnf_clauses"] = len(parsed["clauses"])
                except (ValueError, OSError) as error:
                    item["source_status"] = "CENSORED_INVALID_SOURCE_CNF"
                    item["source_error"] = str(error)[:300]
            if item["source_status"] == "SOURCE_EXACT":
                for arm in ARMS:
                    sample = native(command(args, case, cnf, 30, arm), arm, SCREEN_S, 30)
                    item["screen"][arm] = sample
                    persist(args, report)
                scr = item["screen"]
                item["long_candidate"] = any(long_screen(x) for x in scr.values())
                if all(x["status"] == "COMPLETED" for x in scr.values()):
                    witness = complex_relative_witness(scr["gpmc"], scr["inline_full"], 30)
                    item["screen_numeric_witness"] = witness
                    if scr["gpmc"]["sat"] != scr["inline_full"]["sat"] or not witness["pass"]:
                        numeric_disagreement = True
                        item["screen_disagreement"] = "SAT_OR_SIGNED_COMPLEX_MISMATCH"
                print(f"LONGTAIL_SCREEN case={case} gpmc={scr['gpmc']['status']} "
                      f"rust={scr['inline_full']['status']} "
                      f"long_candidate={item['long_candidate']}", flush=True)
            else:
                item["long_candidate"] = False
                print(f"LONGTAIL_SOURCE_CENSORED case={case} reason={item['source_status']}",
                      flush=True)
            persist(args, report)
        # Select by OBSERVED cost only. Never infer difficulty from QASM gates,
        # qubit count or CNF size, and never cherry-pick by Rust winning.
        qualified = [x for x in report["cases"] if x.get("long_candidate") and
                     "screen_disagreement" not in x]
        qualified.sort(key=lambda x: score_screen(x["screen"]), reverse=True)
        selected = qualified[:MAX_PAIRED_LONG_CASES]
        report["selected"] = [x["case"] for x in selected]
        persist(args, report)
        # Preserve independent original-CNF numeric evidence and fail
        # closed before allowing any expensive pilot to reuse this screen.
        if numeric_disagreement:
            report["status"] = "NUMERIC_DISAGREEMENT_FAIL"
            persist(args, report)
            raise SystemExit("FAIL_SAT_OR_SIGNED_COMPLEX_WMC")
        # Upload the complete 30-second source evidence before any pilot.
        report["status"] = "LONGTAIL_SCREEN_COMPLETE"
        persist(args, report)
        print(f"LONGTAIL_SCREEN_FINAL status={report['status']} "
              f"source_cases={len(report['cases'])} "
              f"screen_long={len(qualified)} selected={report['selected']}", flush=True)
        return
    for item in selected:
        case = item["case"]
        cnf = args.corpus / f"{case}-gpmc.cnf"
        for digits, bits in BITS.items():
            cohort = {"digits": digits, "bits": bits, "status": "IN_PROGRESS",
                      "trials": [], "design": "EXPLORATORY_TWO_TRIALS_NOT_PRODUCTION_GO"}
            item["pilot"].append(cohort)
            persist(args, report)
            if args.frontier_pilot:
                # One genuinely long pair at each precision first, not a
                # seven-trial campaign or a claim from a censored control.
                cohort["design"] = "EXPLORATORY_ONE_PAIRED_120S_TRIAL_NOT_PRODUCTION_GO"
                arms = ("gpmc", "indexed_frontier") if digits == 30 else ("indexed_frontier", "gpmc")
                for arm in arms:
                    cmd = frontier_command(args, case, cnf, digits) if arm == "indexed_frontier" else command(args, case, cnf, digits, arm)
                    row = native(cmd, arm, PAIR_S, digits)
                    row["trial"] = 0
                    cohort["trials"].append(row)
                    persist(args, report)
                    print(f"LONGTAIL_FRONTIER_PILOT case={case} bits={bits} arm={arm} status={row['status']}", flush=True)
                members = {x["arm"]: x for x in cohort["trials"]}
                control, candidate = members["gpmc"], members["indexed_frontier"]
                fastpath = candidate["status"] == "COMPLETED" and candidate.get("factor_ve_completed") is True and candidate.get("frontier_planned") == candidate.get("frontier_completed")
                if control["status"] == "COMPLETED" and candidate["status"] == "COMPLETED":
                    witness = complex_relative_witness(control, candidate, digits)
                    cohort["numeric_witness"] = witness
                    if control["sat"] != candidate["sat"] or not witness["pass"]:
                        numeric_disagreement = True
                        cohort["status"] = "FAIL_SAT_OR_SIGNED_COMPLEX_WMC"
                    else:
                        cohort["status"] = "EXPLORATORY_MATCHED_FRONTIER" if fastpath else "MATCHED_FALLBACK_NOT_FRONTIER"
                elif fastpath:
                    cohort["status"] = "FRONTIER_COMPLETE_REFERENCE_CENSORED_UNVERIFIED"
                else:
                    cohort["status"] = "CENSORED_OR_FALLBACK_FRONTIER_PILOT"
                cohort["frontier_fastpath_complete"] = fastpath
                cohort["promotion"] = "NOT_AUTHORIZED_RESEARCH_ONLY"
                persist(args, report)
                continue
            for trial in range(PILOT_TRIALS):
                ordered = ARMS if trial % 2 == 0 else tuple(reversed(ARMS))
                for arm in ordered:
                    row = native(command(args, case, cnf, digits, arm),
                                 arm, PAIR_S, digits)
                    row["trial"] = trial
                    cohort["trials"].append(row)
                    persist(args, report)
                    print(f"LONGTAIL_TRIAL case={case} bits={bits} "
                          f"trial={trial} arm={arm} status={row['status']}", flush=True)
            rows = cohort["trials"]
            if any(x["status"] != "COMPLETED" for x in rows):
                cohort["status"] = "CENSORED_PILOT_ONE_OR_BOTH_SOLVERS"
                cohort["completion_counts"] = {
                    arm: sum(x["status"] == "COMPLETED" for x in rows if x["arm"] == arm)
                    for arm in ARMS
                }
                cohort["censored_by_arm"] = {
                    arm: [x["status"] for x in rows if x["arm"] == arm]
                    for arm in ARMS
                }
            else:
                for trial in range(PILOT_TRIALS):
                    members = {x["arm"]: x for x in rows if x["trial"] == trial}
                    witness = complex_relative_witness(
                        members["gpmc"], members["inline_full"], digits
                    )
                    if (members["gpmc"]["sat"] != members["inline_full"]["sat"]
                            or not witness["pass"]):
                        numeric_disagreement = True
                        cohort["status"] = "FAIL_SAT_OR_SIGNED_COMPLEX_WMC"
                        cohort["numeric_witness"] = witness
                        break
                else:
                    gpmc = [x["wall_ns"] for x in rows if x["arm"] == "gpmc"]
                    rust = [x["wall_ns"] for x in rows if x["arm"] == "inline_full"]
                    cohort["status"] = "EXPLORATORY_TWO_TRIALS_SIGNED_COMPLEX_MATCH"
                    cohort["gpmc_p50_ms"] = statistics.median(gpmc) / 1e6
                    cohort["rust_p50_ms"] = statistics.median(rust) / 1e6
                    cohort["rust_over_gpmc_wall_ratio"] = (
                        statistics.median(rust) / statistics.median(gpmc)
                    )
                    cohort["rust_paired_wins"] = sum(
                        rows[2 * i + (1 if i % 2 == 0 else 0)]["wall_ns"] <
                        rows[2 * i + (0 if i % 2 == 0 else 1)]["wall_ns"]
                        for i in range(PILOT_TRIALS)
                    )
            persist(args, report)
            print(f"LONGTAIL_PILOT case={case} bits={bits} "
                  f"status={cohort['status']} "
                  f"gpmc_p50_ms={cohort.get('gpmc_p50_ms', 'CENSORED')} "
                  f"rust_p50_ms={cohort.get('rust_p50_ms', 'CENSORED')}",
                  flush=True)
    if numeric_disagreement:
        report["status"] = "NUMERIC_DISAGREEMENT_FAIL"
    elif selected:
        report["status"] = "LONGTAIL_CASE_EXPLORATORY_PILOT_COMPLETE"
    else:
        report["status"] = "NO_ACTUALLY_MEASURED_LONG_CASES_IN_SOURCE_BATCH"
    persist(args, report)
    print(f"LONGTAIL_PILOT_FINAL status={report['status']} "
          f"source_cases={len(report['cases'])} "
          f"case={args.pilot_case} paired_cases={len(selected)}", flush=True)
    if numeric_disagreement:
        raise SystemExit("FAIL_SAT_OR_SIGNED_COMPLEX_WMC")

if __name__ == "__main__":
    main()
