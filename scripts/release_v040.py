"""Bounded, exact-main v0.4.0 source-only admission and one-crate bootstrap."""
import io,json,os,re,subprocess,sys,tarfile,tempfile,time,tomllib,zipfile
from pathlib import Path,PurePosixPath
from urllib.request import Request,urlopen
from urllib.error import HTTPError
from release_source import REPO,api,digest,require,run,verify_assets
from release_python_source import verify_python_assets
V="0.4.0"
BASE="05e650f46d60aed9d4fa5e6147709d12f8c3f6a9"
CRATE="rawmocs-0.4.0.crate"
SOURCE="rawmocs-0.4.0-source.tar.gz"
SDIST="rawmocs-0.4.0.tar.gz"
NAMES={"registry-name":"rawmocs-registry-name","package-consumer":"rawmocs-single-crate-rehearsal","source-version-admission":"rawmocs-v040-source-admission"}
STEPS={"registry-name":"Check exact public registry name (no write)","package-consumer":"Build and verify single publishable package","source-version-admission":"Verify version parity and rebuild CLI and Python source archives"}
DIRS=("crates/rawmocs-core/src","crates/rawmocs-cli/src","crates/rawmocs-python/src","python/rawmocs","python/tests")
def validate_request(r):
    require(set(r)=={"version","revision","qualification","admission_run","artifacts","source_sha256","python_source_sha256","crate_sha256","notes"},"unexpected request fields")
    require(r["version"]==V and r["qualification"]=="v040-single-crate-source-only","wrong qualification")
    require(re.fullmatch("[0-9a-f]{40}",r["revision"]) is not None,"full revision required")
    require(type(r["admission_run"]) is int and r["admission_run"]>0,"bad run")
    require(set(r["artifacts"])==set(NAMES),"missing artifacts")
    for a in r["artifacts"].values():
        require(set(a)=={"id","digest"} and type(a["id"]) is int and a["id"]>0 and re.fullmatch("sha256:[0-9a-f]{64}",a["digest"]) is not None,"bad artifact")
    for k in ("source_sha256","python_source_sha256","crate_sha256"):
        require(re.fullmatch("[0-9a-f]{64}",r[k]) is not None,"bad "+k)
    require(r["notes"]=="releases/v0.4.0.md" and Path(r["notes"]).is_file() and not Path(r["notes"]).is_symlink(),"missing notes")
def verify_run(r,d,jobs):
    require(d["id"]==r["admission_run"] and d["head_sha"]==r["revision"] and d["head_branch"]=="main" and d["repository"]["full_name"]==d["head_repository"]["full_name"]==REPO,"wrong admission identity")
    require(d["path"].split("@")[0]==".github/workflows/rust-crate-rehearsal.yml","wrong workflow")
    require(d["event"]=="workflow_dispatch" or (d["event"]=="push" and "[release-v040]" in (d.get("head_commit") or {}).get("message","")),"no release intent")
    require(d["status"]=="completed" and d["conclusion"]=="success","failed run")
    require(jobs["total_count"]==len(jobs["jobs"])==len(NAMES) and {j["name"] for j in jobs["jobs"]}==set(NAMES),"job set mismatch")
    for j in jobs["jobs"]:
        require(j["status"]=="completed" and j["conclusion"]=="success","failed job")
        steps={s["name"]:s["conclusion"] for s in j["steps"]}
        require(steps.get(STEPS[j["name"]])=="success" and not any(v in ("failure","cancelled","timed_out") for v in steps.values()),"missing or failed step")
        if j["name"]=="source-version-admission":require(steps.get("Test v0.4.0 release admission rejection controls")=="success","missing negative controls")
def artifact(r,key):
    a=r["artifacts"][key];endpoint=f"repos/{REPO}/actions/artifacts/{a['id']}"
    info=api(endpoint)
    require(info["id"]==a["id"] and info["name"]==NAMES[key] and info["digest"]==a["digest"] and not info["expired"] and info["workflow_run"]["id"]==r["admission_run"] and info["workflow_run"]["head_sha"]==r["revision"],"artifact identity")
    b=run("gh","api",endpoint+"/zip")
    require("sha256:"+digest(b)==a["digest"],"artifact digest")
    with zipfile.ZipFile(io.BytesIO(b)) as z:
        require(z.testzip() is None and len(z.namelist())==len(set(z.namelist())),"invalid ZIP")
        return {n:z.read(n) for n in z.namelist() if not n.endswith("/")}
def files(blob,prefix):
    with tarfile.open(fileobj=io.BytesIO(blob),mode="r:gz") as t:
        out={};seen=set()
        for m in t.getmembers():
            p=PurePosixPath(m.name)
            require(not p.is_absolute() and ".." not in p.parts and "\\" not in m.name and p.parts[0]==prefix and m.name not in seen and (m.isfile() or m.isdir()),"unsafe archive")
            seen.add(m.name)
            if m.isfile():out["/".join(p.parts[1:])]=t.extractfile(m).read()
        return out
