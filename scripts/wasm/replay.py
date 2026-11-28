"""Offline corresponding-source rebuild; installed pinned Rust/emsdk required."""
import json,os,subprocess,sys,tempfile
from pathlib import Path,PurePosixPath

def command(source,target,config):
    env=dict(os.environ,CC="emcc",CXX="em++",AR="emar",RANLIB="emranlib",CC_FOR_BUILD="gcc",CFLAGS_FOR_BUILD="-O2",
      CC_wasm32_unknown_emscripten="emcc",AR_wasm32_unknown_emscripten="emar",CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER="emcc",
      GMP_MPFR_SYS_CACHE="_",CARGO_TARGET_DIR=str(target),SOURCE_DATE_EPOCH=config["source_date_epoch"])
    cmd=["cargo","--config",'source.crates-io.replace-with="vendored-sources"',
         "--config","source.vendored-sources.directory="+json.dumps(str(source/"vendor"))]
    cmd+=["rustc","--offline","--locked","--release","--target",config["target"],"--bin","rawmocs","-j","2","--"]
    for flag in config["link_flags"]:cmd+=["-C","link-arg="+flag]
    # Diagnostic output only; no additional build or solver option.
    cmd+=["-C","link-arg=-Wl,-Map="+str(target/"rawmocs-link.map")]
    return cmd,env

def main():
    source,target=(Path(p).resolve() for p in sys.argv[1:])
    if target.exists():raise ValueError("fresh target required")
    config=json.loads((source/"build-config.json").read_text())
    assert subprocess.check_output(["rustc","--version"],text=True).split()[1]=="1.100.1"
    assert "4.0.23" in subprocess.check_output(["emcc","--version"],text=True)
    assert config["target"]=="wasm32-unknown-emscripten"
    executable=json.loads((source/"executable-files.json").read_text())
    assert isinstance(executable,list) and len(executable)==len(set(executable))
    for n in executable:
        p=PurePosixPath(n)
        assert not p.is_absolute() and ".." not in p.parts and "\\" not in n
        path=source/n
        assert path.is_file() and not path.is_symlink() and path.resolve().is_relative_to(source)
        path.chmod(0o755)
    cmd,env=command(source,target,config)
    with tempfile.TemporaryDirectory() as cargo_home:
        env["CARGO_HOME"]=cargo_home
        env["CARGO_NET_OFFLINE"]="true"
        subprocess.run(cmd,cwd=source/"rawmocs",env=env,check=True)
    print("WASM_OFFLINE_VENDOR_REPLAY_PASS",flush=True)
if __name__=="__main__":main()
