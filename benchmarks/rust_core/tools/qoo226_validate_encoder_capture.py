#!/usr/bin/env python3
"""QOO-226: validate REAL QuPRS WmcManager-emitted weighted CNF artifacts.

No hand-written fixture substitution and no solver invocation. This is a
source/provenance and structural gate, NOT a mathematical circuit certificate.
"""
from __future__ import annotations

import argparse
from decimal import Decimal, InvalidOperation
import hashlib
import json
from pathlib import Path
import subprocess

SOURCE_COMMIT = "0b28736651b3740ed3b201edd2efdef3da541e0d"
EXPORTER = "rust/quprs/examples/interval_cnf_reference.rs"
EXPORTER_BLOB = "f9b0bb4afba35a9a67a1b9d2844be3b77884b27a"
NAMES = ("identity", "hh", "x", "t", "rz_near", "rz_far")
MAX_BYTES = 2_000_000
MAX_VARIABLES = 20000
MAX_CLAUSES = 200000


def checksum(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def parse_decimal(value: str) -> None:
    try:
        number = Decimal(value)
    except InvalidOperation as exc:
        raise ValueError("invalid finite numeric token") from exc
    if not number.is_finite():
        raise ValueError("non-finite printed weight")


def parse_cnf(path: Path, dialect: str) -> dict:
    if not path.is_file() or not (1 <= path.stat().st_size <= MAX_BYTES):
        raise ValueError(f"missing/oversized real exporter CNF: {path}")
    lines = path.read_text(encoding="utf-8").splitlines()
    if len(lines) < 2:
        raise ValueError("too few DIMACS lines")
    header = lines[0].split()
    if header[:2] != ["p", "cnf"] or len(header) != 4:
        raise ValueError("unexpected DIMACS header")
    n, m = map(int, header[2:])
    if not (0 < n <= MAX_VARIABLES and 0 <= m <= MAX_CLAUSES):
        raise ValueError("out-of-bounds source CNF")
    if lines[1].strip() != "c t wmc":
        raise ValueError("missing QuPRS WMC marker")
    weights: dict[int, tuple[str, str]] = {}
    clauses: list[tuple[int, ...]] = []
    seen_clause = False
    for line in lines[2:]:
        if not line.strip():
            continue
        words = line.split()
        if words[:3] == ["c", "p", "weight"]:
            if seen_clause or len(words) < 7 or words[-1] != "0":
                raise ValueError("misplaced / invalid weight")
            literal = int(words[3])
            if not (0 < abs(literal) <= n):
                raise ValueError("weight literal outside header")
            if literal in weights:
                raise ValueError("duplicate literal weight")
            if dialect == "gpmc":
                if len(words) != 7:
                    raise ValueError("GPMC weight grammar mismatch")
                re_part, im_part = words[4], words[5]
                parse_decimal(re_part)
                parse_decimal(im_part)
            else:
                if len(words) != 8 or words[5] not in ("+", "-") or not words[6].endswith("i"):
                    raise ValueError("Ganak weight grammar mismatch")
                re_part, im_part = words[4], words[6][:-1]
                parse_decimal(re_part)
                parse_decimal(im_part)
                if Decimal(im_part) < 0:
                    raise ValueError("Ganak imaginary magnitude sign mismatch")
                if words[5] == "-":
                    im_part = "-" + im_part
            weights[literal] = (str(Decimal(re_part)), str(Decimal(im_part)))
        elif words[0] == "c":
            raise ValueError("unrecognized source comment")
        else:
            seen_clause = True
            if words[-1] != "0":
                raise ValueError("clause missing DIMACS 0 terminator")
            clause = tuple(int(token) for token in words[:-1])
            if any(abs(lit) == 0 or abs(lit) > n for lit in clause):
                raise ValueError("invalid CNF literal")
            clauses.append(clause)
    if len(clauses) != m:
        raise ValueError("CNF clauses do not match header")
    return {"variables": n, "clauses_count": m, "clauses": clauses, "weights": weights}


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--quprs", required=True, type=Path)
    p.add_argument("--corpus", required=True, type=Path)
    p.add_argument("--manifest", required=True, type=Path)
    args = p.parse_args()
    repo = args.quprs.resolve()
    commit = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
    blob = subprocess.check_output(
        ["git", "-C", str(repo), "hash-object", EXPORTER], text=True
    ).strip()
    if commit != SOURCE_COMMIT or blob != EXPORTER_BLOB:
        raise SystemExit(f"UNPINNED_QU_PRS_SOURCE: commit={commit} exporter_blob={blob}")
    expected = {f"{case}{suffix}" for case in NAMES for suffix in (
        ".qasm", "-gpmc.cnf", "-ganak.cnf", "-gpmc-phases.tsv", "-ganak-phases.tsv"
    )}
    supplied = {f.name for f in args.corpus.iterdir() if f.is_file()}
    if supplied != expected:
        raise SystemExit(f"ARTIFACT_SET_MISMATCH missing={sorted(expected-supplied)} excess={sorted(supplied-expected)}")
    manifest: dict = {
        "dataset": "QU_PRS_ORIGINAL_RUST_PATHSUM_WMCMANAGER_EMITTED",
        "quprs_source_commit": SOURCE_COMMIT,
        "quprs_exporter_blob": EXPORTER_BLOB,
        "source_gate": "CAPTURED_STRUCTURAL_PASS_NOT_QUANTUM_ORACLE",
        "cases": [],
    }
    for name in NAMES:
        qasm = args.corpus / f"{name}.qasm"
        g = args.corpus / f"{name}-gpmc.cnf"
        k = args.corpus / f"{name}-ganak.cnf"
        gp = args.corpus / f"{name}-gpmc-phases.tsv"
        kp = args.corpus / f"{name}-ganak-phases.tsv"
        gpmc = parse_cnf(g, "gpmc")
        ganak = parse_cnf(k, "ganak")
        if (gpmc["variables"], gpmc["clauses"]) != (ganak["variables"], ganak["clauses"]):
            raise SystemExit(f"encoder dialect CNF structure mismatch: {name}")
        if gpmc["weights"] != ganak["weights"]:
            raise SystemExit(f"same exact printed decimal weights mismatch between dialects: {name}")
        if gp.read_bytes() != kp.read_bytes():
            raise SystemExit(f"phase sidecar dialect mismatch: {name}")
        phase_lines = gp.read_text(encoding="utf-8").splitlines()
        if not phase_lines or phase_lines[0] != "weight_lit\tphase_numerator\tphase_denominator\tmonomial":
            raise SystemExit(f"source phase sidecar bad header: {name}")
        phase_ids = set()
        for phase in phase_lines[1:]:
            fields = phase.split("\t")
            if len(fields) != 4:
                raise SystemExit(f"bad phase sidecar line: {name}")
            wid, num, den, _monomial = fields
            if int(den) == 0 or int(wid) <= 0:
                raise SystemExit(f"invalid rational source phase: {name}")
            phase_ids.add(int(wid))
        positive = {lit for lit in gpmc["weights"] if lit > 0}
        if phase_ids != positive:
            raise SystemExit(f"source phase ids mismatch weight ids: {name}")
        record = {
            "case": name, "kind": "one-qubit QuPRS original research example",
            "variables": gpmc["variables"], "clauses": gpmc["clauses_count"],
            "positive_weight_count": len(positive), "source_phase_count": len(phase_ids),
            "bounded_exact_oracle_eligible": gpmc["variables"] <= 12,
            "qasm_sha256": checksum(qasm), "gpmc_cnf_sha256": checksum(g),
            "ganak_cnf_sha256": checksum(k), "phase_sidecar_sha256": checksum(gp),
            "gpmc_file": g.name, "ganak_file": k.name, "phase_file": gp.name,
        }
        manifest["cases"].append(record)
        print(f"ENCODER_SOURCE_PASS case={name} vars={record['variables']} clauses={record['clauses']} exact_oracle_eligible={record['bounded_exact_oracle_eligible']}")
    args.manifest.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"QU_PRS_SOURCE_ENCODER_CAPTURED cases={len(manifest['cases'])} commit={commit}")


if __name__ == "__main__":
    main()
