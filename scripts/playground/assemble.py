"""Stage a standalone site from an exact official release, never Actions artifacts."""
import json,re,subprocess,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from release_source import REPO,api,digest,require,tag_commit
from wasm_release import read_zip,verify_distribution

ROOT=Path(__file__).resolve().parents[2]
def validate_pin(pin):
    require(set(pin)=={"version","revision","release_id","distribution_zip_sha256","provenance_sha256"},"Playground release pin not configured")
    require(re.fullmatch(r"\d+\.\d+\.\d+",pin["version"]) and re.fullmatch(r"[0-9a-f]{40}",pin["revision"]) and type(pin["release_id"]) is int and pin["release_id"]>0,"invalid Playground pin")
    require(all(re.fullmatch(r"[0-9a-f]{64}",pin[k]) for k in ("distribution_zip_sha256","provenance_sha256")),"invalid Playground digest")

def verify_metadata(pin,release,tag):
    validate_pin(pin)
    require(release["id"]==pin["release_id"] and release["tag_name"]=="v"+pin["version"] and not release["draft"] and not release["prerelease"] and release["published_at"] and tag==pin["revision"],"not the pinned official release")
    require(release["author"]["login"]=="github-actions[bot]","unexpected release producer")
    rows=release["assets"];require(len(rows)==len({a["name"] for a in rows}),"duplicate release assets")
    assets={a["name"]:a["digest"] for a in rows if a["state"]=="uploaded"}
    dn=f"rawmocs-{pin['version']}-wasm.zip";sn=f"rawmocs-{pin['version']}-wasm-source.zip"
    require(assets.get(dn)=="sha256:"+pin["distribution_zip_sha256"] and assets.get("wasm-provenance.json")=="sha256:"+pin["provenance_sha256"] and all(n in assets for n in (sn,"wasm-review.json","wasm-SHA256SUMS",f"rawmocs-{pin['version']}.crate")),"release asset metadata")
    return assets

def verify_payload(pin,assets,manifest_raw,blob):
    require(digest(manifest_raw)==pin["provenance_sha256"] and digest(blob)==pin["distribution_zip_sha256"],"download hash mismatch")
    m=json.loads(manifest_raw);v=pin["version"]
    require(m["solver"]=="RaWMoCS" and m["version"]==v and m["revision"]==pin["revision"] and m["status"]=="released" and m["distribution_archive"]==f"rawmocs-{v}-wasm.zip" and m["distribution_zip_sha256"]==digest(blob),"release provenance identity")
    require(m["source_archive"]==f"rawmocs-{v}-wasm-source.zip" and assets[m["source_archive"]]=="sha256:"+m["source_zip_sha256"] and assets["wasm-review.json"]=="sha256:"+m["review_sha256"] and assets[f"rawmocs-{v}.crate"]=="sha256:"+m["crate_sha256"],"source/review provenance mismatch")
    binary=read_zip(blob)
    verify_distribution(m,binary)
    return m,binary

def bind_release_footer(html,version):
    require(re.fullmatch(r"\d+\.\d+\.\d+",version),"footer release version")
    marker="__RAWMOCS_RELEASE_URL__"
    require(html.count(marker)==1 and 'href="THIRD-PARTY-NOTICES.txt"' in html,"license footer missing")
    return html.replace(marker,f"https://github.com/{REPO}/releases/tag/v{version}")

def main():
    work=Path(sys.argv[1]).resolve();pin=json.loads((ROOT/"playground/release.json").read_text());validate_pin(pin)
    tag="v"+pin["version"];release=api(f"repos/{REPO}/releases/{pin['release_id']}")
    assets=verify_metadata(pin,release,tag_commit(tag))
    work.mkdir(parents=True,exist_ok=True)
    downloads=work/"downloads";downloads.mkdir()
    dn=f"rawmocs-{pin['version']}-wasm.zip"
    subprocess.run(["gh","release","download",tag,"-R",REPO,"--pattern",dn,"--pattern","wasm-provenance.json","--dir",str(downloads)],check=True)
    manifest,binary=verify_payload(pin,assets,(downloads/"wasm-provenance.json").read_bytes(),(downloads/dn).read_bytes())
    site=work/"site";site.mkdir()
    for n in ("index.html","style.css","app.js"):(site/n).write_bytes((ROOT/"playground"/n).read_bytes())
    (site/"index.html").write_text(bind_release_footer((site/"index.html").read_text(),pin["version"]))
    for n,data in binary.items():
        out=site/n;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(data)
    manifest["release_id"]=pin["release_id"];manifest["release_url"]=release["html_url"]
    (site/"PROVENANCE.json").write_text(json.dumps(manifest,indent=2)+"\n")
    (work/"receipt.json").write_text(json.dumps({"status":"STANDALONE_OFFICIAL_ARTIFACT_STAGE_PASS","manifest":manifest},indent=2)+"\n")
    print("STANDALONE_OFFICIAL_ARTIFACT_STAGE_PASS",flush=True)
if __name__=="__main__":main()
