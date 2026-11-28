"""Stable single-crate artifact contract; derived from admitted v0.4.0 checks."""
import json,tomllib
from release_source import digest,require
from release_v040 import files
def verify_crate(r,pkg,src):
    V=r['version']; CRATE=f'rawmocs-{V}.crate'
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
    require(b"EXTRACTED_RUST_CONSUMER_PASS" in pkg["consumer.log"] and b"NATIVE_STDIO_ISOLATION_PASS" in pkg["native-tests.log"] and ("rawmocs "+V).encode() in pkg["cli-version.log"],"missing consumer evidence")
    return f
