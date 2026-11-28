"""Extend exact admitted browser artifact. No repeat of W1b compiler builds."""
import hashlib,json,os,shutil,subprocess,sys,tarfile,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent
W=Path(sys.argv[1]); E=W/"evidence"; E.mkdir(exist_ok=True)
R={"status":"HOLD","upstream_run":46129610268,"upstream_artifact":15256299864}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():(E/"receipt.json").write_text(json.dumps(R,indent=2)+"\n")
def run(name,args,cwd=None,seconds=90):
    with (E/(name+".log")).open("wb") as f:
        r=subprocess.run(args,cwd=cwd,stdout=f,stderr=subprocess.STDOUT,timeout=seconds)
    if r.returncode:
        print((E/(name+".log")).read_text(errors="replace")[-6000:])
        raise RuntimeError(name)
def main():
    assert sha(W/"w1b.zip")=="d8ccbdb5a9ae50e2004da71eabdb91e37c19128cf149d863eb5f6c8e2c412454"
    P=W/"prior";P.mkdir()
    with zipfile.ZipFile(W/"w1b.zip") as z:
        for n in z.namelist():
            assert not Path(n).is_absolute() and ".." not in Path(n).parts
        z.extractall(P)
    receipt_files=[p for p in P.rglob("receipt.json") if json.loads(p.read_text()).get("status")=="W1B_SCOPED_BROWSER_REPRO_PASS"]
    assert len(receipt_files)==1
    B=receipt_files[0].parent; prior=json.loads(receipt_files[0].read_text());A=B/"candidate"
    assert prior["reproducible_same_environment"] and prior["builds"][0]==prior["builds"][1]
    for name,d in prior["artifacts"].items():assert sha(A/name)==d,name
    assert sha(A/"rawmocs.wasm")==prior["builds"][0]["rawmocs.wasm"]
    D=E/"distribution";D.mkdir()
    for n in ("rawmocs.js","rawmocs.wasm","worker.js"):shutil.copy2(A/n,D/n)
    (D/"index.html").write_text("<!doctype html><title>Resource admission</title>")
    shutil.copy2(receipt_files[0],E/"w1b-receipt.json")
    # The test worker differs only by an explicit solve-entry notification.
    # Production candidate worker stays byte-identical to the admitted artifact.
    s=(D/"worker.js").read_text();anchor="let status=0;"
    assert s.count(anchor)==1
    (D/"resource-test-worker.js").write_text(s.replace(anchor,'self.postMessage({type:"started",id:request.id}); '+anchor))
    run("resource-browser",[sys.executable,str(ROOT/"resources.py"),str(D)],seconds=90)
    assert "W1C_RESOURCE_SCOPED_PASS" in (E/"resource-browser.log").read_text()
    shutil.copy2(D/"resource-results.json",E/"resource-results.json")
    (D/"resource-results.json").unlink();(D/"resource-test-worker.js").unlink();(D/"index.html").unlink()
    # Source candidate: exact original crate and all lock-pinned vendored sources.
    S=E/"source";S.mkdir()
    crate=S/"rawmocs-0.4.0.crate"
    run("crate",["curl","-fL","--retry","2","https://static.crates.io/crates/rawmocs/rawmocs-0.4.0.crate","-o",str(crate)])
    assert sha(crate)==prior["crate_sha256"]
    with tarfile.open(crate) as t:t.extractall(W,filter="data")
    C=W/"rawmocs-0.4.0"
    for name in ("released.Cargo.toml","released.Cargo.lock","build.Cargo.toml","build.Cargo.lock"):
        shutil.copy2(B/name,S/name)
    assert (C/"Cargo.lock").read_bytes()==(S/"released.Cargo.lock").read_bytes()
    assert (C/"Cargo.toml").read_bytes()==(S/"released.Cargo.toml").read_bytes()
    shutil.copy2(S/"build.Cargo.toml",C/"Cargo.toml");shutil.copy2(S/"build.Cargo.lock",C/"Cargo.lock")
    run("vendor",["cargo","vendor","--locked",str(S/"vendor")],C,seconds=120)
    for name in ("rug","gmp-mpfr-sys"):assert (S/"vendor"/name).is_dir()
    licenses=[str(p.relative_to(S)) for p in S.rglob("*") if p.is_file() and p.name.upper().startswith(("LICENSE","COPYING","NOTICE"))]
    assert any("gmp-mpfr-sys" in x for x in licenses)
    for n in ("build.py","worker.js","browser_test.py","README.md"):
        shutil.copy2(ROOT.parent/"wasm_w1b"/n,S/n)
    shutil.copy2(ROOT/"closeout.py",S/"closeout.py");shutil.copy2(ROOT/"resources.py",S/"resources.py")
    (S/"SOURCE-NOTES.md").write_text("Exact registry crate, lock-pinned vendored dependencies and the W1b build recipe. Emscripten and Rust system toolchains are separately required, versions in W1b receipt/logs. Source/license inventory is a candidate for redistribution review, not a legal compliance certification. No publication.\n")
    R["license_inventory"]=sorted(licenses)
    R["source_sha256"]={str(p.relative_to(S)):sha(p) for p in sorted(S.rglob("*")) if p.is_file()}
    R["distribution_sha256"]={p.name:sha(p) for p in D.iterdir()}
    R["status"]="W1C_SCOPED_DISTRIBUTION_CANDIDATE_PASS"
try:main()
finally:save();print("W1C_RESULT",R["status"],flush=True)
sys.exit(0 if R["status"]=="W1C_SCOPED_DISTRIBUTION_CANDIDATE_PASS" else 1)
