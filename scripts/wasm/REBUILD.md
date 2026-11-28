# Rebuilding and relinking the WASM combined work

RaWMoCS is MIT licensed. The statically linked Rug, gmp-mpfr-sys, GMP and
MPFR libraries retain their respective licenses, including LGPL terms.
The license texts and corresponding source are in this archive. Runtime
toolchain notices and the final link map are under toolchain/. No additional
restriction is imposed on modification of the libraries or reverse engineering
to debug such modifications under the applicable licenses.

Install Rust 1.100.1 with wasm32-unknown-emscripten and Emscripten SDK 4.0.23,
plus Python 3, make, gcc and m4. Activate emsdk so emcc, emar and emranlib
are on PATH. From the extracted source archive run:

    python3 recipe/replay.py . /path/to/new-empty-target

The target directory must not yet exist. The generated rawmocs.js and
rawmocs.wasm are in the target's wasm32-unknown-emscripten/release directory.
The worker adapter is recipe/worker.js. This command uses the vendored,
locked dependencies offline.

For a modified combined work, copy this source tree, edit the LGPL library
sources under vendor/ (including the bundled GMP/MPFR sources in
gmp-mpfr-sys), and rebuild using a new target directory. Cargo verifies
vendored file checksums. After edits, regenerate the modified crate's
.cargo-checksum.json files mapping: retain its package field (the lockfile
registry identity), remove deleted file entries, and set each remaining or
new source file entry to the SHA-256 of the local file. Do not include
.cargo-checksum.json itself. For a modified crate with directory CRATE_DIR:

    python3 - CRATE_DIR <<'PY'
    import hashlib, json, pathlib, sys
    root = pathlib.Path(sys.argv[1])
    path = root / ".cargo-checksum.json"
    data = json.loads(path.read_text())
    data["files"] = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in sorted(root.rglob("*"))
                     if p.is_file() and p != path}
    path.write_text(json.dumps(data, sort_keys=True))
    PY

Then invoke recipe/replay.py again with a new target. Serve the rebuilt JS,
WASM and worker from a local HTTP server with a compatible page, or use
your own worker adapter. A modified build is not the official released
artifact: its hashes and provenance must not be represented as the original.
The official Playground does not impose any restriction on independently
running the modified combined work.
