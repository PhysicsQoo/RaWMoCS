"""Immutable stable source admission and OIDC publication. No RC bootstrap."""
import io,json,os,re,subprocess,sys,tempfile,time,tomllib,zipfile
from pathlib import Path,PurePosixPath
from urllib.error import HTTPError,URLError
from release_source import REPO,api,digest,require,run,verify_assets,verify_release,releases,tag_commit,workflow_changes
from release_python_source import verify_python_assets, archive_files
from urllib.request import Request, urlopen
from stable_crate_assets import verify_crate
from wasm_release import NAME as WASM_NAME, validate_pin, read_zip, release_assets as wasm_assets

REQUEST=Path("releases/stable-request.json")
BASE_VERSION="0.4.0"
BASE_SHA="d3e9e6ccd4a146bac19f6bd2628038cf20ae331dd12f9dc0cbcd37a4e63e955d"
NAMES={"crate":"rawmocs-stable-crate-admission","source":"rawmocs-stable-source-admission"}
STEPS={"crate":{"Verify version and source scope","Stage and verify exact single stable crate"},
       "source":{"Verify stable CLI and Python extracted sources"},
       "wasm":{"Stage canonical Rust package and build both targets"},
       "assets":{"Replay exact unified publisher asset contract"}}
def validate_request(r):
    fields={"version","revision","admission_run","artifacts","notes"}
    require(set(r)==fields|{"wasm"},"unified release request fields")
    validate_pin(r["wasm"])
    require(r["wasm"]["run"]==r["admission_run"],"WASM must share native package admission run")
    v=r["version"]
    require(re.fullmatch(r"\d+\.\d+\.\d+",v) and tuple(map(int,v.split(".")))>(0,4,0),"stable version after bootstrap required")
    require(re.fullmatch(r"[0-9a-f]{40}",r["revision"]) is not None,"full source revision")
    require(type(r["admission_run"]) is int and r["admission_run"]>0,"admission run")
    require(set(r["artifacts"])==set(NAMES),"artifact set")
    for a in r["artifacts"].values():
        require(set(a)=={"id","digest"} and type(a["id"]) is int and a["id"]>0 and re.fullmatch(r"sha256:[0-9a-f]{64}",a["digest"]),"artifact identity")
    require(r["notes"]==f"releases/v{v}.md","notes path")
def request():
    r=json.loads(REQUEST.read_text());validate_request(r)
    p=Path(r["notes"]);require(p.is_file() and not p.is_symlink() and p.stat().st_size>0,"notes missing")
    return r
def head():
    require(os.environ.get("GITHUB_REPOSITORY")==REPO and os.environ.get("GITHUB_REF")=="refs/heads/main","main only")
    h=run("git","rev-parse","HEAD").decode().strip()
    require(api(f"repos/{REPO}/git/ref/heads/main")["object"]["sha"]==h,"main advanced")
    return h
def intent(event_name,event,sha):
    if event_name=="workflow_dispatch":
        require((event.get("inputs") or {}).get("registry_mode")=="stable-trusted","explicit trusted dispatch")
    elif event_name=="push":
        h=event.get("head_commit") or {}
        require(event.get("ref")=="refs/heads/main" and event.get("after")==sha and h.get("id")==sha and "[publish-stable]" in (h.get("message") or ""),"explicit stable push intent")
    else:raise ValueError("untrusted event")
def authorize():
    h=head();intent(os.environ["GITHUB_EVENT_NAME"],json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text()),h);return h
def verify_run(r,d,jobs):
    require(d["id"]==r["admission_run"] and d["head_sha"]==r["revision"] and d["repository"]["full_name"]==d["head_repository"]["full_name"]==REPO,"admission identity")
    require(d["path"].split("@")[0]==".github/workflows/stable-package-admission.yml" and d["head_branch"] in ("main","release/v"+r["version"]) and d["event"] in ("push","workflow_dispatch"),"admission origin")
    require(d["status"]=="completed" and d["conclusion"]=="success","admission not successful")
    require(jobs["total_count"]==len(jobs["jobs"])==len(STEPS) and {j["name"] for j in jobs["jobs"]}==set(STEPS),"job set")
    for j in jobs["jobs"]:
        steps={s["name"]:s["conclusion"] for s in j["steps"]}
        require(j["status"]=="completed" and j["conclusion"]=="success" and all(steps.get(n)=="success" for n in STEPS[j["name"]]),"missing successful admission step")
        require(not any(x in ("failure","cancelled","timed_out") for x in steps.values()),"failed step")
