"""RC-only first registry registration, based on exact admitted main source.
This module never publishes the stable version, Python package or GitHub release.
"""
import json, os, re, subprocess, sys, time, tomllib
from pathlib import Path
from urllib.error import HTTPError, URLError
from release_source import REPO, digest, require, run
from release_v040 import prepare as admit_base, files, get, CRATE as BASE_CRATE

V="0.4.0-rc.1"
CRATE=f"rawmocs-{V}.crate"
BASE="273f45b83c55b28871350c32ba5070bf77928688"
REQUEST=Path("releases/rc-bootstrap-base.json")
CARGO="1.100.1"

def base_request():
    r=json.loads(REQUEST.read_text())
    require(r["revision"]==BASE and r["version"]=="0.4.0","wrong pinned source")
    return r

def replace_version(data, package):
    pat=rb'(?m)(^name = "'+package.encode()+rb'"\nversion = ")0\.4\.0(")$'
    updated,n=re.subn(pat,lambda m:m[1]+V.encode()+m[2],data)
    require(n==1,"unexpected version field: "+package)
    return updated

def check_rc_package(stable,rc):
    require(set(stable)==set(rc),"RC file set differs")
    for path,data in stable.items():
        expected=replace_version(data,"rawmocs") if path in ("Cargo.toml","Cargo.toml.orig","Cargo.lock") else data
        require(rc[path]==expected,"RC content differs: "+path)
    m=tomllib.loads(rc["Cargo.toml"].decode())
    require(m["package"]["name"]=="rawmocs" and m["package"]["version"]==V,"wrong RC identity")
    require(m["lib"]["name"]==m["bin"][0]["name"]=="rawmocs","wrong targets")

def no_token_env(root):
    env=os.environ.copy();env.pop("CARGO_REGISTRY_TOKEN",None)
    env["CARGO_TARGET_DIR"]=str(root/"target")
    return env

def package(root,verify):
    require(os.environ.get("GITHUB_REPOSITORY")==REPO,"wrong repository")
    require(run("cargo","--version").decode().split()[1]==CARGO,"wrong cargo version")
    r=base_request();assets=admit_base(r)
    stable=files(assets[BASE_CRATE],"rawmocs-0.4.0")
    root.mkdir(parents=True,exist_ok=False);e=root/"evidence";e.mkdir()
    checkout=root/"source";stage=root/"stage";env=no_token_env(root)
    subprocess.run(["git","worktree","add","--detach",str(checkout),BASE],env=env,check=True)
    try:
        subprocess.run([sys.executable,"scripts/stage_single_rust_crate.py",str(checkout),str(stage)],env=env,check=True)
    finally:
        subprocess.run(["git","worktree","remove","--force",str(checkout)],env=env,check=True)
    (stage/"Cargo.toml").write_bytes(replace_version((stage/"Cargo.toml").read_bytes(),"rawmocs"))
    (stage/"Cargo.lock").write_bytes(replace_version(stable["Cargo.lock"],"rawmocs"))
    args=["cargo","package","--locked","--manifest-path",str(stage/"Cargo.toml")]
    if not verify:args.append("--no-verify")
    subprocess.run(args,env=env,check=True)
    blob=(root/"target/package"/CRATE).read_bytes()
    check_rc_package(stable,files(blob,"rawmocs-"+V))
    (e/CRATE).write_bytes(blob)
    if verify:
        # Test the exact extracted archive, not a checkout or newly generated file.
        import tarfile,io
        with tarfile.open(fileobj=io.BytesIO(blob),mode="r:gz") as t:
            t.extractall(root/"extracted",filter="data")
        path=root/"extracted"/("rawmocs-"+V)/"Cargo.toml"
        subprocess.run(["cargo","test","--locked","--release","--manifest-path",str(path),"--test","shared_core","--test","native_stdio"],env=env,check=True)
        out=subprocess.check_output(["cargo","run","--locked","--release","--manifest-path",str(path),"--bin","rawmocs","--","--version"],env=env)
        require(out.decode().strip()=="rawmocs "+V,"RC CLI version mismatch")
        (e/"cli-version.txt").write_bytes(out)
    receipt={"version":V,"name":"rawmocs","source_revision":BASE,"base_admission_run":r["admission_run"],"base_crate_sha256":r["crate_sha256"],"rc_sha256":digest(blob),"tested":verify}
    (e/"admission.json").write_text(json.dumps(receipt,indent=2)+"\n")
    print("RC_PACKAGE_ADMISSION_PASS" if verify else "RC_PACKAGE_REPRODUCED",receipt,flush=True)
    return receipt

def reproduce(root,admitted):
    r=package(root,False)
    old=json.loads((admitted/"admission.json").read_text())
    require(old==dict(r,tested=True),"RC admission mismatch")
    require((admitted/CRATE).read_bytes()==(root/"evidence"/CRATE).read_bytes(),"RC archive differs")
    (root/"evidence/admission.json").write_text(json.dumps(old,indent=2)+"\n")