def verify_crate(r,pkg,src):
    require(pkg["status.txt"].strip()==b"SINGLE_RAWMOCS_CRATE_PASS" and digest(pkg[CRATE])==r["crate_sha256"],"crate admission")
    require(pkg["SHA256SUMS"].decode().split()==[r["crate_sha256"],"./"+CRATE],"crate checksum record")
    f=files(pkg[CRATE],"rawmocs-"+V)
    mapping={"src/lib.rs":"crates/rawmocs-cli/src/lib.rs","src/reference_core.rs":"crates/rawmocs-core/src/lib.rs","src/general.rs":"crates/rawmocs-cli/src/general.rs","src/factor_prejoin.rs":"crates/rawmocs-cli/src/factor_prejoin.rs","src/main.rs":"crates/rawmocs-cli/src/main.rs","tests/shared_core.rs":"crates/rawmocs-cli/tests/shared_core.rs","tests/native_stdio.rs":"crates/rawmocs-cli/tests/native_stdio.rs","LICENSE":"LICENSE","README.md":"packaging/rawmocs/README.md","Cargo.toml.orig":"packaging/rawmocs/Cargo.toml"}
    require(set(f)==set(mapping)|{"Cargo.toml","Cargo.lock"},"unexpected crate members")
    require(f["Cargo.toml"]==pkg["normalized-Cargo.toml"] and f["Cargo.lock"]==pkg["Cargo.lock"],"manifest/lock mismatch")
    m=tomllib.loads(f["Cargo.toml"].decode())
    require(m["package"]["name"]=="rawmocs" and m["package"]["version"]==V and m["lib"]["name"]=="rawmocs" and m["bin"][0]["name"]=="rawmocs","wrong package")
    require(tomllib.loads(f["Cargo.toml.orig"].decode())["package"]["publish"]==["crates-io"],"not publishable")
    for field in ("dependencies","dev-dependencies","build-dependencies"):
        for n,d in m.get(field,{}).items():
            require(n not in ("rawmocs-core","rawmocs-cli","rawmocs-python") and (not isinstance(d,dict) or not ({"path","git"}&d.keys())),"non-registry dependency")
    if "packaging/rawmocs/Cargo.lock" in src:
        require(f["Cargo.lock"]==src["packaging/rawmocs/Cargo.lock"],"committed crate lock mismatch")
    receipt=json.loads(pkg["source-map.json"])
    require(receipt["revision"]==r["revision"] and receipt["version"]==V and len(receipt["sources"])==10,"source map identity")
    for dst,path in mapping.items():
        raw=src[path];data=raw
        if dst=="src/lib.rs":data=raw.replace(b"mod general;",b"mod general;\nmod reference_core;").replace(b"rawmocs_core::",b"crate::reference_core::")
        elif dst=="src/main.rs" or dst.startswith("tests/"):data=raw.replace(b"rawmocs_mpfr::",b"rawmocs::")
        require(f[dst]==data and receipt["sources"]["Cargo.toml" if dst=="Cargo.toml.orig" else dst]=={"source":path,"source_sha256":digest(raw),"staged_sha256":digest(data)},"source mismatch "+dst)
    require(b"EXTRACTED_RUST_CONSUMER_PASS" in pkg["consumer.log"] and b"NATIVE_STDIO_ISOLATION_PASS" in pkg["native-tests.log"] and b"rawmocs 0.4.0" in pkg["cli-version.log"],"missing consumer evidence")
    return f
def prepare(r):
    validate_request(r);require(os.environ.get("GITHUB_REPOSITORY")==REPO,"wrong repository")
    main=api(f"repos/{REPO}/git/ref/heads/main")["object"]["sha"]
    subprocess.run(["git","merge-base","--is-ancestor",r["revision"],main],check=True)
    for d in DIRS:require(run("git","rev-parse",BASE+":"+d)==run("git","rev-parse",r["revision"]+":"+d),"solver source changed "+d)
    endpoint=f"repos/{REPO}/actions/runs/{r['admission_run']}"
    d=api(endpoint);verify_run(r,d,api(f"{endpoint}/attempts/{d['run_attempt']}/jobs?per_page=100"))
    n=artifact(r,"registry-name");p=artifact(r,"package-consumer");s=artifact(r,"source-version-admission")
    require(json.loads(n["registry-name.json"])=={"name":"rawmocs","api_http":404,"index_http":404,"transport_ok":True,"status":"AVAILABLE_OBSERVED_NOT_RESERVED"},"name preflight")
    require(s["status.txt"].strip()==b"V040_SOURCE_VERSION_ADMISSION_PASS","missing source PASS")
    scope=json.loads(s["version-scope.json"])
    require(scope["revision"]==r["revision"] and scope["version"]==V and scope["baseline"]==BASE and scope["status"]=="V040_RELEASE_VERSION_SCOPE_PASS","scope receipt")
    base={k:s["cli-source/"+k] for k in (SOURCE,"SHA256SUMS","source-package.json")}
    verify_assets(r,base);src=files(base[SOURCE],"rawmocs-"+V)
    assets=base|{k:s[k] for k in (SDIST,"source-inventory.json","package-candidate.json","rebuilt-package-candidate.json")}
    verify_python_assets(r,assets);verify_crate(r,p,src)
    # Verify every canonical source and lock, not only the mapped source digests.
    for k in src:
        if any(k.startswith(d+"/") for d in DIRS) or k.startswith("crates/rawmocs-cli/tests/") or k in ("LICENSE","packaging/rawmocs/README.md","packaging/rawmocs/Cargo.lock"):
            require(src[k]==run("git","show",r["revision"]+":"+k),"immutable canonical source mismatch "+k)
    for k in ("crates/rawmocs-core/Cargo.toml","crates/rawmocs-cli/Cargo.toml","crates/rawmocs-python/Cargo.toml","crates/rawmocs-cli/Cargo.lock","crates/rawmocs-python/Cargo.lock","pyproject.toml","packaging/rawmocs/Cargo.toml"):
        require(src[k]==run("git","show",r["revision"]+":"+k),"immutable source mismatch")
        if k.startswith("crates/") and k.endswith(".toml"):require(tomllib.loads(src[k].decode())["package"]["publish"] is False,"internal crate enabled")
    assets[CRATE]=p[CRATE];assets["crate-SHA256SUMS"]=(r["crate_sha256"]+"  "+CRATE+"\n").encode();assets["crate-source-map.json"]=p["source-map.json"]
    print("V040_RELEASE_ADMISSION_PASS",r["revision"],flush=True)
    return assets
