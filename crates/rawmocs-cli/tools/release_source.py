"""Fail-closed source release admission; never execute artifact contents."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

REPO = "PhysicsQoo/RaWMoCS"
SCIENCE_JOBS = {
    "core", "a0-science", "portability (ubuntu-24.04-arm)",
    "portability (windows-latest)", "portability (macos-15)", "portability (macos-15-intel)",
    "First release CLI and source-locked transfer gate",
    "Regenerate original CNF and independent interval without artifacts",
    *("Release matched native GPMC (" + c + ")" for c in
      ("qpeinexact22-432", "grover6-432", "gf2-7-333")),
}
CODSPEED_JOBS = {
    "Compile and validate Rust benchmarks (not a timing report)",
    "CodSpeed simulation (opt in after repository connection)",
    "Release short original QPE8 Simulation (same precision)",
}
REQUIRED_STEPS = {
    "First release CLI and source-locked transfer gate": {
        "Source archive and extracted-tree installation admission",
        "Scoped Python encoder and real solver process conformance",
        "Isolated actual-search kernel OOM and non-OOM signal controls"},
    "Regenerate original CNF and independent interval without artifacts": {
        "Regenerate six source CNFs and verify seven cohorts"},
    "Release short original QPE8 Simulation (same precision)": {
        "CodSpeed bounded release Simulation",
        "Verify simulated output, precision and plan signature"},
    "CodSpeed simulation (opt in after repository connection)": {
        "Upload all 62 legacy and six new paired CodSpeed identities in ONE workflow"},
}
for name in SCIENCE_JOBS:
    if name.startswith("Release matched native GPMC ("):
        REQUIRED_STEPS[name] = {"Seven order-rotated same-precision native pairs"}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def api(path):
    return json.loads(subprocess.check_output(["gh", "api", path], text=True))


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def request(path):
    r = json.loads(path.read_text())
    require(set(r) == {"version", "revision", "science_run", "codspeed_run",
                       "artifact_id", "artifact_digest", "source_sha256", "notes"},
            "unexpected request fields")
    require(re.fullmatch(r"\d+\.\d+\.\d+", r["version"]), "stable version required")
    require(re.fullmatch(r"[0-9a-f]{40}", r["revision"]), "full revision required")
    for key in ("science_run", "codspeed_run", "artifact_id"):
        require(type(r[key]) is int and r[key] > 0, f"invalid {key}")
    require(re.fullmatch(r"sha256:[0-9a-f]{64}", r["artifact_digest"]), "invalid artifact digest")
    require(re.fullmatch(r"[0-9a-f]{64}", r["source_sha256"]), "invalid source digest")
    require(r["notes"] == f"releases/v{r['version']}.md", "invalid notes path")
    return r


def verify_run(r, run, jobs, kind):
    expected = SCIENCE_JOBS if kind == "science" else CODSPEED_JOBS
    path = ".github/workflows/rust-science.yml" if kind == "science" else ".github/workflows/codspeed.yml"
    require(run["id"] == r[kind + "_run"], "wrong run ID")
    require(run["repository"]["full_name"] == REPO and run["head_repository"]["full_name"] == REPO,
            "wrong source repository")
    require(run["path"].split("@", 1)[0] == path, "wrong workflow")
    require(run["head_branch"] == "main" and run["head_sha"] == r["revision"], "wrong run revision")
    require(run["event"] in ("push", "workflow_dispatch"), "untrusted run event")
    require(run["status"] == "completed" and run["conclusion"] == "success", "run not successful")
    require(jobs["total_count"] == len(jobs["jobs"]), "incomplete job listing")
    require(len(jobs["jobs"]) == len(expected) and {j["name"] for j in jobs["jobs"]} == expected,
            "missing or unexpected jobs")
    for j in jobs["jobs"]:
        require(j["status"] == "completed" and j["conclusion"] == "success", f"failed job {j['name']}")
        steps = {s["name"]: s["conclusion"] for s in j["steps"]}
        require(all(steps.get(s) == "success" for s in REQUIRED_STEPS.get(j["name"], ())),
                f"missing or failed required step {j['name']}")
        require(not any(s["conclusion"] in ("failure", "cancelled", "timed_out") for s in j["steps"]),
                f"failed step in {j['name']}")


def verify_assets(r, directory):
    v = r["version"]
    archive = f"rawmocs-{v}-source.tar.gz"
    files = (archive, "SHA256SUMS", "source-package.json")
    for name in files:
        require((directory / name).is_file() and not (directory / name).is_symlink(), f"missing asset {name}")
    digest = sha256((directory / archive).read_bytes())
    require(digest == r["source_sha256"], "source digest mismatch")
    require((directory / "SHA256SUMS").read_text().split() == [digest, archive], "checksum record mismatch")
    receipt = json.loads((directory / "source-package.json").read_text())
    for key, value in {"status": "SOURCE_PACKAGE_INSTALL_PASS", "revision": r["revision"],
                       "version": v, "target": "x86_64-unknown-linux-gnu", "source_archive": archive,
                       "source_sha256": digest, "deterministic_archive": True,
                       "redistributed_binary": False, "offline_build": False,
                       "controls": ["version", "help", "SAT-zero", "UNSAT", "general", "timeout"]}.items():
        require(receipt.get(key) == value, f"receipt mismatch: {key}")
    prefix = f"rawmocs-{v}/"
    with tarfile.open(directory / archive, "r:gz") as tar:
        members = tar.getmembers()
        require(tar.pax_headers.get("comment") == r["revision"], "archive revision mismatch")
        require(all(m.name == prefix.rstrip("/") or m.name.startswith(prefix) for m in members), "archive prefix mismatch")
        require(len({m.name for m in members}) == len(members), "duplicate archive path")
        for m in members:
            require(".." not in Path(m.name).parts and (m.isfile() or m.isdir()), "unsafe archive member")
        def read(name):
            m = tar.getmember(prefix + name)
            require(m.isfile(), f"not a regular file: {name}")
            return tar.extractfile(m).read()
        require(bool(read("LICENSE")), "missing license")
        require(sha256(read("crates/rawmocs-cli/Cargo.lock")) == receipt["cargo_lock_sha256"], "lock mismatch")
        require(tomllib.loads(read("crates/rawmocs-cli/Cargo.toml").decode())["package"]["version"] == v,
                "manifest version mismatch")
    return files


def main():
    p = argparse.ArgumentParser()
    p.add_argument("mode", choices=("prepare", "verify"))
    p.add_argument("--request", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    r = request(args.request)
    require(os.environ.get("GITHUB_REPOSITORY") == REPO, "unexpected repository")
    subprocess.run(["git", "merge-base", "--is-ancestor", r["revision"], "HEAD"], check=True)
    require(tomllib.loads(command("git", "show", r["revision"] + ":crates/rawmocs-cli/Cargo.toml"))["package"]["version"] == r["version"], "target version mismatch")
    for kind in ("science", "codspeed"):
        endpoint = f"repos/{REPO}/actions/runs/{r[kind + '_run']}"
        run = api(endpoint)
        jobs = api(f"{endpoint}/attempts/{run['run_attempt']}/jobs?per_page=100")
        verify_run(r, run, jobs, kind)
    artifact = api(f"repos/{REPO}/actions/artifacts/{r['artifact_id']}")
    require(artifact["id"] == r["artifact_id"] and artifact["name"] == "rawmocs-v010-candidate-evidence"
            and not artifact["expired"] and artifact["digest"] == r["artifact_digest"]
            and artifact["workflow_run"]["id"] == r["science_run"]
            and artifact["workflow_run"]["head_sha"] == r["revision"], "artifact provenance mismatch")
    if args.mode == "prepare":
        args.out.mkdir(parents=True, exist_ok=False)
        with tempfile.TemporaryDirectory() as tmp:
            subprocess.run(["gh", "run", "download", str(r["science_run"]), "-R", REPO,
                            "-n", artifact["name"], "-D", tmp], check=True)
            source = Path(tmp) / ".release-evidence" / "source-package"
            files = verify_assets(r, source)
            for name in files:
                shutil.copyfile(source / name, args.out / name)
    verify_assets(r, args.out)
    print("RELEASE_ADMISSION_PASS", r["revision"], r["version"])


if __name__ == "__main__":
    main()