def admitted(root):
    r=json.loads((root/"evidence/admission.json").read_text())
    require(r["name"]=="rawmocs" and r["version"]==V and r["source_revision"]==BASE and r["tested"] is True,"unadmitted RC")
    b=(root/"target/package"/CRATE).read_bytes()
    require(digest(b)==r["rc_sha256"] and b==(root/"evidence"/CRATE).read_bytes(),"RC archive mismatch")
    m=tomllib.loads((root/"stage/Cargo.toml").read_text())
    require(m["package"]["name"]=="rawmocs" and m["package"]["version"]==V and m["package"]["publish"]==["crates-io"],"RC publication identity")
    return r

def registry_state(expected):
    a,_=get("https://crates.io/api/v1/crates/rawmocs")
    i,index=get("https://index.crates.io/ra/wm/rawmocs")
    if a==i==404:return "ABSENT"
    require(a==i==200,"registry state inconclusive")
    rows=[json.loads(x) for x in index.splitlines() if x]
    require(len(rows)==1 and rows[0]["vers"]==V and rows[0]["cksum"]==expected and rows[0]["yanked"] is False,"unexpected registered crate or version")
    status,blob=get("https://static.crates.io/crates/rawmocs/"+CRATE)
    require(status==200 and digest(blob)==expected,"RC registry archive mismatch")
    return "MATCH"

def check_registry(expected,attempts=6,delay=5):
    for n in range(attempts):
        try:return registry_state(expected)
        except (HTTPError,URLError,TimeoutError,ValueError):
            if n==attempts-1:raise
            time.sleep(delay)

def require_manual_main():
    require(os.environ.get("GITHUB_REPOSITORY")==REPO and os.environ.get("GITHUB_REF")=="refs/heads/main" and os.environ.get("GITHUB_EVENT_NAME")=="workflow_dispatch","explicit main dispatch only")

def publish_or_verify(root,mode):
    require_manual_main()
    r=admitted(root);sha=r["rc_sha256"]
    require(os.environ.get("CARGO_REGISTRY_TOKEN"),"registry credential unavailable")
    state=check_registry(sha)
    if state=="MATCH":
        print("RC_REGISTRY_ALREADY_MATCHES",sha)
    else:
        require(mode=="bootstrap" and state=="ABSENT","OIDC verification cannot create a crate")
        env=no_token_env(root)
        # Repackage with the same Cargo version and lock before exposing the token.
        subprocess.run(["cargo","package","--locked","--no-verify","--manifest-path",str(root/"stage/Cargo.toml")],env=env,check=True)
        admitted(root)
        require(check_registry(sha)=="ABSENT","registry changed before upload")
        env["CARGO_REGISTRY_TOKEN"]=os.environ["CARGO_REGISTRY_TOKEN"]
        subprocess.run(["cargo","publish","--locked","--no-verify","--registry","crates-io","--manifest-path",str(root/"stage/Cargo.toml")],env=env,check=True)
        # Never retry a write. Retry readback only.
        for n in range(20):
            try:
                if registry_state(sha)=="MATCH":break
            except (HTTPError,URLError,TimeoutError,ValueError):pass
            if n==19:raise ValueError("upload may have succeeded; inspect registry before retry")
            time.sleep(3)
    require(check_registry(sha)=="MATCH","registry readback failed")
    (root/"evidence/registry.json").write_text(json.dumps(dict(r,status="RC_REGISTRY_VERIFIED",mode=mode),indent=2)+"\n")
    print("RC_REGISTRY_VERIFIED",V,sha,mode,flush=True)

def consumer(root):
    require_manual_main()
    r=admitted(root);require(check_registry(r["rc_sha256"])=="MATCH","unverified registry")
    p=root/"consumer";p.mkdir();(p/"src").mkdir()
    (p/"Cargo.toml").write_text('[package]\nname="rc-registry-consumer"\nversion="0.0.0"\nedition="2021"\npublish=false\n[workspace]\n[dependencies]\nrawmocs="='+V+'"\n')
    (p/"src/main.rs").write_text('fn main(){let r=rawmocs::solve_text("p cnf 1 1\\nc t wmc\\n1 0\\n",rawmocs::SolveOptions::default()).unwrap();assert!(r.sat);let z=r.imag_mpfr().binary();assert_eq!(z.significand,"0");println!("RC_REGISTRY_CONSUMER_PASS");}\n')
    env=no_token_env(root)
    subprocess.run(["cargo","generate-lockfile","--manifest-path",str(p/"Cargo.toml")],env=env,check=True)
    lock=tomllib.loads((p/"Cargo.lock").read_text())
    v=[x for x in lock["package"] if x["name"]=="rawmocs"]
    require(len(v)==1 and v[0]["version"]==V and v[0]["source"].startswith("registry+") and v[0]["checksum"]==r["rc_sha256"],"registry consumer lock mismatch")
    subprocess.run(["cargo","run","--locked","--release","--manifest-path",str(p/"Cargo.toml")],env=env,check=True)
    (root/"evidence/registry-consumer.Cargo.lock").write_bytes((p/"Cargo.lock").read_bytes())
    print("RC_REGISTRY_CONSUMER_LOCK_PASS",flush=True)

if __name__=="__main__":
    mode,folder,*extra=sys.argv[1:]
    root=Path(folder).resolve()
    if mode=="prepare":package(root,True)
    elif mode=="reproduce":reproduce(root,Path(extra[0]))
    elif mode in ("bootstrap","trusted"):publish_or_verify(root,mode)
    elif mode=="consumer":consumer(root)
    else:raise ValueError("unknown RC mode")
