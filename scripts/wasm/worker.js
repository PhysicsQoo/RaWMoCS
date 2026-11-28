// Disposable single-solve worker. Never synthesize a numerical result on failure.
importScripts("rawmocs.js");
let mod, request, out=[], err=[];
self.onmessage=async ({data})=>{
  try {
    if(data.type==="prepare" && !request) {
      if(typeof data.cnf!=="string" || new TextEncoder().encode(data.cnf).length>1048576 ||
         ![30,60,100,130].includes(data.digits) || !["auto","general","prejoin","auto-frontier","auto-separator","general-separator"].includes(data.mode) ||
         !Number.isInteger(data.timeout) || data.timeout<0 || data.timeout>200000) throw Error("INVALID_REQUEST");
      request=data;
      mod=await createRawmocs({noInitialRun:true,print:x=>out.push(String(x)),printErr:x=>err.push(String(x)),
        locateFile:x=>new URL(x,self.location.href).href});
      self.postMessage({type:"ready",id:request.id});
    } else if(data.type==="solve" && mod && data.id===request.id) {
      const m=mod;mod=null;
      m.FS.writeFile("/input.cnf",request.cnf);
      let status=0;
      try { status=m.callMain(["--digits",String(request.digits),"--solver",request.mode,"--timeout-ms",String(request.timeout),"/input.cnf"])||0; }
      catch(e){ if(e && typeof e.status==="number") status=e.status; else throw e; }
      const stdout=out.join("\n");
      const stderr=err.join("\n");
      if(status!==0) {
        self.postMessage({type:"failure",id:request.id,status,stderr,stdout});
      } else {
        if(!/^s (SATISFIABLE|UNSATISFIABLE)\nc r mpfr \S+ \S+$/.test(stdout.trim())) throw Error("INVALID_COUNT_OUTPUT");
        self.postMessage({type:"count",id:request.id,status,stdout:stdout.trim(),stderr});
      }
    } else throw Error("INVALID_WORKER_STATE");
  }catch(e){ self.postMessage({type:"error",id:data.id,message:String(e)}); }
};
