"""Verify the same-revision WASM candidate and form deterministic release assets."""
import io,json,re,stat,tomllib,zipfile
from pathlib import PurePosixPath
from release_source import digest,require
from release_python_source import archive_files
from wasm.license_inventory import is_notice

NAME="rawmocs-wasm-candidate"
BINARY={"rawmocs.js","rawmocs.wasm","worker.js"}
STEPS={"rustc","emcc","node","stage","native-build","locked-fetch","vendor","clean-build-1","clean-build-2","real-browser","resource-browser"}

def read_zip(blob):
    with zipfile.ZipFile(io.BytesIO(blob)) as z:
        entries=z.infolist()
        require(len(entries)==len({x.filename for x in entries}),"duplicate ZIP member")
        require(sum(x.file_size for x in entries)<=512*1024*1024,"ZIP size bound")
        for x in entries:
            p=PurePosixPath(x.filename)
            require(p.parts and not p.is_absolute() and ".." not in p.parts and "\\" not in x.filename and not stat.S_ISLNK(x.external_attr>>16),"unsafe ZIP member")
        require(z.testzip() is None,"invalid ZIP CRC")
        return {x.filename:z.read(x) for x in entries if not x.is_dir()}

def zip_bytes(members):
    out=io.BytesIO()
    with zipfile.ZipFile(out,"w",compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
        for name,data in sorted(members.items()):
            info=zipfile.ZipInfo(name,(1980,1,1,0,0,0))
            info.create_system=3;info.external_attr=0o100644<<16
            z.writestr(info,data,compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)
    return out.getvalue()

def validate_pin(w):
    require(set(w)=={"run","id","digest","review_sha256"},"WASM request fields")
    require(all(type(w[k]) is int and w[k]>0 for k in ("run","id")),"WASM artifact/run identity")
    require(re.fullmatch(r"sha256:[0-9a-f]{64}",w["digest"]) and re.fullmatch(r"[0-9a-f]{64}",w["review_sha256"]),"WASM digest format")

def verify_review(r,raw):
    w=r["wasm"];require(digest(raw)==w["review_sha256"],"WASM review digest")
    review=json.loads(raw)
    require(set(review)=={"status","version","revision","candidate_digest","review_reference","scope"},"WASM review fields")
    require(review["status"]=="APPROVED_FOR_REDISTRIBUTION" and review["version"]==r["version"] and review["revision"]==r["revision"] and review["candidate_digest"]==w["digest"],"WASM review identity")
    require(review["scope"]=="binary-corresponding-source-licenses" and isinstance(review["review_reference"],str) and review["review_reference"].strip(),"WASM source/license review required")

def verify_receipt(r,p,crate,src,source_map):
    q=json.loads(p["receipt.json"])
    require(q["status"]=="UNIFIED_WASM_CANDIDATE_PASS" and q["revision"]==r["revision"] and q["version"]==r["version"] and q["target"]=="wasm32-unknown-emscripten","WASM receipt identity")
    require(q["steps"]==dict.fromkeys(STEPS,0) and q["reproducible_same_environment"] is True and q["offline_vendor_replay"] is True and q["offline_vendor_builds"]==2 and q["build_differences"]=={},"WASM offline replay or reproducibility missing")
    binary={k[len("candidate/"):]:v for k,v in p.items() if k.startswith("candidate/")}
    require(set(binary)==BINARY and q["artifacts"]=={n:digest(v) for n,v in binary.items()} and binary["rawmocs.wasm"][:4]==b"\x00asm","WASM binary identity")
    expected={n:q["artifacts"][n] for n in ("rawmocs.js","rawmocs.wasm")}
    require(q["builds"]==[expected,expected] and binary["worker.js"]==src["scripts/wasm/worker.js"],"WASM build/worker mismatch")
    source={k[len("source/"):]:v for k,v in p.items() if k.startswith("source/")}
    require(q["source_sha256"]=={n:digest(v) for n,v in source.items()},"WASM corresponding source digest")
    executable=json.loads(source["executable-files.json"])
    require(isinstance(executable,list) and len(executable)==len(set(executable)) and set(executable)<=set(source),"WASM executable source inventory")
    require(json.loads(source["source-map.json"])==json.loads(source_map),"WASM/native source map mismatch")
    staged=archive_files(crate,r["version"])
    for n in ("Cargo.toml","Cargo.lock"):
        original=staged["Cargo.toml.orig" if n=="Cargo.toml" else n]
        require(source["source."+n]==original and source["rawmocs/"+n]==source["build."+n],"WASM manifest/lock overlay")
    overlay=b'\n[dependencies.gmp-mpfr-sys]\nversion = "=1.7.1"\ndefault-features = false\nfeatures = ["mpfr", "force-cross"]\n'
    require(source["build.Cargo.toml"]==source["source.Cargo.toml"]+overlay,"unexpected WASM manifest overlay")
    lock=tomllib.loads(source["source.Cargo.lock"].decode())
    root=next(x for x in lock["package"] if x["name"]=="rawmocs")
    root["dependencies"]=sorted(root["dependencies"]+["gmp-mpfr-sys"])
    require(tomllib.loads(source["build.Cargo.lock"].decode())==lock,"unexpected WASM lock overlay")
    native={n:v for n,v in staged.items() if n.startswith(("src/","tests/")) or n in ("LICENSE","README.md")}
    require({n[len("rawmocs/"):]:v for n,v in source.items() if n.startswith("rawmocs/") and n not in ("rawmocs/Cargo.toml","rawmocs/Cargo.lock")}==native,"WASM staged source differs from crate")
    require(q["source_hashes"]=={n:digest(v) for n,v in native.items() if n.startswith("src/") and n.endswith(".rs")},"native source hashes")
    recipe={n[len("scripts/wasm/"):]:v for n,v in src.items() if n.startswith("scripts/wasm/")}
    recipe["stage_single_rust_crate.py"]=src["scripts/stage_single_rust_crate.py"]
    require({n[len("recipe/"):]:v for n,v in source.items() if n.startswith("recipe/")}==recipe,"WASM recipe source mismatch")
    config=json.loads(source["build-config.json"])
    require(config["target"]==q["target"] and config["link_flags"]==q["link_flags"],"WASM build configuration")
    licenses=sorted(n for n in source if is_notice(n))
    require(q["license_inventory"]==licenses and any("gmp-mpfr-sys" in n for n in licenses),"WASM license inventory")
    verify_toolchain(source)
    vendored={}
    for n,v in source.items():
        if n.startswith("vendor/") and n.endswith("/.cargo-checksum.json"):
            folder=n.rsplit("/",1)[0];meta=tomllib.loads(source[folder+"/Cargo.toml"].decode())["package"]
            key=(meta["name"],meta["version"]);require(key not in vendored,"duplicate vendor dependency")
            checksum=json.loads(v);vendored[key]=checksum["package"]
            require(all(digest(source[folder+"/"+path])==sha for path,sha in checksum["files"].items()),"vendor file checksum mismatch")
    require(vendored=={(x["name"],x["version"]):x["checksum"] for x in lock["package"] if "checksum" in x},"incomplete vendor lock inventory")
    require(b"WASM_BROWSER_WORKER_PASS cases=32 failures=2 stop=1 reentry=1" in p["real-browser.log"] and b"WASM_RESOURCE_SCOPED_PASS" in p["resource-browser.log"] and all(b"WASM_OFFLINE_VENDOR_REPLAY_PASS" in p[f"clean-build-{i}.log"] for i in (1,2)),"missing WASM browser/replay evidence")
    rows=json.loads(p["browser-results.json"])["rows"]
    expected_cases={(name,d,m) for name in ("signed","sat_zero","unsat","tiny_nonzero") for d in (30,60,100,130) for m in ("auto","general")}
    require(len(rows)==36 and {(x["case"],x["digits"],x["mode"]) for x in rows if "digits" in x}==expected_cases and all(x["result"]["type"]=="count" for x in rows if "digits" in x),"WASM parity matrix")
    return binary,source,q

def verify_toolchain(source):
    info=json.loads(source["toolchain/inventory.json"])
    require(re.fullmatch(r"[0-9a-f]{40}",info["rust_commit"]) and "release: 1.100.1" in info["rust_version"] and "4.0.23" in info["emscripten_version"],"toolchain notice identity")
    expected={n[len("toolchain/"):]:digest(v) for n,v in source.items()
              if n.startswith("toolchain/") and n not in ("toolchain/inventory.json","toolchain/link-map.txt")}
    require(info["files"]==expected and all(is_notice(n) for n in expected),"toolchain notice inventory")
    rust=[n for n in expected if n.startswith("rust/")]
    require(any(PurePosixPath(n).name=="LICENSE-MIT" for n in rust) and
            any(PurePosixPath(n).name=="LICENSE-APACHE" for n in rust) and
            any(PurePosixPath(n).name.upper().startswith("COPYRIGHT") for n in rust) and
            "emscripten/LICENSE" in expected and "emscripten/system/lib/libc/musl/COPYRIGHT" in expected,
            "required runtime notices")
    require(bool(source["toolchain/link-map.txt"].strip()),"link map missing")

def distribution_payload(binary,source,version):
    require(set(binary)==BINARY,"runtime binary contract")
    verify_toolchain(source)
    files={"licenses/"+n:v for n,v in source.items() if is_notice(n)}
    require("licenses/rawmocs/LICENSE" in files and any("gmp-mpfr-sys" in n for n in files),"missing library notices")
    files["licenses/toolchain/inventory.json"]=source["toolchain/inventory.json"]
    files["REBUILD.md"]=source["recipe/REBUILD.md"]
    names="\n".join(sorted(files))
    files["THIRD-PARTY-NOTICES.txt"]=(
        f"RaWMoCS {version} — MIT licensed application\n"
        "This combined work uses Rug, gmp-mpfr-sys, GMP and MPFR under their applicable licenses, including the GNU LGPL. "
        "See the included notices and GPL/LGPL license texts. Modification of these libraries and reverse engineering to debug modifications are permitted under the applicable licenses.\n"
        "Rust and Emscripten runtime notices are included; see the toolchain inventory and corresponding source link map for review.\n"
        f"Same-version corresponding source: https://github.com/PhysicsQoo/RaWMoCS/releases/tag/v{version}\n"
        "See REBUILD.md for offline modification and relinking instructions.\n\n"
        "Included documents:\n"+names+"\n").encode()
    return {**binary,**files}

def verify_distribution(m,files):
    runtime={n:files[n] for n in BINARY if n in files}
    notices={n:v for n,v in files.items() if n not in BINARY}
    require(set(runtime)==BINARY and runtime["rawmocs.wasm"][:4]==b"\x00asm" and
            m["distribution_sha256"]=={n:digest(v) for n,v in runtime.items()},"binary archive contract")
    require({"THIRD-PARTY-NOTICES.txt","REBUILD.md","licenses/rawmocs/LICENSE",
             "licenses/toolchain/inventory.json","licenses/toolchain/emscripten/LICENSE",
             "licenses/toolchain/emscripten/system/lib/libc/musl/COPYRIGHT"}<=set(notices)
            and all(v for v in notices.values()) and
            all(n.startswith("licenses/") or n in ("THIRD-PARTY-NOTICES.txt","REBUILD.md") for n in notices)
            and m["license_sha256"]=={n:digest(v) for n,v in notices.items()},"distribution notice contract")
    require(f"https://github.com/PhysicsQoo/RaWMoCS/releases/tag/v{m['version']}".encode() in notices["THIRD-PARTY-NOTICES.txt"],"notice source version")

def release_assets(r,p,crate,src,source_map,review):
    verify_review(r,review)
    binary,source,q=verify_receipt(r,p,crate,src,source_map)
    v=r["version"];dn=f"rawmocs-{v}-wasm.zip";sn=f"rawmocs-{v}-wasm-source.zip"
    source["admission-receipt.json"]=p["receipt.json"]
    distribution=distribution_payload(binary,source,v)
    assets={dn:zip_bytes(distribution),sn:zip_bytes(source),"wasm-review.json":review}
    manifest={"solver":"RaWMoCS","version":v,"revision":r["revision"],"status":"released",
              "distribution_sha256":q["artifacts"],"license_sha256":{n:digest(data) for n,data in distribution.items() if n not in BINARY},"distribution_archive":dn,"distribution_zip_sha256":digest(assets[dn]),
              "source_archive":sn,"source_zip_sha256":digest(assets[sn]),"crate_sha256":digest(crate),
              "candidate_digest":r["wasm"]["digest"],"review_sha256":digest(review),"license_inventory":q["license_inventory"]}
    verify_distribution(manifest,distribution)
    assets["wasm-provenance.json"]=(json.dumps(manifest,sort_keys=True,indent=2)+"\n").encode()
    assets["wasm-SHA256SUMS"]="".join(f"{digest(data)}  {name}\n" for name,data in sorted(assets.items())).encode()
    return assets
