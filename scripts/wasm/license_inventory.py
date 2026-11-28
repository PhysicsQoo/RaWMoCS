"""Collect notices from the exact installed toolchains, not a moving web branch.

This is a conservative notice inventory, not an assertion that every file
is linked. The final link map is retained for human redistribution review.
"""
import hashlib,json,re,shutil,subprocess
from pathlib import Path

def is_notice(name):
    n=Path(name).name.upper()
    return (n.startswith(("LICENSE","COPYING","NOTICE","COPYRIGHT","AUTHORS"))
            and not n.endswith((".PY",".RS",".JS",".C",".H")))

def sha(data): return hashlib.sha256(data).hexdigest()

def collect(source):
    root=source/"toolchain"
    root.mkdir()
    sysroot=Path(subprocess.check_output(["rustc","--print","sysroot"],text=True).strip())
    rustlib=sysroot/"lib/rustlib/src/rust"
    emcc=Path(shutil.which("emcc") or "").resolve()
    em=emcc.parent
    assert (rustlib/"library").is_dir(), "install matching rust-src"
    assert (em/"system/lib").is_dir() and (em/"LICENSE").is_file(), "Emscripten SDK source missing"
    def copy(origin,folder,recursive=False):
        if not origin.is_dir(): return
        for p in sorted(origin.rglob("*") if recursive else origin.iterdir()):
            if p.is_file() and not p.is_symlink() and is_notice(p.name):
                out=root/folder/p.relative_to(origin)
                out.parent.mkdir(parents=True,exist_ok=True)
                out.write_bytes(p.read_bytes())
    copy(sysroot/"share/doc/rust","rust/distribution")
    copy(sysroot/"share/doc/rust/html","rust/html")
    copy(rustlib,"rust/source")
    copy(rustlib/"library","rust/library",True)
    copy(em,"emscripten")
    for d in ("system","src","third_party"):
        copy(em/d,"emscripten/"+d,True)
    copy(em.parent,"llvm-distribution")
    inventory={p.relative_to(root).as_posix():sha(p.read_bytes())
               for p in sorted(root.rglob("*")) if p.is_file()}
    rust=[n for n in inventory if n.startswith("rust/")]
    assert any(Path(n).name=="LICENSE-MIT" for n in rust)
    assert any(Path(n).name=="LICENSE-APACHE" for n in rust)
    assert any(Path(n).name.upper().startswith("COPYRIGHT") for n in rust), "Rust copyright inventory missing"
    assert "emscripten/LICENSE" in inventory
    assert "emscripten/system/lib/libc/musl/COPYRIGHT" in inventory
    rust_version=subprocess.check_output(["rustc","-vV"],text=True)
    commit=re.search(r"^commit-hash: ([0-9a-f]{40})$",rust_version,re.M)
    assert commit and "release: 1.100.1" in rust_version
    em_version=subprocess.check_output(["emcc","--version"],text=True)
    assert "4.0.23" in em_version
    (root/"inventory.json").write_text(json.dumps({
        "rust_commit":commit.group(1),"rust_version":rust_version.strip(),
        "emscripten_version":em_version.strip(),"files":inventory,
        "scope":"conservative installed runtime notices; review link-map.txt for actual link inputs",
        "rust_source":"https://github.com/rust-lang/rust/tree/"+commit.group(1),
        "emscripten_source":"https://github.com/emscripten-core/emscripten/tree/4.0.23"
    },sort_keys=True,indent=2)+"\n")