def artifact(r,key):
    a=r["wasm"] if key=="wasm" else r["artifacts"][key]
    name=WASM_NAME if key=="wasm" else NAMES[key]
    ep=f"repos/{REPO}/actions/artifacts/{a['id']}"
    info=api(ep)
    require(info["id"]==a["id"] and info["name"]==name and info["digest"]==a["digest"] and not info["expired"] and info["workflow_run"]["id"]==r["admission_run"] and info["workflow_run"]["head_sha"]==r["revision"],"artifact metadata")
    b=run("gh","api",ep+"/zip");require("sha256:"+digest(b)==a["digest"],"ZIP digest")
    return read_zip(b)
def prepare(r):
    h=head()
    subprocess.run(["git","merge-base","--is-ancestor",r["revision"],h],check=True)
    changed=run("git","diff","--name-only","--no-renames",r["revision"],h).decode().splitlines()
    allowed={str(REQUEST)}
    if "wasm" in r:allowed.add(f"releases/v{r['version']}-wasm-review.json")
    require(set(changed)<=allowed,"unadmitted changes since package admission")
    require(not workflow_changes(r["revision"],h),"workflow changed since package admission")
    ep=f"repos/{REPO}/actions/runs/{r['admission_run']}"
    d=api(ep);verify_run(r,d,api(f"{ep}/attempts/{d['run_attempt']}/jobs?per_page=100"))
    p=artifact(r,"crate");s=artifact(r,"source");w=artifact(r,"wasm")
    assets,src=verify_packages(r,p,s)
    v=r["version"];crate=f"rawmocs-{v}.crate"
    review=Path(f"releases/v{v}-wasm-review.json")
    require(review.is_file() and not review.is_symlink(),"WASM redistribution review missing")
    assets.update(wasm_assets(r,w,p[crate],src,p["source-map.json"],review.read_bytes()))
    print("STABLE_WASM_ASSET_ADMISSION_PASS",r["revision"],flush=True)
    print("STABLE_PACKAGE_ADMISSION_PASS",r["revision"],r["crate_sha256"],flush=True)
    return assets

def verify_packages(r,p,s):
    v=r["version"]
    crate=f"rawmocs-{v}.crate";source=f"rawmocs-{v}-source.tar.gz";sdist=f"rawmocs-{v}.tar.gz"
    require(s["status.txt"].strip()==b"STABLE_SOURCE_VERSION_ADMISSION_PASS","source admission")
    scope=json.loads(s["version-scope.json"])
    require(scope["status"]=="STABLE_RELEASE_VERSION_SCOPE_PASS" and scope["revision"]==r["revision"] and scope["version"]==v and scope["publication_authorized"] is False,"version scope receipt")
    for path in ("crates/rawmocs-core/src","crates/rawmocs-cli/src","crates/rawmocs-python/src","python/rawmocs","python/tests"):
        require(scope["source_trees"][path]==run("git","rev-parse",r["revision"]+":"+path).decode().strip(),"source tree receipt mismatch")
    r.update(crate_sha256=digest(p[crate]),source_sha256=digest(s["cli-source/"+source]),python_source_sha256=digest(s[sdist]))
    assets={k:s["cli-source/"+k] for k in (source,"SHA256SUMS","source-package.json")}
    verify_assets(r,assets)
    src=archive_files(assets[source],v)
    # Require exact tracked source archive identity for all files (including locks and workflows).
    tree=run("git","ls-tree","-r","--name-only",r["revision"]).decode().splitlines()
    # git archive may omit paths marked export-ignore. Reproduce the archive and compare bytes.
    import gzip
    tar=run("git","archive","--format=tar","--prefix=rawmocs-"+v+"/",r["revision"])
    require(gzip.decompress(assets[source])==tar,"source archive differs from Git")
    require(set(src)<=set(tree),"source archive contains untracked members")
    assets.update({k:s[k] for k in (sdist,"source-inventory.json","package-candidate.json","rebuilt-package-candidate.json")})
    verify_python_assets(r,assets)
    verify_crate(r,p,src)
    require(b"STABLE_PUBLISHER" not in s["status.txt"],"unexpected status")
    assets[crate]=p[crate];assets["crate-SHA256SUMS"]=(r["crate_sha256"]+"  "+crate+"\n").encode();assets["crate-source-map.json"]=p["source-map.json"]
    return assets,src

