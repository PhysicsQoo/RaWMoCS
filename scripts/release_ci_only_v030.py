"""One-version CI-only carry-forward admission for v0.3.0.

The full main run qualified exactly BASELINE. The new revision must
differ only in these explicitly reviewed CI/test/document files.
Revalidate historical full runs and require fresh exact-main bounded
science, source installation, API and short simulation receipts.
Never reuse the old source archive or authorize another version.
"""
import argparse
import json
import re
import subprocess
from release_python_source import RESEARCH_SKIPS

BASELINE = "416578566f4b73be43ee7202f64e9d11ccd11388"
BASELINE_SCIENCE = 46044562938
BASELINE_CODSPEED = 46044562898
ALLOWED = {
    ".github/workflows/codspeed.yml", ".github/workflows/rust-science.yml",
    "crates/rawmocs-cli/tools/native_transport_light.py",
    "crates/rawmocs-cli/tools/test_native_transport_light.py",
    "docs/ci-parallel-release.md", "scripts/ci_scope.py",
    "scripts/release_source.py", "scripts/test_ci_scope.py",
    "scripts/test_release_python_source.py", "scripts/release_ci_only_v030.py",
    "releases/v0.3.0.md",
}
ROUTINE = "Routine small-family science and R6-Light"
SKIPS = RESEARCH_SKIPS | {"python-portability", "python-manylinux",
                         "Release shared-core native transport (parallel)",
                         "Release matched native GPMC ($"+ "{{ matrix.case }})"}
SHORT = "Release short original QPE8 Simulation (same precision)"
SIM = "CodSpeed simulation (opt in after repository connection)"
REQUIRED = {
    "core": {"Classify exact change scope (ordinary CI, not release admission)",
             "Verify ordinary scope classifier", "Verify v0.3.0 CI-only release source scope",
             "Verify R6-Light parity controls", "Verify Python package integrity rejects malformed artifacts",
             "Verify release gate rejects incomplete science receipts",
             "Run cargo fmt --all -- --check",
             "Run cargo clippy --workspace --all-targets -- -D warnings",
             "Run cargo test --workspace"},
    "a0-science": {"Independent original A0 reference (48 complex, 620 properties, cache, rewrite safety)",
                   "New solver versus source-pinned Full WMC fixtures and exhaustive baseline"},
    "First release CLI and source-locked transfer gate": {
        "Build and test candidate with resolved dependency lock",
        "Prototype Python native wheel and API contract", "Archive Python packaging evidence",
        "Source archive and extracted-tree installation admission",
        "Scoped Python encoder and real solver process conformance",
        "Isolated actual-search kernel OOM and non-OOM signal controls"},
    ROUTINE: {"Qualify small source-locked family representatives (release)",
              "R6-Light small original-family native transport rehearsal",
              "Archive bounded routine science evidence"},
    SHORT: {"CodSpeed bounded release Simulation", "Verify simulated output, precision and plan signature"},
}
SCIENCE = {"core", "a0-science", "First release CLI and source-locked transfer gate", ROUTINE,
           *("portability (" + x + ")" for x in ("ubuntu-24.04-arm","windows-latest","macos-15","macos-15-intel"))}
CODSPEED = {"Compile and validate Rust benchmarks (not a timing report)", SHORT}

def require(ok, message):
    if not ok:
        raise ValueError(message)

def check_diff(raw):
    records = raw.split(b"\0")
    require(records.pop() == b"" and len(records) % 2 == 0, "malformed source diff")
    seen = set()
    for header, filename in zip(records[::2], records[1::2]):
        match = re.fullmatch(rb":([0-7]{6}) ([0-7]{6}) ([0-9a-f]{40}) ([0-9a-f]{40}) ([AM])", header)
        require(match is not None, "unsupported source change")
        before, after, _, _, status = match.groups()
        path = filename.decode("utf-8", "strict")
        require(path in ALLOWED and path not in seen, "non-CI source change: " + path)
        require(after == b"100644" and ((status == b"M" and before == b"100644") or
                (status == b"A" and before == b"000000")), "non-regular source change")
        seen.add(path)
    require(bool(seen), "empty CI-only change")
    return sorted(seen)

def git(*args):
    return subprocess.check_output(["git", *args])

def verify_scope(revision):
    require(re.fullmatch(r"[0-9a-f]{40}", revision) is not None, "exact revision required")
    for sha in (BASELINE, revision):
        require(git("rev-parse", "--verify", sha + "^{commit}").decode().strip() == sha,
                "wrong Git identity")
    subprocess.run(["git", "merge-base", "--is-ancestor", BASELINE, revision], check=True)
    changed = check_diff(git("diff", "--raw", "--no-renames", "--no-abbrev", "-z", BASELINE, revision, "--"))
    report = {"status": "V030_CI_ONLY_SCOPE_PASS", "baseline": BASELINE,
              "revision": revision, "changed_paths": changed, "publication_authorized": False}
    print(json.dumps(report, sort_keys=True), flush=True)
    return report

def verify_baseline(api, verify_run):
    r = {"version": "0.3.0", "revision": BASELINE, "science_run": BASELINE_SCIENCE,
         "codspeed_run": BASELINE_CODSPEED}
    for kind in ("science", "codspeed"):
        endpoint = "repos/PhysicsQoo/RaWMoCS/actions/runs/" + str(r[kind + "_run"])
        data = api(endpoint)
        jobs = api(endpoint + "/attempts/" + str(data["run_attempt"]) + "/jobs?per_page=100")
        verify_run(r, data, jobs, kind)

def verify_bounded_run(r, data, jobs, kind):
    require(r["version"] == "0.3.0" and r.get("qualification") == "v030-ci-only",
            "wrong qualification")
    require(data["id"] == r[kind + "_run"] and data["head_sha"] == r["revision"]
            and data["head_branch"] == "main" and data["event"] == "push"
            and data["repository"]["full_name"] == data["head_repository"]["full_name"] == "PhysicsQoo/RaWMoCS",
            "wrong exact-main bounded run")
    filename = "rust-science.yml" if kind == "science" else "codspeed.yml"
    require(data["path"].split("@",1)[0] == ".github/workflows/" + filename
            and "[release-package]" in (data.get("head_commit") or {}).get("message","")
            and data["status"] == "completed" and data["conclusion"] == "success",
            "missing package intent or failed run")
    passed, skipped = (SCIENCE, SKIPS) if kind == "science" else (CODSPEED, {SIM})
    expected = passed | skipped
    require(jobs["total_count"] == len(jobs["jobs"]) == len(expected)
            and {j["name"] for j in jobs["jobs"]} == expected, "wrong bounded job inventory")
    for j in jobs["jobs"]:
        name = j["name"]
        require(j["status"] == "completed" and j["conclusion"] ==
                ("skipped" if name in skipped else "success"), "failed/skipped required job: " + name)
        if name in skipped:
            continue
        steps = {x["name"]: x["conclusion"] for x in j["steps"]}
        needed = REQUIRED.get(name, set())
        if name.startswith("portability ("):
            needed = {"Run cargo test --workspace"}
        require(all(steps.get(x) == "success" for x in needed), "missing required step: " + name)
        require(not any(x["conclusion"] in ("failure", "cancelled", "timed_out") for x in j["steps"]),
                "failed step: " + name)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--revision", required=True)
    verify_scope(parser.parse_args().revision)
