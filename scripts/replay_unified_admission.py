"""Read-only actual native and WASM artifact replay; no review or publication."""
import subprocess,sys,tomllib
from pathlib import Path
from stable_release import NAMES,verify_packages
from wasm_release import NAME,BINARY,verify_receipt,distribution_payload,verify_distribution,zip_bytes,read_zip
from release_source import digest

root=Path(__file__).resolve().parents[1]
def files(path):
    assert path.is_dir()
    return {p.relative_to(path).as_posix():p.read_bytes() for p in sorted(path.rglob("*")) if p.is_file()}
def main():
    folder=Path(sys.argv[1]).resolve()
    v=tomllib.loads((root/"packaging/rawmocs/Cargo.toml").read_text())["package"]["version"]
    revision=subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip()
    r={"revision":revision,"version":v}
    p=files(folder/NAMES["crate"]);s=files(folder/NAMES["source"]);w=files(folder/NAME)
    assets,src=verify_packages(r,p,s)
    binary,source,receipt=verify_receipt(r,w,p[f"rawmocs-{v}.crate"],src,p["source-map.json"])
    distribution=distribution_payload(binary,source,v)
    verify_distribution({"version":v,"distribution_sha256":receipt["artifacts"],"license_sha256":{n:digest(data) for n,data in distribution.items() if n not in BINARY}},distribution)
    for payload in (distribution,source):
        archive=zip_bytes(payload)
        assert read_zip(archive)==payload and zip_bytes(read_zip(archive))==archive
    print("UNIFIED_PUBLISHER_ACTUAL_ASSET_REPLAY_PASS",revision,v,flush=True)
    print("PUBLICATION_AUTHORIZED=false; redistribution review still required",flush=True)
if __name__=="__main__":main()