def index_state(rows,v,sha):
    require(len({x["vers"] for x in rows})==len(rows),"duplicate registry versions")
    base=[x for x in rows if x["vers"]==BASE_VERSION]
    require(len(base)==1 and base[0]["cksum"]==BASE_SHA and base[0]["yanked"] is False,"stable registry bootstrap identity")
    targets=[x for x in rows if x["vers"]==v]
    if not targets:return "READY"
    require(len(targets)==1 and targets[0]["cksum"]==sha and targets[0]["yanked"] is False,"registry version conflict")
    return "MATCH"
def get(url):
    try:
        with urlopen(Request(url,headers={"User-Agent":"RaWMoCS release (https://github.com/PhysicsQoo/RaWMoCS)"}),timeout=20) as r:return r.status,r.read()
    except HTTPError as e:
        if e.code==404:return 404,b""
        raise

def registry_state(r):
    code,_=get("https://crates.io/api/v1/crates/rawmocs");c,index=get("https://index.crates.io/ra/wm/rawmocs")
    require(code==c==200,"existing crate registry identity required")
    state=index_state([json.loads(x) for x in index.splitlines() if x],r["version"],r["crate_sha256"])
    if state=="MATCH":
        c,b=get("https://static.crates.io/crates/rawmocs/rawmocs-"+r["version"]+".crate")
        require(c==200 and digest(b)==r["crate_sha256"],"registry download mismatch")
    return state
def checked_state(r):
    for n in range(6):
        try:return registry_state(r)
        except (HTTPError,URLError,TimeoutError):
            if n==5:raise
            time.sleep(3)
def preflight(r,assets):
    state=checked_state(r);tag="v"+r["version"];actual=tag_commit(tag);existing=releases(tag)
    require(len(existing)<=1 and actual in (None,r["revision"]),"conflicting tag/release")
    if state=="READY":
        require(actual is None and not existing,"tag/release exists before registry upload")
    elif existing:
        require(actual==r["revision"],"release tag mismatch")
        if existing[0]["draft"]:verify_exact_draft(existing[0],r,assets)
        else:verify_release(existing[0],r,assets,False)
    return state
def verify_exact_draft(d,r,assets):
    verify_release(d,r,assets,True)
    require(d["author"]["login"]=="github-actions[bot]" and d["name"]=="RaWMoCS v"+r["version"] and d["target_commitish"] in ("main",r["revision"]) and (d.get("body") or "").strip()==Path(r["notes"]).read_text().strip(),"draft identity mismatch")
