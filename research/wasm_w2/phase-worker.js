// One materialization per disposable worker; no synthetic acknowledgement.
importScripts("phase.js");
self.onmessage=async({data})=>{
  let out=[],err=[];
  try{
    if(typeof data.cnf!=="string" || new TextEncoder().encode(data.cnf).length>1048576) throw Error("INVALID_INPUT");
    const m=await createPhaseBridge({noInitialRun:true,print:x=>out.push(String(x)),printErr:x=>err.push(String(x))});
    m.FS.writeFile("/input.cnf",data.cnf);
    let status=0;
    try{status=m.callMain(["/input.cnf","/materialized.cnf"])||0;}
    catch(e){if(e && typeof e.status==="number") status=e.status;else throw e;}
    if(status!==0) throw Error("PHASE_BRIDGE_FAILED "+err.join("\n"));
    const cnf=m.FS.readFile("/materialized.cnf",{encoding:"utf8"});
    const expected=data.cnf.split(/\r?\n/).filter(x=>x.startsWith("c p quprs-phase ")).length;
    const ack=out.filter(x=>x.startsWith("c o quprs-phase-weights "));
    if((expected ? ack.length!==1 || ack[0]!==`c o quprs-phase-weights ${expected}` : ack.length!==0) ||
       cnf.split(/\r?\n/).some(x=>x.startsWith("c p quprs-phase "))) throw Error("PHASE_ACK_MISMATCH");
    self.postMessage({type:"materialized",id:data.id,cnf,ack:ack[0]||null});
  }catch(e){self.postMessage({type:"error",id:data.id,message:String(e)});}
};
