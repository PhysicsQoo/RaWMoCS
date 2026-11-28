"""Bounded active cancellation and input/resource-failure controls in Chromium."""
import functools,http.server,json,sys,threading
from pathlib import Path
from playwright.sync_api import sync_playwright
root=Path(sys.argv[1]).resolve()
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(http.server.SimpleHTTPRequestHandler,directory=str(root)))
threading.Thread(target=server.serve_forever,daemon=True).start()
# Deterministic dense signed-complex CNF workload, only for cancellation.
# No assertion about solver difficulty or performance. Completion before
# the stop is a HOLD, not a successful active-stop witness.
import random
rng=random.Random(3401)
n=100;clauses=[]
for _ in range(420):
    clauses.append(" ".join(str(v if rng.randrange(2) else -v) for v in rng.sample(range(1,n+1),3))+" 0")
hard=f"p cnf {n} {len(clauses)}\nc t wmc\n"+"\n".join(clauses)+"\n"
small="p cnf 1 1\nc t wmc\nc p weight 1 -0.125 0.25 0\n1 0\n"
js="""async req=>new Promise(resolve=>{
 const w=new Worker(req.active?"resource-test-worker.js":"worker.js");
 let done=false,started=false,stopTimer;
 const finish=v=>{if(done)return;done=true;clearTimeout(timer);clearTimeout(stopTimer);w.terminate();resolve(v)};
 const timer=setTimeout(()=>finish({type:"watchdog"}),12000);
 w.onerror=e=>finish({type:"worker-error",message:e.message});
 w.onmessage=({data})=>{
   if(data.id!==req.id)return finish({type:"wrong-id"});
   if(data.type==="ready"){w.postMessage({type:"solve",id:req.id});return}
   if(data.type==="started"){
      started=true;
      stopTimer=setTimeout(()=>{w.terminate();finish({type:"active-stop",started})},50);
      return
   }
   finish(data);
 };
 w.postMessage({type:"prepare",id:req.id,cnf:req.cnf,digits:30,mode:"general",timeout:2000});
})"""
try:
 with sync_playwright() as p:
    b=p.chromium.launch(headless=True);page=b.new_page()
    page.goto(f"http://127.0.0.1:{server.server_port}/index.html")
    rows=[]
    for label,cnf,active in (("active_stop",hard,True),("oversize","x"*1048577,False),("fresh_worker",small,False)):
        r=page.evaluate(js,dict(id=len(rows)+1,cnf=cnf,active=active))
        rows.append(dict(case=label,result=r))
        print(label,json.dumps(r),flush=True)
    (root/"resource-results.json").write_text(json.dumps(rows,indent=2)+"\n")
    assert rows[0]["result"]=={"type":"active-stop","started":True},rows[0]
    assert rows[1]["result"]["type"]=="error" and "INVALID_REQUEST" in rows[1]["result"]["message"],rows[1]
    assert rows[2]["result"]["type"]=="count",rows[2]
    b.close()
    print("WASM_RESOURCE_SCOPED_PASS active_stop=1 oversize=1 reentry=1")
finally:server.shutdown()
