"""Rebuild an explicitly unpublishable candidate from the admitted parser fix.
Reuse release crate and exact dependency lock; no changes to numerical kernels.
"""
import hashlib,json,os,re,shutil,subprocess,sys,tarfile,tomllib
from pathlib import Path
FIX="40b83fd2a70ce0608fece16bdfca7870db1cc7c3"
VERSION="0.4.1-alpha.1"
TARGET="wasm32-unknown-emscripten"
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(name,args,cwd=None,env=None,seconds=360):
    with (E/(name+".log")).open("wb") as f:
        r=subprocess.run(args,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=seconds)
    if r.returncode:
        print((E/(name+".log")).read_text(errors="replace")[-6000:])
        raise RuntimeError(name)
W,base,fix,D,E=map(lambda s:Path(s).resolve(),sys.argv[1:])
R={"status":"HOLD","version":VERSION,"fix":FIX}
def main():
    assert subprocess.check_output(["git","rev-parse","HEAD"],cwd=fix,text=True).strip()==FIX
    archive=base/"source/rawmocs-0.4.0.crate"
    assert sha(archive)=="d3e9e6ccd4a146bac19f6bd2628038cf20ae331dd12f9dc0cbcd37a4e63e955d"
    with tarfile.open(archive) as t:t.extractall(W,filter="data")
    C=W/"rawmocs-0.4.0";stage=W/"staged-fix"
    run("stage-fix",[sys.executable,str(fix/"scripts/stage_single_rust_crate.py"),str(fix),str(stage)])
    changed=[]
    for p in sorted((C/"src").glob("*.rs")):
        q=stage/"src"/p.name
        if p.read_bytes()!=q.read_bytes():
            changed.append(p.name);shutil.copy2(q,p)
    assert changed==["factor_prejoin.rs","lib.rs"],changed
    assert {p.name for p in (C/"src").glob("*.rs")}=={p.name for p in (stage/"src").glob("*.rs")}
    R["changed_sources"]=changed
    R["source_hashes"]={p.name:sha(p) for p in sorted((C/"src").glob("*.rs"))}
    manifest=C/"Cargo.toml";lock=C/"Cargo.lock"
    old=manifest.read_text()
    new,n=re.subn(r'(?m)^version = "0\.4\.0"$','version = "'+VERSION+'"',old,count=1);assert n==1
    # Normalized crates.io manifest may omit publish; forbid publishing this private candidate.
    if re.search(r'(?m)^publish = .*$',new):new=re.sub(r'(?m)^publish = .*$',"publish = false",new,count=1)
    else:new=new.replace("[package]","[package]\npublish = false",1)
    manifest.write_text(new)
    orig=tomllib.loads(lock.read_text())
    new,n=re.subn(r'(name = "rawmocs"\nversion = ")0\.4\.0(")',r'\g<1>'+VERSION+r'\g<2>',lock.read_text());assert n==1
    lock.write_text(new)
    packages={p["name"]:p for p in orig["package"]};packages["rawmocs"]["version"]=VERSION
    assert tomllib.loads(new)==orig
    shutil.copy2(manifest,E/"candidate-native.Cargo.toml");shutil.copy2(lock,E/"candidate-native.Cargo.lock")
    run("candidate-native-tests",["cargo","test","--locked","--release","--lib","source_phase_decimal_input","--","--nocapture"],C)
    run("candidate-native-build",["cargo","build","--locked","--release","--bin","rawmocs","-j","2"],C)
    native=W/"candidate-native";shutil.copy2(C/"target/release/rawmocs",native)
    R["native_sha256"]=sha(native)
    assert packages["rug"]["version"]=="1.30.0"
    assert packages["gmp-mpfr-sys"]["version"]=="1.7.1" and packages["gmp-mpfr-sys"]["checksum"]=="7db155b537cb791b133341f99f68371d86ee7fa4c79aacfbc376d72d23c70531"
    manifest.write_text(manifest.read_text()+'\n[dependencies.gmp-mpfr-sys]\nversion = "=1.7.1"\ndefault-features = false\nfeatures = ["mpfr", "force-cross"]\n')
    anchor='name = "rawmocs"\nversion = "'+VERSION+'"\ndependencies = [\n'
    assert lock.read_text().count(anchor)==1
    lock.write_text(lock.read_text().replace(anchor,anchor+' "gmp-mpfr-sys",\n'))
    packages["rawmocs"]["dependencies"]=sorted(packages["rawmocs"]["dependencies"]+["gmp-mpfr-sys"])
    assert tomllib.loads(lock.read_text())==orig
    shutil.copy2(manifest,E/"candidate-wasm.Cargo.toml");shutil.copy2(lock,E/"candidate-wasm.Cargo.lock")
    run("candidate-locked-fetch",["cargo","fetch","--locked"],C,seconds=90)
    target=W/"candidate-wasm-target"
    env=dict(os.environ,CC="emcc",CXX="em++",AR="emar",RANLIB="emranlib",CC_FOR_BUILD="gcc",CFLAGS_FOR_BUILD="-O2",
      CC_wasm32_unknown_emscripten="emcc",AR_wasm32_unknown_emscripten="emar",CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER="emcc",
      GMP_MPFR_SYS_CACHE="_",CARGO_TARGET_DIR=str(target),SOURCE_DATE_EPOCH="1795824587")
    flags=["-sMODULARIZE=1","-sEXPORT_NAME=createRawmocs","-sENVIRONMENT=worker","-sEXPORTED_RUNTIME_METHODS=FS,callMain","-sEXIT_RUNTIME=1","-sALLOW_MEMORY_GROWTH=1","-sMAXIMUM_MEMORY=536870912","-sSTACK_SIZE=16777216","-sINITIAL_MEMORY=33554432","-sSTACK_OVERFLOW_CHECK=2"]
    cmd=["cargo","rustc","--offline","--locked","--release","--target",TARGET,"--bin","rawmocs","-j","2","--"]
    for flag in flags:cmd+=["-C","link-arg="+flag]
    R["builds"]=[]
    for i in (1,2):
        run(f"candidate-clean-wasm-{i}",cmd,C,env,seconds=420)
        out=target/TARGET/"release"
        hashes={n:sha(out/n) for n in ("rawmocs.js","rawmocs.wasm")}
        R["builds"].append(hashes)
        assert (out/"rawmocs.wasm").read_bytes()[:4]==b"\x00asm"
        if i==1:
            for n in hashes:shutil.copy2(out/n,D/n)
        assert {p.name:sha(p) for p in sorted((C/"src").glob("*.rs"))}==R["source_hashes"]
        shutil.rmtree(target)
    assert R["builds"][0]==R["builds"][1"],R["builds"]
    R["distribution_sha256"]={n:sha(D/n) for n in ("rawmocs.js","rawmocs.wasm","worker.js")}
    bundle=E/"candidate-distribution";bundle.mkdir()
    for n in R["distribution_sha256"]:shutil.copy2(D/n,bundle/n)
    # Retain exact source and build recipe, not a published release.
    S=E/"candidate-source";S.mkdir()
    shutil.copytree(C/"src",S/"src");shutil.copy2(manifest,S/"Cargo.toml");shutil.copy2(lock,S/"Cargo.lock")
    R["status"]="CANDIDATE_REPRODUCIBLE_BUILD_PASS"
try:main()
finally:
    (E/"candidate-receipt.json").write_text(json.dumps(R,indent=2)+"\n")
    print("W2_CANDIDATE_RESULT",R["status"],flush=True)
sys.exit(0 if R["status"]=="CANDIDATE_REPRODUCIBLE_BUILD_PASS" else 1)
