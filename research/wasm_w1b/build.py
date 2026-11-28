"""W1b: immutable released crate; two clean same-environment builds, real browser."""
import hashlib, json, os, re, shutil, subprocess, sys, tarfile, tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parent
WORK=Path(sys.argv[1]).resolve()
E=WORK/"evidence";E.mkdir(parents=True)
A=E/"candidate";A.mkdir()
SHA="d3e9e6ccd4a146bac19f6bd2628038cf20ae331dd12f9dc0cbcd37a4e63e955d"
TARGET="wasm32-unknown-emscripten"
receipt={"status":"HOLD","crate_sha256":SHA,"version":"0.4.0","target":TARGET,"steps":{}}
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():(E/"receipt.json").write_text(json.dumps(receipt,indent=2)+"\n")
def run(name,cmd,cwd=None,env=None,timeout=60):
    with (E/(name+".log")).open("wb") as log:
        try:r=subprocess.run(cmd,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=timeout);rc=r.returncode
        except subprocess.TimeoutExpired:rc=124
    receipt["steps"][name]=rc;save()
    print("W1B_STEP",name,rc,flush=True)
    if rc:
        print((E/(name+".log")).read_text(errors="replace")[-8000:],flush=True)
        raise RuntimeError(name)
def main():
    for name,cmd in [("rustc",["rustc","-vV"]),("emcc",["emcc","--version"]),("node",["node","--version"])]:
        run(name,cmd)
    archive=WORK/"rawmocs-0.4.0.crate"
    run("download",["curl","-fL","--retry","2","https://static.crates.io/crates/rawmocs/rawmocs-0.4.0.crate","-o",str(archive)])
    assert digest(archive)==SHA
    with tarfile.open(archive) as t:t.extractall(WORK,filter="data")
    crate=WORK/"rawmocs-0.4.0"
    source_hash={str(p.relative_to(crate)):digest(p) for p in sorted((crate/"src").rglob("*.rs"))}
    manifest=crate/"Cargo.toml";lock=crate/"Cargo.lock"
    shutil.copy2(manifest,E/"released.Cargo.toml");shutil.copy2(lock,E/"released.Cargo.lock")
    original=tomllib.loads(lock.read_text());packages={p["name"]:p for p in original["package"]}
    assert packages["rug"]["version"]=="1.30.0"
    assert packages["gmp-mpfr-sys"]["version"]=="1.7.1" and packages["gmp-mpfr-sys"]["checksum"]=="7db155b537cb791b133341f99f68371d86ee7fa4c79aacfbc376d72d23c70531"
    manifest.write_text(manifest.read_text()+'\n[dependencies.gmp-mpfr-sys]\nversion = "=1.7.1"\ndefault-features = false\nfeatures = ["mpfr", "force-cross"]\n')
    new,n=re.subn(r'(name = "rawmocs"\nversion = "0\.4\.0"\ndependencies = \[\n)',r'\g<1> "gmp-mpfr-sys",\n',lock.read_text())
    assert n==1
    lock.write_text(new)
    packages["rawmocs"]["dependencies"]=sorted(packages["rawmocs"]["dependencies"]+["gmp-mpfr-sys"])
    assert tomllib.loads(new)==original
    shutil.copy2(manifest,E/"build.Cargo.toml");shutil.copy2(lock,E/"build.Cargo.lock")
    run("locked-fetch",["cargo","fetch","--locked"],crate,timeout=90)
    target=WORK/"target"
    env=dict(os.environ,CC="emcc",CXX="em++",AR="emar",RANLIB="emranlib",CC_FOR_BUILD="gcc",CFLAGS_FOR_BUILD="-O2",
      CC_wasm32_unknown_emscripten="emcc",AR_wasm32_unknown_emscripten="emar",CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER="emcc",
      GMP_MPFR_SYS_CACHE="_",CARGO_TARGET_DIR=str(target),SOURCE_DATE_EPOCH="1795824587")
    flags=["-sMODULARIZE=1","-sEXPORT_NAME=createRawmocs","-sENVIRONMENT=worker","-sEXPORTED_RUNTIME_METHODS=FS,callMain","-sEXIT_RUNTIME=1","-sALLOW_MEMORY_GROWTH=1","-sMAXIMUM_MEMORY=536870912","-sSTACK_SIZE=16777216","-sINITIAL_MEMORY=33554432","-sSTACK_OVERFLOW_CHECK=2"]
    cmd=["cargo","rustc","--offline","--locked","--release","--target",TARGET,"--bin","rawmocs","-j","2","--"]
    for flag in flags:cmd+=["-C","link-arg="+flag]
    builds=[]
    for i in (1,2):
        run(f"clean-build-{i}",cmd,crate,env,timeout=360)
        out=target/TARGET/"release"
        files={name:digest(out/name) for name in ("rawmocs.js","rawmocs.wasm")}
        assert (out/"rawmocs.wasm").read_bytes()[:4]==b"\x00asm"
        builds.append(files);receipt["builds"]=builds;save()
        if i==1:
            for name in files:shutil.copy2(out/name,A/name)
        assert {str(p.relative_to(crate)):digest(p) for p in sorted((crate/"src").rglob("*.rs"))}==source_hash
        shutil.rmtree(target)
    receipt["reproducible_same_environment"]=builds[0]==builds[1];save()
    shutil.copy2(ROOT/"worker.js",A/"worker.js")
    (A/"index.html").write_text("<!doctype html><title>RaWMoCS worker gate</title>")
    receipt["source_hashes"]=source_hash
    receipt["artifacts"]={p.name:digest(p) for p in sorted(A.iterdir())}
    receipt["link_flags"]=flags
    run("real-browser",[sys.executable,str(ROOT/"browser_test.py"),str(A)],timeout=180)
    assert "W1B_BROWSER_WORKER_PASS" in (E/"real-browser.log").read_text()
    if not receipt["reproducible_same_environment"]:
        receipt["status"]="BROWSER_PASS_REPRODUCIBILITY_HOLD";return
    receipt["status"]="W1B_SCOPED_BROWSER_REPRO_PASS"
try:main()
finally:
    save();print("W1B_RESULT",receipt["status"],flush=True)
sys.exit(0 if receipt["status"]=="W1B_SCOPED_BROWSER_REPRO_PASS" else 1)
