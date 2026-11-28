#!/usr/bin/env bash
# Rehearse registry distribution without publishing, changing release versions,
# or merging the intentionally separate portable/native source workspaces.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
out="${1:?evidence directory required}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/crates"
cp -R "$root/crates/rawmocs-core" "$root/crates/rawmocs-cli" "$stage/crates/"
rm -rf "$stage/crates/"*/target
for p in rawmocs-core rawmocs-cli; do cp "$root/LICENSE" "$stage/crates/$p/LICENSE"; done
python3 - "$stage" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1])
for name,description in (("rawmocs-core","Portable exact weighted model counting core"),
                         ("rawmocs-cli","Native MPFR weighted model counting library and CLI")):
    f=p/"crates"/name/"Cargo.toml"
    s=f.read_text()
    assert s.count("publish = false")==1
    s=s.replace("publish = false",'publish = ["crates-io"]\ndescription = "'+description+'"\nrepository = "https://github.com/PhysicsQoo/RaWMoCS"\ninclude = ["src/**", "tests/**", "examples/**", "LICENSE", "Cargo.toml"]')
    if name=="rawmocs-cli":
        assert s.count("[workspace]")==1
        s=s.replace("[workspace]","")
        old='rawmocs-core = { path = "../rawmocs-core" }'
        assert old in s
        s=s.replace(old,'rawmocs-core = { path = "../rawmocs-core", version = "=0.1.0" }')
    f.write_text(s)
(p/"Cargo.toml").write_text('[workspace]\nmembers = ["crates/rawmocs-core", "crates/rawmocs-cli"]\nresolver = "2"\n')
PY
git -C "$root" rev-parse HEAD > "$out/source-sha.txt"
rustc -Vv > "$out/rustc.txt"
cargo -V > "$out/cargo.txt"
# Cargo's multi-package mode resolves unpublished workspace dependencies for
# packaging. The next step builds an independent consumer of the archives.
cargo package --manifest-path "$stage/Cargo.toml" --workspace --allow-dirty --no-verify 2>&1 | tee "$out/package.log"
cp "$stage/target/package/"*.crate "$out/"
mkdir -p "$stage/extracted"
for f in "$out/"*.crate; do tar -xzf "$f" -C "$stage/extracted"; done
python3 - "$stage" <<'PY'
from pathlib import Path
import sys,tomllib
p=Path(sys.argv[1])/"extracted"
core=p/"rawmocs-core-0.1.0"; cli=p/"rawmocs-cli-0.3.0"
for d in (core,cli):
    assert (d/"LICENSE").is_file() and (d/"src/lib.rs").is_file()
    meta=tomllib.loads((d/"Cargo.toml").read_text())
    assert meta["package"]["publish"]==["crates-io"]
    assert "workspace" not in meta
dep=tomllib.loads((cli/"Cargo.toml").read_text())["dependencies"]["rawmocs-core"]
assert dep["version"]=="=0.1.0" and "path" not in dep and "git" not in dep
PY
mkdir -p "$stage/consumer/src"
cat > "$stage/consumer/Cargo.toml" <<EOF
[package]
name = "rawmocs-packaged-consumer"
version = "0.0.0"
edition = "2021"
publish = false
[workspace]
[dependencies]
rawmocs_mpfr = { package = "rawmocs-cli", version = "=0.3.0" }
[patch.crates-io]
rawmocs-cli = { path = "$stage/extracted/rawmocs-cli-0.3.0" }
rawmocs-core = { path = "$stage/extracted/rawmocs-core-0.1.0" }
EOF
cat > "$stage/consumer/src/main.rs" <<'RS'
use rawmocs_mpfr::{solve_text, SolveOptions};
fn main() {
    let r=solve_text("p cnf 1 1\nc t wmc\nc p weight 1 -2 3 0\n1 0\n",SolveOptions::default()).unwrap();
    assert!(r.sat);
    assert_eq!(r.real.parse::<f64>().unwrap(),-2.);
    assert_eq!(r.imag.parse::<f64>().unwrap(),3.);
    println!("R2_PACKAGED_CONSUMER_PASS");
}
RS
cargo generate-lockfile --manifest-path "$stage/consumer/Cargo.toml"
cp "$stage/consumer/Cargo.lock" "$out/consumer.Cargo.lock"
cp "$stage/extracted/rawmocs-cli-0.3.0/Cargo.toml" "$out/normalized-cli.toml"
cp "$stage/extracted/rawmocs-core-0.1.0/Cargo.toml" "$out/normalized-core.toml"
cargo run --release --locked --manifest-path "$stage/consumer/Cargo.toml" 2>&1 | tee "$out/consumer.log"
grep -Fx R2_PACKAGED_CONSUMER_PASS "$out/consumer.log"
(cd "$out"; sha256sum ./*.crate > SHA256SUMS)
