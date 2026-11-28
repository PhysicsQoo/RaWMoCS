"""W2a: exact W1c solver binary, original QuPRS phase transport, native/browser parity."""
import hashlib,json,os,re,shutil,subprocess,sys,tarfile,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent
W=Path(sys.argv[1]).resolve(); Q=Path(sys.argv[2]).resolve(); E=W/"evidence"; E.mkdir(exist_ok=True)
R={"status":"HOLD","quprs":"2a510a7bba0d5af612c84fc2822cc29bcea45daa","w1c_artifact":15257956508}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(name,cmd,cwd=None,seconds=120):
    with (E/(name+".log")).open("wb") as f:
        r=subprocess.run(cmd,cwd=cwd,stdout=f,stderr=subprocess.STDOUT,timeout=seconds)
    if r.returncode:
        print((E/(name+".log")).read_text(errors="replace")[-5000:])
        raise RuntimeError(name)
def main():
    assert sha(W/"w1c.zip")=="9165b990cc77f7a625c27b66eda4cde6521509caaffabac48ad7a767f24feeef"
    P=W/"prior";P.mkdir()
    with zipfile.ZipFile(W/"w1c.zip") as z:
        for n in z.namelist():assert not Path(n).is_absolute() and ".." not in Path(n).parts
        z.extractall(P)
    receipts=[p for p in P.rglob("receipt.json") if json.loads(p.read_text()).get("status")=="W1C_SCOPED_DISTRIBUTION_CANDIDATE_PASS"]
    assert len(receipts)==1
    base=receipts[0].parent;prior=json.loads(receipts[0].read_text())
    D=W/"browser";D.mkdir()
    for name,digest in prior["distribution_sha256"].items():
        src=base/"distribution"/name;assert sha(src)==digest,name;shutil.copy2(src,D/name)
    assert subprocess.check_output(["git","rev-parse","HEAD"],cwd=Q,text=True).strip()==R["quprs"]
    R["solver_sha256"]=prior["distribution_sha256"]
    R["original_phase_sha256"]={n:sha(Q/n) for n in ("scripts/web/PhaseWeights.hpp","rust/quprs/native/mpfr_weight_bridge.c")}
    # Existing QuPRS build recipe supplies pinned GMP/MPFR and original GPMC comparator.
    run("quprs-counter",["bash","scripts/web/build-counter.sh"],Q,seconds=800)
    prefix=Q/".browser-prefix";c=Q/"rust/quprs/native/mpfr_weight_bridge.c";h=Q/"scripts/web"
    run("phase-c-native",["cc","-O2","-c",str(c),"-o",str(W/"phase-native.o")])
    run("phase-native",["c++","-O2","-I"+str(h),str(ROOT/"bridge.cc"),str(W/"phase-native.o"),"-lmpfr","-lgmpxx","-lgmp","-o",str(W/"phase-native")])
    run("phase-c-wasm",["emcc","-O2","-I"+str(prefix/"include"),"-c",str(c),"-o",str(W/"phase.o")])
    run("phase-wasm",["em++","-O2","-fexceptions","-I"+str(h),"-I"+str(prefix/"include"),str(ROOT/"bridge.cc"),str(W/"phase.o"),"-L"+str(prefix/"lib"),"-lmpfr","-lgmpxx","-lgmp","-sMODULARIZE=1","-sEXPORT_NAME=createPhaseBridge","-sENVIRONMENT=worker","-sEXPORTED_RUNTIME_METHODS=FS,callMain","-sALLOW_MEMORY_GROWTH=1","-sMAXIMUM_MEMORY=67108864","-sINITIAL_MEMORY=33554432","-o",str(D/"phase.js")])
    shutil.copy2(ROOT/"phase-worker.js",D/"phase-worker.js")
    (D/"index.html").write_text("<!doctype html><title>W2 phase parity</title>")
    for n in ("gpmc.js","gpmc.wasm","runner.js","output.js"):shutil.copy2(Q/"web/engines"/n,D/n)
    crate=base/"source/rawmocs-0.4.0.crate"
    assert sha(crate)=="d3e9e6ccd4a146bac19f6bd2628038cf20ae331dd12f9dc0cbcd37a4e63e955d"
    with tarfile.open(crate) as t:t.extractall(W,filter="data")
    C=W/"rawmocs-0.4.0"
    run("native-rawmocs",["cargo","build","--locked","--release","--bin","rawmocs","-j","2"],C,seconds=360)
    run("browser-parity",[sys.executable,str(ROOT/"parity.py"),str(D),str(W/"phase-native"),str(Q/"web/engines/native-gpmc"),str(C/"target/release/rawmocs"),str(E)],seconds=160)
    assert "W2A_PHASE_PARITY_PASS" in (E/"browser-parity.log").read_text()
    R["bridge_sha256"]={n:sha(D/n) for n in ("phase.js","phase.wasm","phase-worker.js")}
    for n in R["bridge_sha256"]:shutil.copy2(D/n,E/n)
    R["status"]="W2A_SCOPED_PHASE_PARITY_PASS"
try:main()
finally:
    (E/"receipt.json").write_text(json.dumps(R,indent=2)+"\n")
    print("W2A_RESULT",R["status"],flush=True)
sys.exit(0 if R["status"]=="W2A_SCOPED_PHASE_PARITY_PASS" else 1)
