#!/usr/bin/env bash
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
cd "$root"
out="${1:?evidence directory required}"
mkdir -p "$out"
out="$(realpath "$out")"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
python scripts/verify_v040_version_scope.py > "$out/version-scope.json"
bash crates/rawmocs-cli/tools/package_source.sh "$out/cli-source"
export RAWMOCS_TEST_CLI="$root/crates/rawmocs-cli/target/release/rawmocs"
cargo build --locked --release --manifest-path crates/rawmocs-cli/Cargo.toml
"$RAWMOCS_TEST_CLI" --version | tee "$out/cli-version.txt"
grep -Fx 'rawmocs 0.4.0' "$out/cli-version.txt"
python -m pip install 'maturin==1.8.3'
maturin build --locked --release --interpreter python --compatibility linux --out "$tmp/wheels"
python -m pip install --force-reinstall --no-deps "$tmp/wheels/"*.whl
python -c 'import rawmocs; assert rawmocs.__version__=="0.4.0"; print(rawmocs.__version__)' | tee "$out/python-version.txt"
python -m unittest discover -s python/tests -v 2>&1 | tee "$out/python-contract.log"
maturin sdist --out "$tmp/sdist"
cp "$tmp/sdist/rawmocs-0.4.0.tar.gz" "$out/"
cargo metadata --locked --manifest-path crates/rawmocs-python/Cargo.toml --format-version 1 > "$out/cargo-metadata.json"
python scripts/python_source_inventory.py --sdist "$out/rawmocs-0.4.0.tar.gz" --metadata "$out/cargo-metadata.json" --out "$out/source-inventory.json"
python scripts/python_package_candidate.py --wheel "$tmp/wheels/"*.whl --platform linux_x86_64 --sdist "$out/rawmocs-0.4.0.tar.gz" --out "$out/package-candidate.json"
mkdir "$tmp/rebuild"
tar -xzf "$out/rawmocs-0.4.0.tar.gz" -C "$tmp/rebuild"
(
 cd "$tmp/rebuild/rawmocs-0.4.0"
 maturin build --locked --release --interpreter python --compatibility linux --out "$tmp/rebuilt-wheels"
 python -m pip install --force-reinstall --no-deps "$tmp/rebuilt-wheels/"*.whl
 python -c 'import rawmocs; assert rawmocs.__version__=="0.4.0"; r=rawmocs.solve("p cnf 1 1\\nc t wmc\\n1 0\\n".replace("\\\\n","\\n")); assert r.sat and r.as_decimal()==(1,0)'
 cd "$root"
 python -m unittest discover -s python/tests -v
) 2>&1 | tee "$out/clean-source-contract.log"
python scripts/python_package_candidate.py --wheel "$tmp/rebuilt-wheels/"*.whl --platform linux_x86_64 --out "$out/rebuilt-package-candidate.json"
(cd "$out"; sha256sum rawmocs-0.4.0.tar.gz cli-source/rawmocs-0.4.0-source.tar.gz > SHA256SUMS)
echo V040_SOURCE_VERSION_ADMISSION_PASS | tee "$out/status.txt"
