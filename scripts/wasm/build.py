"""Build the current canonical public Rust package for native and WASM targets.
Read-only admission candidate; never publishes or downloads a previous solver.
"""
import hashlib, json, os, re, shutil, subprocess, sys, tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parent
WORK=Path(sys.argv[1]).resolve()
E=WORK/"evidence";E.mkdir(parents=True)
A=E/"candidate";A.mkdir()
REPO=ROOT.parents[1]
TARGET="wasm32-unknown-emscripten"
receipt={"status":"HOLD","target":TARGET,"steps":{}}
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():(E/"receipt.json").write_text(json.dumps(receipt,indent=2)+"\n")
def run(name,cmd,cwd=None,env=None,timeout=60):
    with (E/(name+".log")).open("wb") as log:
        try:r=subprocess.run(cmd,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=timeout);rc=r.returncode
        except subprocess.TimeoutExpired:rc=124
    receipt["steps"][name]=rc;save()
    print("WASM_STEP",name,rc,flush=True)
    if rc:
        print((E/(name+".log")).read_text(errors="replace")[-8000:],flush=True)
        raise RuntimeError(name)
def main():
    for name,cmd in [("rustc",["rustc","-vV"]),("emcc",["emcc","--version"]),("node",["node","--version"])]:
        run(name,cmd)
    source_dir=E/"source";source_dir.mkdir()
    crate=source_dir/"rawmocs"
    run("stage",[sys.executable,str(REPO/"scripts/stage_single_rust_crate.py"),str(REPO),str(crate)])
    shutil.copy2(E/"stage.log",E/"source-map.json")
    source=json.loads((E/"source-map.json").read_text())
    receipt["revision"]=source["revision"]
    receipt["version"]=source["version"]
    shutil.copy2(REPO/"packaging/rawmocs/Cargo.lock",crate/"Cargo.lock")
    native_target=WORK/"native-target"
    native_env=dict(os.environ,CARGO_TARGET_DIR=str(native_target))
    run("native-build",["cargo","build","--locked","--release","--bin","rawmocs","-j","2"],crate,native_env,timeout=360)
    native=native_target/"release/rawmocs"
    assert subprocess.check_output([str(native),"--version"],text=True).strip()=="rawmocs "+receipt["version"]
    source_hash={str(p.relative_to(crate)):digest(p) for p in sorted((crate/"src").rglob("*.rs"))}
    manifest=crate/"Cargo.toml";lock=crate/"Cargo.lock"
    shutil.copy2(manifest,E/"source.Cargo.toml");shutil.copy2(lock,E/"source.Cargo.lock")
    original=tomllib.loads(lock.read_text());packages={p["name"]:p for p in original["package"]}
    assert packages["rug"]["version"]=="1.30.0"
    assert packages["gmp-mpfr-sys"]["version"]=="1.7.1" and packages["gmp-mpfr-sys"]["checksum"]=="7db155b537cb791b133341f99f68371d86ee7fa4c79aacfbc376d72d23c70531"
    manifest.write_text(manifest.read_text()+'\n[dependencies.gmp-mpfr-sys]\nversion = "=1.7.1"\ndefault-features = false\nfeatures = ["mpfr", "force-cross"]\n')
    new,n=re.subn(r'(name = "rawmocs"\nversion = "'+re.escape(receipt["version"])+r'"\ndependencies = \[\n)',r'\g<1> "gmp-mpfr-sys",\n',lock.read_text())
    assert n==1
    lock.write_text(new)
    packages["rawmocs"]["dependencies"]=sorted(packages["rawmocs"]["dependencies"]+["gmp-mpfr-sys"])
    assert tomllib.loads(new)==original
    shutil.copy2(manifest,E/"build.Cargo.toml");shutil.copy2(lock,E/"build.Cargo.lock")
    run("locked-fetch",["cargo","fetch","--locked"],crate,timeout=90)
    target=WORK/"target"
    flags=["-sMODULARIZE=1","-sEXPORT_NAME=createRawmocs","-sENVIRONMENT=worker","-sEXPORTED_RUNTIME_METHODS=FS,callMain","-sEXIT_RUNTIME=1","-sALLOW_MEMORY_GROWTH=1","-sMAXIMUM_MEMORY=536870912","-sSTACK_SIZE=16777216","-sINITIAL_MEMORY=33554432","-sSTACK_OVERFLOW_CHECK=2"]
    config={"target":TARGET,"link_flags":flags,"source_date_epoch":subprocess.check_output(["git","-C",str(REPO),"show","-s","--format=%ct",receipt["revision"]],text=True).strip()}
    (source_dir/"build-config.json").write_text(json.dumps(config,indent=2)+"\n")
    for name in ("source.Cargo.toml","source.Cargo.lock","build.Cargo.toml","build.Cargo.lock","source-map.json"):
        shutil.copy2(E/name,source_dir/name)
    shutil.copytree(ROOT,source_dir/"recipe",ignore=shutil.ignore_patterns("__pycache__"))
    shutil.copy2(REPO/"scripts/stage_single_rust_crate.py",source_dir/"recipe/stage_single_rust_crate.py")
    run("vendor",["cargo","vendor","--locked","--offline",str(source_dir/"vendor")],crate,timeout=120)
    # Both builds use exactly the corresponding source and the same vendored
    # source replacement, never compare registry-cache and vendor paths.
    executable=sorted(str(p.relative_to(source_dir)) for p in source_dir.rglob("*") if p.is_file() and p.stat().st_mode&0o111)
    (source_dir/"executable-files.json").write_text(json.dumps(executable,indent=2)+"\n")
    source_inventory={str(p.relative_to(source_dir)):digest(p) for p in sorted(source_dir.rglob("*")) if p.is_file()}
    builds=[]
    for i in (1,2):
        # Exercise the permissions of a ZIP-extracted source on replay.
        if i==2:
            for p in source_dir.rglob("*"):
                if p.is_file():p.chmod(0o644)
        run(f"clean-build-{i}",[sys.executable,str(source_dir/"recipe/replay.py"),str(source_dir),str(target)],timeout=360)
        assert "WASM_OFFLINE_VENDOR_REPLAY_PASS" in (E/f"clean-build-{i}.log").read_text()
        assert {str(p.relative_to(source_dir)):digest(p) for p in sorted(source_dir.rglob("*")) if p.is_file()}==source_inventory
        receipt["offline_vendor_replay"]=i==2
        receipt["offline_vendor_builds"]=i
        out=target/TARGET/"release"
        files={name:digest(out/name) for name in ("rawmocs.js","rawmocs.wasm")}
        assert (out/"rawmocs.wasm").read_bytes()[:4]==b"\x00asm"
        builds.append(files);receipt["builds"]=builds;save()
        print("WASM_BUILD_DIGEST",i,json.dumps(files,sort_keys=True),flush=True)
        if i==1:
            for name in files:shutil.copy2(out/name,A/name)
        else:
            differences={}
            for name in files:
                if files[name]!=builds[0][name]:
                    left=(A/name).read_bytes();right=(out/name).read_bytes()
                    offset=next((j for j,(a,b) in enumerate(zip(left,right)) if a!=b),min(len(left),len(right)))
                    differences[name]={"first_different_offset":offset,"first_size":len(left),"second_size":len(right)}
                    hold=E/"reproducibility-hold";hold.mkdir(exist_ok=True)
                    shutil.copy2(out/name,hold/name)
            receipt["build_differences"]=differences
            print("WASM_BUILD_DIFFERENCES",json.dumps(differences,sort_keys=True),flush=True)
        assert {str(p.relative_to(crate)):digest(p) for p in sorted((crate/"src").rglob("*.rs"))}==source_hash
        shutil.rmtree(target)
    receipt["reproducible_same_environment"]=builds[0]==builds[1];save()
    shutil.copy2(ROOT/"worker.js",A/"worker.js")
    (A/"index.html").write_text("<!doctype html><title>RaWMoCS worker gate</title>")
    receipt["source_hashes"]=source_hash
    receipt["artifacts"]={n:digest(A/n) for n in ("rawmocs.js","rawmocs.wasm","worker.js")}
    receipt["link_flags"]=flags
    run("real-browser",[sys.executable,str(ROOT/"browser_test.py"),str(A),str(native)],timeout=180)
    assert "WASM_BROWSER_WORKER_PASS" in (E/"real-browser.log").read_text()
    if not receipt["reproducible_same_environment"]:
        receipt["status"]="BROWSER_PASS_REPRODUCIBILITY_HOLD";return
    # Test-only instrumentation; never ship this variant.
    resource=WORK/"resource-test"; shutil.copytree(A,resource)
    worker=(resource/"worker.js").read_text(); anchor="let status=0;"
    assert worker.count(anchor)==1
    (resource/"resource-test-worker.js").write_text(worker.replace(anchor,'self.postMessage({type:"started",id:request.id}); '+anchor))
    run("resource-browser",[sys.executable,str(ROOT/"resources.py"),str(resource)],timeout=90)
    assert "WASM_RESOURCE_SCOPED_PASS" in (E/"resource-browser.log").read_text()
    shutil.copy2(resource/"resource-results.json",E/"resource-results.json")
    shutil.move(A/"browser-results.json",E/"browser-results.json")
    (A/"index.html").unlink()
    licenses=sorted(str(p.relative_to(source_dir)) for p in source_dir.rglob("*") if p.is_file() and p.name.upper().startswith(("LICENSE","COPYING","NOTICE")))
    assert any("gmp-mpfr-sys" in p for p in licenses)
    receipt["license_inventory"]=licenses
    receipt["source_sha256"]={str(p.relative_to(source_dir)):digest(p) for p in sorted(source_dir.rglob("*")) if p.is_file()}
    receipt["status"]="UNIFIED_WASM_CANDIDATE_PASS"
try:main()
finally:
    save();print("WASM_RESULT",receipt["status"],flush=True)
sys.exit(0 if receipt["status"]=="UNIFIED_WASM_CANDIDATE_PASS" else 1)
