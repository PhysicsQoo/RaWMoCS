const el=id=>document.getElementById(id);
const CAP=1048576;
let active=null,report=null,provenance=null,nextId=0;
const setState=text=>{el("state").textContent=text;};
const render=value=>{el("result").textContent=value?JSON.stringify(value,null,2):"";};
function clear(){report=null;render(null);el("download").disabled=true;}
function finish(job,state,value=null){
  if(active!==job)return;
  clearTimeout(job.timer);job.worker.terminate();active=null;
  el("run").disabled=!provenance;el("stop").disabled=true;
  for(const id of ["cnf","file","digits","mode","timeout"])el(id).disabled=false;
  report=value;render(value);el("download").disabled=!value;setState(state);
}
async function digest(text){
  const bytes=await crypto.subtle.digest("SHA-256",new TextEncoder().encode(text));
  return Array.from(new Uint8Array(bytes),v=>v.toString(16).padStart(2,"0")).join("");
}
for(const id of ["cnf","digits","mode","timeout"])el(id).addEventListener("input",clear);
el("file").addEventListener("change",async()=>{
  clear();const f=el("file").files[0];if(!f)return;
  if(f.size>CAP){setState("INVALID_INPUT: file too large");return;}
  try{el("cnf").value=await f.text();setState("Input loaded");}catch{setState("INVALID_INPUT: read failed");}
});
el("stop").onclick=()=>{if(active)finish(active,"CANCELLED");};
el("run").onclick=async()=>{
  if(active||!provenance)return;clear();
  const cnf=el("cnf").value,digits=Number(el("digits").value),mode=el("mode").value,timeout=Number(el("timeout").value);
  if(!cnf.trim()||new TextEncoder().encode(cnf).length>CAP||!Number.isInteger(timeout)||timeout<1||timeout>200000){
    setState("INVALID_INPUT: check input and timeout");return;
  }
  const id=++nextId,worker=new Worker("worker.js"),job={id,worker,timer:null,start:performance.now()};
  active=job;el("run").disabled=true;el("stop").disabled=false;
  for(const key of ["cnf","file","digits","mode","timeout"])el(key).disabled=true;
  setState("Running");
  job.timer=setTimeout(()=>finish(job,"TIMEOUT"),timeout);
  worker.onerror=()=>finish(job,"WORKER_FAILURE");
  worker.onmessage=async({data})=>{
    if(active!==job)return;
    if(data.id!==id){finish(job,"INVALID_RESPONSE");return;}
    if(data.type==="ready"){worker.postMessage({type:"solve",id});return;}
    if(data.type!=="count"||data.status!==0){
      finish(job,"SOLVER_FAILURE: "+String(data.stderr||data.message||data.type).slice(0,1000));return;
    }
    const m=/^s (SATISFIABLE|UNSATISFIABLE)\nc r mpfr (\S+) (\S+)$/.exec(data.stdout.trim());
    const receipt=/RAWMOCS_COMPLETE route=(\S+) digits=(\d+) bits=(\d+) solve_ns=(\d+)/.exec(data.stderr);
    const decimal=/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;
    const zero=s=>/^[+-]?0+(?:\.0*)?(?:[eE][+-]?\d+)?$/.test(s);
    const bits={30:100,60:200,100:333,130:432};
    if(!m||!receipt||!decimal.test(m[2])||!decimal.test(m[3])||Number(receipt[2])!==digits||Number(receipt[3])!==bits[digits]||(m[1]==="UNSATISFIABLE"&&(!zero(m[2])||!zero(m[3])))){
      finish(job,"INVALID_RESPONSE");return;
    }
    try{
      const value={solver:"RaWMoCS",version:provenance.version,sat:m[1]==="SATISFIABLE",real:m[2],imag:m[3],precision_digits:digits,precision_bits:bits[digits],requested_mode:mode,actual_route:receipt[1],solve_ns:receipt[4],wall_ms:performance.now()-job.start,input_sha256:await digest(cnf),artifact_sha256:provenance.distribution_sha256,certified_interval:false};
      finish(job,"Completed",value);
    }catch{finish(job,"REPORT_FAILURE");}
  };
  worker.postMessage({type:"prepare",id,cnf,digits,mode,timeout});
};
el("download").onclick=()=>{
  if(!report)return;const url=URL.createObjectURL(new Blob([JSON.stringify(report,null,2)+"\n"],{type:"application/json"}));
  const a=document.createElement("a");a.href=url;a.download="rawmocs-report.json";a.click();URL.revokeObjectURL(url);
};
try{
  const r=await fetch("PROVENANCE.json");if(!r.ok)throw Error("provenance");
  provenance=await r.json();
  if(provenance.solver!=="RaWMoCS"||!provenance.distribution_sha256?.["rawmocs.wasm"])throw Error("identity");
  el("version").textContent="RaWMoCS "+provenance.version+" · "+provenance.status;
  el("run").disabled=false;setState("Ready");
}catch{provenance=null;setState("BUILD_IDENTITY_FAILURE");}
