"""Offline corresponding-source rebuild; installed pinned Rust/emsdk required."""
import json,os,subprocess,sys,tempfile
from pathlib import Path

def command(source,target,config,offline_vendor=False):
    env=dict(os.environ,CC="emcc",CXX="em++",AR="emar",RANLIB="emranlib",CC_FOR_BUILD="gcc",CFLAGS_FOR_BUILD="-O2",
      CC_wasm32_unknown_emscripten="emcc",AR_wasm32_unknown_emscripten="emar",CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER="emcc",
      GMP_MPFR_SYS_CACHE="_",CARGO_TARGET_DIR=str(target),SOURCE_DATE_EPOCH=config["source_date_epoch"])
    cmd=["cargo"]
    if offline_vendor:
        cmd+=["--config",'source.crates-io.replace-with="vendored-sources"',
              "--config","source.vendored-sources.directory="+json.dumps(str(source/"vendor"))]
    cmd+=["rustc","--offline","--locked","--release","--target",config["target"],"--bin","rawmocs","-j","2","--"]
    for flag in config["link_flags"]:cmd+=["-C","link-arg="+flag]
    return cmd,env

def main():
    source,target=(Path(p).resolve() for p in sys.argv[1:])
    if target.exists():raise ValueError("fresh target required")
    config=json.loads((source/"build-config.json").read_text())
    assert subprocess.check_output(["rustc","--version"],text=True).split()[1]=="1.100.1"
    assert "4.0.23" in subprocess.check_output(["emcc","--version"],text=True)
    assert config["target"]=="wasm32-unknown-emscripten"
    cmd,env=command(source,target,config,True)
    with tempfile.TemporaryDirectory() as cargo_home:
        env["CARGO_HOME"]=cargo_home
        env["CARGO_NET_OFFLINE"]="true"
        subprocess.run(cmd,cwd=source/"rawmocs",env=env,check=True)
    print("WASM_OFFLINE_VENDOR_REPLAY_PASS",flush=True)
if __name__=="__main__":main()