def registry(r,assets):
    h=authorize()
    if preflight(r,assets)=="MATCH":print("STABLE_REGISTRY_ALREADY_MATCHES",flush=True);return
    require(os.environ.get("CARGO_REGISTRY_TOKEN"),"OIDC credential missing")
    require(run("cargo","--version").decode().split()[1]=="1.100.1","Cargo toolchain mismatch")
    crate=f"rawmocs-{r['version']}.crate"
    with tempfile.TemporaryDirectory() as tmp:
        p=Path(tmp);checkout=p/"source";stage=p/"stage"
        subprocess.run(["git","worktree","add","--detach",str(checkout),r["revision"]],check=True)
        try:
            env=os.environ.copy();env.pop("CARGO_REGISTRY_TOKEN",None);env["CARGO_TARGET_DIR"]=str(p/"target")
            subprocess.run([sys.executable,"scripts/stage_single_rust_crate.py",str(checkout),str(stage)],env=env,check=True)
            (stage/"Cargo.lock").write_bytes(archive_files(assets[crate],r["version"])["Cargo.lock"])
            args=["--locked","--no-verify","--manifest-path",str(stage/"Cargo.toml")]
            subprocess.run(["cargo","package",*args],env=env,check=True)
            require((p/"target/package"/crate).read_bytes()==assets[crate],"repackaged crate bytes differ")
            require(head()==h and preflight(r,assets)=="READY","state changed before upload")
            env["CARGO_REGISTRY_TOKEN"]=os.environ["CARGO_REGISTRY_TOKEN"]
            subprocess.run(["cargo","publish",*args,"--registry","crates-io"],env=env,check=True)
            for n in range(20):
                try:
                    if registry_state(r)=="MATCH":
                        print("STABLE_REGISTRY_PUBLICATION_VERIFIED",r["crate_sha256"],flush=True);return
                except (ValueError,HTTPError,URLError,TimeoutError):pass
                if n<19:time.sleep(3)
            raise ValueError("upload may have succeeded; inspect registry before retry")
        finally:subprocess.run(["git","worktree","remove","--force",str(checkout)],check=True)
def publish(r,assets):
    h=authorize();require(preflight(r,assets)=="MATCH","registry not verified")
    tag="v"+r["version"];existing=releases(tag);actual=tag_commit(tag)
    if existing and not existing[0]["draft"]:
        print("STABLE_RELEASE_ALREADY_PUBLISHED",existing[0]["html_url"]);return
    # Create exact tag only after matching registry bytes. A retry may resume this tag;
    # never move an existing tag. No workflow-tree exception is needed.
    if actual is None:
        require(not existing and head()==h and checked_state(r)=="MATCH" and tag_commit(tag) is None,"tag creation precondition")
        subprocess.run(["gh","api","--method","POST",f"repos/{REPO}/git/refs","-f","ref=refs/tags/"+tag,"-f","sha="+r["revision"]],check=True)
    require(tag_commit(tag)==r["revision"],"exact tag not visible")
    if not existing:
        with tempfile.TemporaryDirectory() as tmp:
            paths=[]
            for n,data in assets.items():
                p=Path(tmp)/n;p.write_bytes(data);paths.append(str(p))
            require(head()==h and not releases(tag),"release state changed")
            subprocess.run(["gh","release","create",tag,*paths,"-R",REPO,"--verify-tag","--draft","--title","RaWMoCS "+tag,"--notes-file",r["notes"]],check=True)
        for n in range(6):
            existing=releases(tag)
            if existing:break
            time.sleep(1)
    require(len(existing)==1,"draft missing/duplicate")
    endpoint=f"repos/{REPO}/releases/{existing[0]['id']}"
    verify_exact_draft(api(endpoint),r,assets)
    require(head()==h and tag_commit(tag)==r["revision"] and checked_state(r)=="MATCH","state changed before publish")
    subprocess.run(["gh","api","--method","PATCH",endpoint,"-F","draft=false"],check=True)
    final=api(endpoint);verify_release(final,r,assets,False)
    require(tag_commit(tag)==r["revision"],"published tag mismatch")
    print("STABLE_RELEASE_PUBLISHED_AND_TAG_VERIFIED",final["html_url"],flush=True)
def main():
    require(len(sys.argv)==2 and sys.argv[1] in ("validate","registry","publish"),"mode")
    r=request();assets=prepare(r)
    if sys.argv[1]=="validate":
        state=preflight(r,assets)
        print("STABLE_REGISTRY_PREFLIGHT",state)
        with open(os.environ["GITHUB_OUTPUT"],"a") as f:print("registry_state="+state,file=f)
    elif sys.argv[1]=="registry":registry(r,assets)
    else:publish(r,assets)
if __name__=="__main__":main()