def get(url):
    try:
        with urlopen(Request(url,headers={"User-Agent":"RaWMoCS release (https://github.com/PhysicsQoo/RaWMoCS)"}),timeout=20) as r:return r.status,r.read()
    except HTTPError as e:
        if e.code==404:return 404,b""
        raise
def registry_state(r):
    a,_=get("https://crates.io/api/v1/crates/rawmocs");i,index=get("https://index.crates.io/ra/wm/rawmocs")
    if a==i==404:return "ABSENT"
    require(a==i==200,"registry state inconclusive")
    rows=[json.loads(x) for x in index.splitlines() if x];vs=[x for x in rows if x["vers"]==V]
    require(len(vs)==1 and vs[0]["cksum"]==r["crate_sha256"] and vs[0]["yanked"] is False,"registry version conflict")
    code,b=get("https://static.crates.io/crates/rawmocs/"+CRATE)
    require(code==200 and digest(b)==r["crate_sha256"],"registry download mismatch")
    return "MATCH"
def registry(mode):
    from release_source import load_request
    require(mode in ("bootstrap","trusted") and os.environ.get("GITHUB_REF")=="refs/heads/main" and os.environ.get("GITHUB_EVENT_NAME")=="workflow_dispatch","explicit main dispatch only")
    r=load_request();require(r["version"]==V,"wrong release")
    assets=prepare(r);require(os.environ.get("CARGO_REGISTRY_TOKEN"),"registry credential unavailable")
    if registry_state(r)=="MATCH":
        print("REGISTRY_ALREADY_MATCHES",r["crate_sha256"]);return
    require(mode=="bootstrap","OIDC cannot bootstrap an absent crate")
    with tempfile.TemporaryDirectory() as tmp:
        p=Path(tmp);checkout=p/"source";stage=p/"stage"
        subprocess.run(["git","worktree","add","--detach",str(checkout),r["revision"]],check=True)
        try:
            env=os.environ.copy();env.pop("CARGO_REGISTRY_TOKEN",None);env["CARGO_TARGET_DIR"]=str(p/"target")
            subprocess.run([sys.executable,"scripts/stage_single_rust_crate.py",str(checkout),str(stage)],env=env,check=True)
            (stage/"Cargo.lock").write_bytes(files(assets[CRATE],"rawmocs-"+V)["Cargo.lock"])
            subprocess.run(["cargo","package","--locked","--no-verify","--manifest-path",str(stage/"Cargo.toml")],env=env,check=True)
            require((p/"target/package"/CRATE).read_bytes()==assets[CRATE],"repackaged bytes differ; refuse upload")
            require(registry_state(r)=="ABSENT","registry name no longer available")
            env["CARGO_REGISTRY_TOKEN"]=os.environ["CARGO_REGISTRY_TOKEN"]
            subprocess.run(["cargo","publish","--locked","--no-verify","--registry","crates-io","--manifest-path",str(stage/"Cargo.toml")],env=env,check=True)
            for n in range(20):
                try:
                    if registry_state(r)=="MATCH":print("REGISTRY_PUBLICATION_VERIFIED",r["crate_sha256"]);return
                except (ValueError,HTTPError):pass
                if n<19:time.sleep(3)
            raise ValueError("upload may have succeeded; inspect registry before retry")
        finally:subprocess.run(["git","worktree","remove","--force",str(checkout)],check=True)
if __name__=="__main__":
    require(len(sys.argv)==2,"registry mode required");registry(sys.argv[1])
