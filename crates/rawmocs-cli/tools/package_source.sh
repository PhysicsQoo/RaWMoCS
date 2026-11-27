#!/usr/bin/env bash
# Source-only package; do not publish or redistribute a linked binary here.
set -euo pipefail
export LC_ALL=C
root=$(git rev-parse --show-toplevel)
cd "$root"
version=$(python3 - <<'PY'
import re
import tomllib
with open("crates/rawmocs-cli/Cargo.toml", "rb") as f:
    version = tomllib.load(f)["package"]["version"]
assert re.fullmatch(r"\d+\.\d+\.\d+", version)
print(version)
PY
)
sha=$(git rev-parse HEAD)
test -z "$(git status --porcelain --untracked-files=no)"
test "$(uname -s)" = Linux
test "$(uname -m)" = x86_64
out=${1:?usage: package_source.sh OUTPUT_DIRECTORY}
mkdir -p "$out"
out=$(realpath "$out")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
archive="rawmocs-$version-source.tar.gz"
# Git commit time + gzip without header timestamps; repeat byte-for-byte.
git archive --format=tar --prefix="rawmocs-$version/" "$sha" | gzip -n > "$out/$archive"
git archive --format=tar --prefix="rawmocs-$version/" "$sha" | gzip -n > "$tmp/repeat.tar.gz"
cmp "$out/$archive" "$tmp/repeat.tar.gz"
(cd "$out"; sha256sum "$archive" > SHA256SUMS; sha256sum -c SHA256SUMS)
tar -xzf "$out/$archive" -C "$tmp"
src="$tmp/rawmocs-$version"
test -f "$src/LICENSE"
cmp "$src/crates/rawmocs-cli/Cargo.lock" "$root/crates/rawmocs-cli/Cargo.lock"
test ! -e "$src/.git"
# Build from the extracted source tree, not from the repository build output.
# Registry/build dependencies may be downloaded. This is not an offline build.
env CARGO_TARGET_DIR="$tmp/target" timeout -k 10s 1200s cargo install --locked \
  --path "$src/crates/rawmocs-cli" --root "$tmp/install" \
  > "$out/install.stdout" 2> "$out/install.stderr"
bin="$tmp/install/bin/rawmocs"
"$bin" --version > "$out/version.txt"
grep -Fx "rawmocs $version" "$out/version.txt"
"$bin" --help > "$out/help.txt"
printf 'p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n' > "$tmp/sat-zero.cnf"
printf 'p cnf 1 2\nc t wmc\n1 0\n-1 0\n' > "$tmp/unsat.cnf"
printf 'p cnf 33 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n' > "$tmp/general.cnf"
for name in sat-zero unsat general; do
  "$bin" --digits 60 "$tmp/$name.cnf" > "$out/$name.stdout" 2> "$out/$name.stderr"
done
grep -F 'RAWMOCS_COMPLETE route=general' "$out/general.stderr"
python3 - "$out" <<'PY'
from fractions import Fraction
from pathlib import Path
import sys
out = Path(sys.argv[1])
for name, status, expected in (
    ("sat-zero", "SATISFIABLE", 0),
    ("unsat", "UNSATISFIABLE", 0),
    ("general", "SATISFIABLE", (2**10-1)*2**23),
):
    lines = (out / (name + ".stdout")).read_text().splitlines()
    assert len(lines) == 2 and lines[0] == "s " + status
    row = lines[1].split()
    assert len(row) == 5 and row[:3] == ["c", "r", "mpfr"]
    assert Fraction(row[3]) == expected and Fraction(row[4]) == 0
PY
rc=0
"$bin" --digits 60 --timeout-ms 0 "$tmp/general.cnf" > "$out/deadline.stdout" 2> "$out/deadline.stderr" || rc=$?
test "$rc" -eq 4
test ! -s "$out/deadline.stdout"
grep -F 'RAWMOCS_FAILURE kind=TIMEOUT' "$out/deadline.stderr"
# Verify the installed binary's explicit large route, including the
# unchanged default cap and a parse-inclusive deadline.
printf 'p cnf 4097 0\nc t wmc\nc p weight 1 -0.5 0.25 0\n' > "$tmp/large.cnf"
python3 - "$tmp/large.cnf" <<'PY'
from pathlib import Path
import sys
with Path(sys.argv[1]).open("a") as f:
    for v in range(2, 4098):
        f.write(f"c p weight -{v} 0 0 0\n")
