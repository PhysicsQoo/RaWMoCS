"""Admit a version from exact main CI receipts; no downloaded code is executed."""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile

REPO = "PhysicsQoo/RaWMoCS"
REQUEST = Path("releases/request.json")
SCIENCE = {
    "core", "a0-science", "portability (ubuntu-24.04-arm)",
    "portability (windows-latest)", "portability (macos-15)",
    "portability (macos-15-intel)",
    "First release CLI and source-locked transfer gate",
    "Regenerate original CNF and independent interval without artifacts",
    *("Release matched native GPMC (" + c + ")" for c in
      ("qpeinexact22-432", "grover6-432", "gf2-7-333")),
}
CODSPEED = {
    "Compile and validate Rust benchmarks (not a timing report)",
    "CodSpeed simulation (opt in after repository connection)",
    "Release short original QPE8 Simulation (same precision)",
}
STEPS = {
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
for name in SCIENCE:
    if name.startswith("Release matched native GPMC ("):
        STEPS[name] = {"Seven order-rotated same-precision native pairs"}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def run(*args):
    return subprocess.check_output(args)


def api(path):
    return json.loads(run("gh", "api", path))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load_request():
    r = json.loads(REQUEST.read_text())
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
    notes = Path(r["notes"])
    require(notes.is_file() and not notes.is_symlink() and notes.stat().st_size > 0, "missing notes")
    return r


def verify_run(r, run_data, jobs, kind):
    expected = SCIENCE if kind == "science" else CODSPEED
    workflow = "rust-science.yml" if kind == "science" else "codspeed.yml"
    require(run_data["id"] == r[kind + "_run"], "wrong run ID")
    require(run_data["repository"]["full_name"] == REPO
            and run_data["head_repository"]["full_name"] == REPO, "wrong source repository")
    require(run_data["path"].split("@", 1)[0] == ".github/workflows/" + workflow, "wrong workflow")
    require(run_data["head_branch"] == "main" and run_data["head_sha"] == r["revision"], "wrong run revision")
    require(run_data["event"] in ("push", "workflow_dispatch"), "untrusted run event")
    require(run_data["status"] == "completed" and run_data["conclusion"] == "success", "run not successful")
    require(jobs["total_count"] == len(jobs["jobs"]) == len(expected)
            and {j["name"] for j in jobs["jobs"]} == expected, "missing or unexpected jobs")
    for j in jobs["jobs"]:
        require(j["status"] == "completed" and j["conclusion"] == "success", f"failed job {j['name']}")
        steps = {s["name"]: s["conclusion"] for s in j["steps"]}
        require(all(steps.get(s) == "success" for s in STEPS.get(j["name"], ())),
                f"missing or failed required step {j['name']}")
        require(not any(s["conclusion"] in ("failure", "cancelled", "timed_out") for s in j["steps"]),
                f"failed step in {j['name']}")


def verify_assets(r, assets):
    v = r["version"]
    archive = f"rawmocs-{v}-source.tar.gz"
    expected = {archive, "SHA256SUMS", "source-package.json"}
    require(set(assets) == expected, "unexpected assets")
    sha = digest(assets[archive])
    require(sha == r["source_sha256"], "source digest mismatch")
    require(assets["SHA256SUMS"].decode().split() == [sha, archive], "checksum record mismatch")
    receipt = json.loads(assets["source-package.json"])
    for key, value in {
        "status": "SOURCE_PACKAGE_INSTALL_PASS", "revision": r["revision"],
        "version": v, "target": "x86_64-unknown-linux-gnu", "source_archive": archive,
        "source_sha256": sha, "deterministic_archive": True, "redistributed_binary": False,
        "offline_build": False, "controls": ["version", "help", "SAT-zero", "UNSAT", "general", "timeout"],
    }.items():
        require(receipt.get(key) == value, f"receipt mismatch: {key}")
    prefix = f"rawmocs-{v}/"
    with tarfile.open(fileobj=io.BytesIO(assets[archive]), mode="r:gz") as tar:
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
        require(digest(read("crates/rawmocs-cli/Cargo.lock")) == receipt["cargo_lock_sha256"], "lock mismatch")
        require(tomllib.loads(read("crates/rawmocs-cli/Cargo.toml").decode())["package"]["version"] == v,
                "manifest version mismatch")


def prepare(r):
    require(os.environ.get("GITHUB_REPOSITORY") == REPO, "unexpected repository")
    main = api(f"repos/{REPO}/git/ref/heads/main")["object"]["sha"]
    subprocess.run(["git", "merge-base", "--is-ancestor", r["revision"], main], check=True)
    manifest = run("git", "show", r["revision"] + ":crates/rawmocs-cli/Cargo.toml").decode()
    require(tomllib.loads(manifest)["package"]["version"] == r["version"], "target version mismatch")
    for kind in ("science", "codspeed"):
        endpoint = f"repos/{REPO}/actions/runs/{r[kind + '_run']}"
        run_data = api(endpoint)
        jobs = api(f"{endpoint}/attempts/{run_data['run_attempt']}/jobs?per_page=100")
        verify_run(r, run_data, jobs, kind)
    endpoint = f"repos/{REPO}/actions/artifacts/{r['artifact_id']}"
    artifact = api(endpoint)
    require(artifact["id"] == r["artifact_id"] and artifact["name"] == "rawmocs-v010-candidate-evidence"
            and not artifact["expired"] and artifact["digest"] == r["artifact_digest"]
            and artifact["workflow_run"]["id"] == r["science_run"]
            and artifact["workflow_run"]["head_sha"] == r["revision"], "artifact provenance mismatch")
    blob = run("gh", "api", endpoint + "/zip")
    require("sha256:" + digest(blob) == r["artifact_digest"], "artifact ZIP digest mismatch")
    names = (f"rawmocs-{r['version']}-source.tar.gz", "SHA256SUMS", "source-package.json")
    with zipfile.ZipFile(io.BytesIO(blob)) as z:
        require(len(z.namelist()) == len(set(z.namelist())), "duplicate ZIP member")
        assets = {name: z.read(".release-evidence/source-package/" + name) for name in names}
    verify_assets(r, assets)
    print("RELEASE_ADMISSION_PASS", r["revision"], r["version"], flush=True)
    return assets


def tag_commit(tag):
    rows = run("git", "ls-remote", f"https://github.com/{REPO}.git",
               f"refs/tags/{tag}", f"refs/tags/{tag}^{{}}").decode().splitlines()
    refs = dict((ref, sha) for sha, ref in (row.split() for row in rows))
    return refs.get(f"refs/tags/{tag}^{{}}", refs.get(f"refs/tags/{tag}"))


def releases(tag):
    pages = json.loads(run("gh", "api", "--paginate", "--slurp", f"repos/{REPO}/releases?per_page=100"))
    return [r for page in pages for r in page if r["tag_name"] == tag]


def verify_release(release, r, assets, draft):
    require(release["tag_name"] == "v" + r["version"] and release["draft"] is draft
            and not release["prerelease"], "release state mismatch")
    expected = {name: "sha256:" + digest(data) for name, data in assets.items()}
    require(len(release["assets"]) == len(expected)
            and {a["name"]: a["digest"] for a in release["assets"]} == expected
            and all(a["state"] == "uploaded" for a in release["assets"]), "release asset digest mismatch")


def publish(r, assets):
    require(os.environ.get("GITHUB_REF") == "refs/heads/main"
            and os.environ.get("GITHUB_EVENT_NAME") in ("push", "workflow_dispatch"), "publication must run on main")
    current = api(f"repos/{REPO}/git/ref/heads/main")["object"]["sha"]
    require(run("git", "rev-parse", "HEAD").decode().strip() == current, "stale release request checkout")
    tag = "v" + r["version"]
    existing = releases(tag)
    actual = tag_commit(tag)
    if existing:
        require(len(existing) == 1 and not existing[0]["draft"], "existing draft/duplicate release; inspect manually")
        require(actual == r["revision"], "existing tag points elsewhere")
        verify_release(existing[0], r, assets, False)
        print("RELEASE_ALREADY_PUBLISHED", existing[0]["html_url"])
        return
    require(actual is None, "existing tag without release; inspect manually")
    with tempfile.TemporaryDirectory() as tmp:
        paths = []
        for name, data in assets.items():
            path = Path(tmp) / name
            path.write_bytes(data)
            paths.append(str(path))
        # Draft first; no partial public release if upload/verification fails.
        subprocess.run(["gh", "release", "create", tag, *paths, "-R", REPO, "--draft",
                        "--target", r["revision"], "--title", f"RaWMoCS {tag}",
                        "--notes-file", r["notes"]], check=True)
        drafts = releases(tag)
        require(len(drafts) == 1 and drafts[0]["target_commitish"] == r["revision"], "draft target mismatch")
        verify_release(drafts[0], r, assets, True)
        require(tag_commit(tag) in (None, r["revision"]), "tag appeared at wrong revision")
        require(api(f"repos/{REPO}/git/ref/heads/main")["object"]["sha"] == current, "main advanced before publication")
        subprocess.run(["gh", "release", "edit", tag, "-R", REPO, "--draft=false"], check=True)
    published = releases(tag)
    require(len(published) == 1 and tag_commit(tag) == r["revision"], "published tag mismatch")
    verify_release(published[0], r, assets, False)
    print("RELEASE_PUBLISHED_AND_TAG_VERIFIED", published[0]["html_url"], r["revision"])


def main():
    require(len(sys.argv) == 2 and sys.argv[1] in ("validate", "publish"), "usage: release_source.py validate|publish")
    r = load_request()
    assets = prepare(r)
    if sys.argv[1] == "publish":
        publish(r, assets)


if __name__ == "__main__":
    main()
