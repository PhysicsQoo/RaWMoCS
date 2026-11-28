#!/usr/bin/env bash
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
out="${1:?evidence directory required}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
stage="$tmp/rawmocs"
python3 "$root/scripts/stage_single_rust_crate.py" "$root" "$stage" > "$out/source-map.json"
version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$out/source-map.json")"
rustc -Vv > "$out/rustc.txt"
cargo -V > "$out/cargo.txt"
export CARGO_TARGET_DIR="$tmp/target"
cp "$root/packaging/rawmocs/Cargo.lock" "$stage/Cargo.lock"
cp "$stage/Cargo.lock" "$out/Cargo.lock"
cargo package --locked --manifest-path "$stage/Cargo.toml" 2>&1 | tee "$out/package.log"
cp "$CARGO_TARGET_DIR/package/rawmocs-$version.crate" "$out/"
mkdir "$tmp/extracted"
tar -xzf "$out/rawmocs-$version.crate" -C "$tmp/extracted"
pkg="$tmp/extracted/rawmocs-$version"
python3 - "$pkg" "$out/source-map.json" <<'PY'
from pathlib import Path
import hashlib,json,sys,tomllib
p=Path(sys.argv[1]); receipt=json.load(open(sys.argv[2]))
m=tomllib.loads((p/"Cargo.toml").read_text())
assert m["package"]["name"]=="rawmocs"
assert m["package"]["version"]==receipt["version"]
assert m["lib"]["name"]=="rawmocs"
assert "rawmocs-core" not in m["dependencies"] and "rawmocs-cli" not in m["dependencies"]
for d in m["dependencies"].values():
    assert not isinstance(d,dict) or not ({"path","git"} & d.keys())
for name,v in receipt["sources"].items():
    if name=="Cargo.toml":continue # Cargo normalizes manifest during package
    assert hashlib.sha256((p/name).read_bytes()).hexdigest()==v["staged_sha256"],name
assert (p/"LICENSE").is_file()
PY
cp "$pkg/Cargo.toml" "$out/normalized-Cargo.toml"
cargo test --locked --release --manifest-path "$pkg/Cargo.toml" --test shared_core --test native_stdio 2>&1 | tee "$out/native-tests.log"
# Independent consumer of the archive, not of the canonical source checkout.
mkdir -p "$tmp/consumer/src"
cat > "$tmp/consumer/Cargo.toml" <<EOF
[package]
name = "rawmocs-registry-consumer"
version = "0.0.0"
edition = "2021"
publish = false
[workspace]
[dependencies]
rawmocs = "=$version"
[patch.crates-io]
rawmocs = { path = "$pkg" }
EOF
sed 's/rawmocs_mpfr::/rawmocs::/g' "$root/crates/rawmocs-cli/tools/external_consumer.rs" > "$tmp/consumer/src/main.rs"
printf 'p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n' > "$tmp/sat-zero.cnf"
cargo generate-lockfile --manifest-path "$tmp/consumer/Cargo.toml"
cp "$tmp/consumer/Cargo.lock" "$out/consumer.Cargo.lock"
cargo run --locked --release --manifest-path "$tmp/consumer/Cargo.toml" -- "$tmp/sat-zero.cnf" 2>&1 | tee "$out/consumer.log"
grep -Fx EXTRACTED_RUST_CONSUMER_PASS "$out/consumer.log"
cargo run --locked --release --manifest-path "$pkg/Cargo.toml" --bin rawmocs -- --version 2>&1 | tee "$out/cli-version.log"
grep -Fx "rawmocs $version" "$out/cli-version.log"
(cd "$out"; sha256sum ./*.crate > SHA256SUMS)
echo SINGLE_RAWMOCS_CRATE_PASS > "$out/status.txt"
# Verify the exact release-side crate parser against the canonical checkout.
if ! python3 "$root/scripts/replay_v040_assets.py" crate "$out"; then
  rm -f "$out/status.txt"
  exit 1
fi
cat "$out/status.txt"