PY
"$bin" --digits 60 --solver general --large-general --timeout-ms 20000 "$tmp/large.cnf" > "$out/large.stdout" 2> "$out/large.stderr"
grep -F 'RAWMOCS_SOURCE_ADMITTED variables=4097 clauses=0 literals=0 ' "$out/large.stderr"
grep -F 'RAWMOCS_COMPLETE route=general digits=60 bits=200 ' "$out/large.stderr"
python3 - "$out/large.stdout" <<'PY'
from fractions import Fraction
from pathlib import Path
import sys
lines = Path(sys.argv[1]).read_text().splitlines()
assert len(lines) == 2 and lines[0] == "s SATISFIABLE"
row = lines[1].split()
assert len(row) == 5 and row[:3] == ["c", "r", "mpfr"]
assert Fraction(row[3]) == Fraction(1, 2) and Fraction(row[4]) == Fraction(1, 4)
PY
rc=0
"$bin" --digits 60 "$tmp/large.cnf" > "$out/large-default.stdout" 2> "$out/large-default.stderr" || rc=$?
test "$rc" -eq 5
test ! -s "$out/large-default.stdout"
grep -F 'RAWMOCS_FAILURE kind=RESOURCE_LIMIT' "$out/large-default.stderr"
rc=0
"$bin" --digits 60 --solver general --large-general --timeout-ms 0 "$tmp/large.cnf" > "$out/large-deadline.stdout" 2> "$out/large-deadline.stderr" || rc=$?
test "$rc" -eq 4
test ! -s "$out/large-deadline.stdout"
grep -F 'RAWMOCS_FAILURE kind=TIMEOUT: source deadline' "$out/large-deadline.stderr"
# Build an independent consumer outside both the repository and extracted tree.
# Seed its lock with the package's exact dependencies, adding only the root.
consumer="$tmp/consumer"
mkdir -p "$consumer/src"
cp "$src/crates/rawmocs-cli/tools/external_consumer.rs" "$consumer/src/main.rs"
cat > "$consumer/Cargo.toml" <<EOF
[package]
name = "rawmocs-api-consumer"
version = "0.1.0"
edition = "2021"

[dependencies]
rawmocs_mpfr = { package = "rawmocs-cli", path = "../rawmocs-$version/crates/rawmocs-cli" }
EOF
python3 - "$src/crates/rawmocs-cli/Cargo.lock" "$consumer/Cargo.lock" <<'PY'
from pathlib import Path
import sys
source, dest = map(Path, sys.argv[1:])
lock = source.read_text()
anchor = '[[package]]\nname = "rawmocs-cli"\n'
assert lock.count(anchor) == 1
root = '[[package]]\nname = "rawmocs-api-consumer"\nversion = "0.1.0"\ndependencies = [\n "rawmocs-cli",\n]\n\n'
dest.write_text(lock.replace(anchor, root + anchor))
PY
cp "$consumer/Cargo.lock" "$tmp/consumer-original.lock"
env CARGO_TARGET_DIR="$tmp/target" timeout -k 10s 1200s cargo build --locked --offline --release \
  --manifest-path "$consumer/Cargo.toml" > "$out/consumer-build.stdout" 2> "$out/consumer-build.stderr"
cmp "$consumer/Cargo.lock" "$tmp/consumer-original.lock"
cmp "$src/crates/rawmocs-cli/Cargo.lock" "$root/crates/rawmocs-cli/Cargo.lock"
timeout -k 2s 30s "$tmp/target/release/rawmocs-api-consumer" "$tmp/sat-zero.cnf" \
  > "$out/consumer.stdout" 2> "$out/consumer.stderr"
test ! -s "$out/consumer.stderr"
test "$(cat "$out/consumer.stdout")" = EXTRACTED_RUST_CONSUMER_PASS
cat "$out/consumer.stdout"
cp "$consumer/Cargo.lock" "$out/consumer-Cargo.lock"
rustc -Vv > "$out/rustc.txt"
cargo -V > "$out/cargo.txt"
ldd "$bin" > "$out/linked-libraries.txt"
python3 - "$out" "$sha" "$version" "$archive" "$bin" "$src/crates/rawmocs-cli/Cargo.lock" <<'PY'
import hashlib
import json
from pathlib import Path
import sys
out, sha, version, archive, binary, lock = sys.argv[1:]
def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
p = Path(out)
receipt = {
    "status": "SOURCE_PACKAGE_INSTALL_PASS",
    "revision": sha,
    "version": version,
    "target": "x86_64-unknown-linux-gnu",
    "source_archive": archive,
    "source_sha256": digest(p / archive),
    "cargo_lock_sha256": digest(lock),
    "installed_binary_sha256": digest(binary),
    "deterministic_archive": True,
    "controls": ["version", "help", "SAT-zero", "UNSAT", "general", "timeout"],
    "large_general_controls": ["analytic", "default-cap", "parse-deadline"],
    "rust_consumer_controls": ["file-text", "SAT-zero", "UNSAT", "MPFR-binary", "diagnostics",
                               "invalid-input", "timeout", "cancel", "reentry", "quiet-stdio", "locked-offline"],
    "redistributed_binary": False,
    "offline_build": False,
}
(p / "source-package.json").write_text(json.dumps(receipt, indent=2) + "\n")
PY
printf 'SOURCE_PACKAGE_INSTALL_PASS %s\n' "$sha"
