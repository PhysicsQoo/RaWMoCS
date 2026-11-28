"""W1: exact registry crate cross-target feasibility, not release admission."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parent
WORK = Path(sys.argv[1]).resolve()
EVIDENCE = WORK / "evidence"
EVIDENCE.mkdir(parents=True, exist_ok=False)
SHA = "d3e9e6ccd4a146bac19f6bd2628038cf20ae331dd12f9dc0cbcd37a4e63e955d"
TARGET = "wasm32-unknown-emscripten"
receipt = {"version": "0.4.0", "crate_sha256": SHA, "target": TARGET,
           "steps": {}, "status": "HOLD"}

def save():
    (EVIDENCE / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")

def run(name, args, cwd=None, env=None, seconds=60):
    print("W1_STEP", name, flush=True)
    with (EVIDENCE / (name + ".log")).open("wb") as log:
        try:
            result = subprocess.run(args, cwd=cwd, env=env, stdout=log,
                                    stderr=subprocess.STDOUT, timeout=seconds)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code = 124
    receipt["steps"][name] = code
    save()
    print("W1_STEP_RESULT", name, code, flush=True)
    if code:
        print((EVIDENCE / (name + ".log")).read_text(errors="replace")[-6000:], flush=True)
    return code

def check(name, args, **kw):
    if run(name, args, **kw):
        raise RuntimeError(name + " failed")

def main():
    for command in (["rustc", "-vV"], ["cargo", "-V"], ["emcc", "--version"], ["node", "--version"]):
        check(command[0] + "-version", command)
    package = WORK / "rawmocs-0.4.0.crate"
    check("registry-download", ["curl", "--fail", "--location", "--retry", "2",
          "https://static.crates.io/crates/rawmocs/rawmocs-0.4.0.crate", "-o", str(package)])
    assert hashlib.sha256(package.read_bytes()).hexdigest() == SHA
    with tarfile.open(package) as tar:
        tar.extractall(WORK, filter="data")
    crate = WORK / "rawmocs-0.4.0"
    original_lock = tomllib.loads((crate / "Cargo.lock").read_text())
    packages = {p["name"]: p for p in original_lock["package"]}
    assert packages["rug"]["version"] == "1.30.0"
    assert packages["gmp-mpfr-sys"]["version"] == "1.7.1"
    assert packages["gmp-mpfr-sys"]["checksum"] == "7db155b537cb791b133341f99f68371d86ee7fa4c79aacfbc376d72d23c70531"
    shutil.copy2(crate / "Cargo.lock", EVIDENCE / "released.Cargo.lock")
    shutil.copy2(crate / "Cargo.toml", EVIDENCE / "released.Cargo.toml")
    (crate / "examples").mkdir(exist_ok=True)
    shutil.copy2(ROOT / "w1_probe.rs", crate / "examples/w1_probe.rs")
    # Published manifest disables automatic example discovery. Register only
    # the disposable harness, leaving all released solver sources intact.
    manifest = crate / "Cargo.toml"
    manifest.write_text(manifest.read_text() + '\n[[example]]\nname = "w1_probe"\npath = "examples/w1_probe.rs"\n')
    shutil.copy2(manifest, EVIDENCE / "probe.Cargo.toml")
    check("example-metadata", ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=crate)
    metadata = json.loads((EVIDENCE / "example-metadata.log").read_text())
    assert any(t["name"] == "w1_probe" and "example" in t["kind"] for p in metadata["packages"] for t in p["targets"])
    # Toolchain/clock control, independent of GMP/Rug.
    clock = WORK / "clock.rs"
    clock.write_text('fn main(){let t=std::time::Instant::now(); assert!(std::time::Instant::now()>=t); println!("W1_CLOCK_PASS");}')
    check("clock-build", ["rustc", "--target", TARGET, str(clock), "-o", str(WORK / "clock.js")], seconds=60)
    check("clock-run", ["node", str(WORK / "clock.js")], seconds=20)
    assert "W1_CLOCK_PASS" in (EVIDENCE / "clock-run.log").read_text()
    check("fetch-locked", ["cargo", "fetch", "--locked"], cwd=crate, seconds=90)
    env = dict(os.environ, CC="emcc", CXX="em++", AR="emar", RANLIB="emranlib",
               CC_wasm32_unknown_emscripten="emcc", AR_wasm32_unknown_emscripten="emar",
               CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER="emcc", GMP_MPFR_SYS_CACHE="_",
               CARGO_TARGET_DIR=str(WORK / "target"))
    build = ["cargo", "build", "--offline", "--locked", "--release", "--target", TARGET, "--example", "w1_probe", "-j", "2"]
    code = run("unmodified-cross-build", build, cwd=crate, env=env, seconds=100)
    if code:
        log = (EVIDENCE / "unmodified-cross-build.log").read_text(errors="replace")
        if "force-cross" not in log:
            receipt["status"] = "BASELINE_BUILD_BLOCKED"
            return
        # Documented experimental feature only, in a disposable manifest. Do
        # not modify released sources, override ABI bindings, or skip C tests.
        receipt["baseline"] = "CROSS_GUARD_CONFIRMED"
        manifest = crate / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[dependencies.gmp-mpfr-sys]\nversion = "=1.7.1"\ndefault-features = false\nfeatures = ["mpfr", "force-cross"]\n')
        lockfile = crate / "Cargo.lock"
        old = lockfile.read_text()
        import re
        new, count = re.subn(r'(name = "rawmocs"\nversion = "0\.4\.0"\ndependencies = \[\n)', r'\g<1> "gmp-mpfr-sys",\n', old)
        assert count == 1
        lockfile.write_text(new)
        changed = tomllib.loads(new)
        expected = original_lock
        for p in expected["package"]:
            if p["name"] == "rawmocs":
                p["dependencies"] = sorted(p["dependencies"] + ["gmp-mpfr-sys"])
        assert changed == expected
        shutil.copy2(manifest, EVIDENCE / "experimental.Cargo.toml")
        shutil.copy2(lockfile, EVIDENCE / "experimental.Cargo.lock")
        if run("documented-force-cross-build", build, cwd=crate, env=env, seconds=360):
            receipt["status"] = "FORCE_CROSS_BUILD_BLOCKED"
            return
        receipt["build_mode"] = "experimental_force_cross"
    else:
        receipt["build_mode"] = "unmodified"
    out = WORK / "target" / TARGET / "release/examples/w1_probe.js"
    wasm = out.with_suffix(".wasm")
    assert out.is_file() and wasm.is_file() and wasm.read_bytes()[:4] == b"\x00asm"
    receipt["wasm_sha256"] = hashlib.sha256(wasm.read_bytes()).hexdigest()
    if run("wasm-mpfr-smoke", ["node", str(out)], seconds=30):
        receipt["status"] = "WASM_RUNTIME_BLOCKED"
        return
    wasm_log = (EVIDENCE / "wasm-mpfr-smoke.log").read_text()
    assert "W1_MPFR_SMOKE_PASS cases=12 failure_controls=2 reentry=1" in wasm_log
    assert len([s for s in wasm_log.splitlines() if s.startswith("W1_COUNT ")]) == 12
    receipt["status"] = "WASM_NODE_SMOKE_PASS"
    # Native parity with the same released crate and identical in-memory input.
    native_env = dict(os.environ, CARGO_TARGET_DIR=str(WORK / "native-target"))
    native = ["cargo", "build", "--offline", "--locked", "--release", "--example", "w1_probe", "-j", "2"]
    if run("native-build", native, cwd=crate, env=native_env, seconds=180):
        return
    if run("native-mpfr-smoke", [str(WORK / "native-target/release/examples/w1_probe")], seconds=30):
        return
    native_log = (EVIDENCE / "native-mpfr-smoke.log").read_text()
    assert "W1_MPFR_SMOKE_PASS cases=12 failure_controls=2 reentry=1" in native_log
    counts = lambda s: [x for x in s.splitlines() if x.startswith("W1_COUNT ")]
    assert counts(wasm_log) == counts(native_log)
    receipt["status"] = "W1_NODE_NATIVE_SMOKE_PASS"

try:
    main()
finally:
    save()
    print("W1_RESULT", receipt["status"], flush=True)
# A diagnostic failure is not a successful portability admission.
sys.exit(0 if receipt["status"] == "W1_NODE_NATIVE_SMOKE_PASS" else 1)
