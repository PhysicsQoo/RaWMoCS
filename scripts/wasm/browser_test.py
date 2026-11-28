"""Real Chromium Worker gate, exact dyadic oracle and failure/lifecycle controls."""
from decimal import Decimal
import functools, http.server, json, subprocess, sys, threading
from pathlib import Path
from playwright.sync_api import sync_playwright

root=Path(sys.argv[1]).resolve()
native=Path(sys.argv[2]).resolve()
assert native.is_file()
class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*args): pass
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(Handler,directory=str(root)))
threading.Thread(target=server.serve_forever,daemon=True).start()
try:
    with sync_playwright() as p:
        browser=p.chromium.launch(headless=True)
        page=browser.new_page()
        page.goto(f"http://127.0.0.1:{server.server_port}/index.html")
        cases=[
          ("signed","p cnf 1 1\nc t wmc\nc p weight 1 -0.125 0.25 0\n1 0\n",True,"-0.125","0.25"),
          ("sat_zero","p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",True,"0","0"),
          ("unsat","p cnf 1 2\nc t wmc\n1 0\n-1 0\n",False,"0","0"),
        ]
        evaluate="""async (req)=>new Promise((resolve,reject)=>{
          const w=new Worker("worker.js"); let done=false;
          const finish=(v)=>{if(done)return;done=true;clearTimeout(timer);w.terminate();resolve(v)};
          const timer=setTimeout(()=>finish({type:"watchdog"}),15000);
          w.onerror=e=>finish({type:"worker-error",message:e.message});
          w.onmessage=({data})=>{
            if(data.id!==req.id) return finish({type:"wrong-id"});
            if(data.type==="ready"){
              if(req.stop){w.terminate();setTimeout(()=>finish({type:"stopped"}),100);return;}
              w.postMessage({type:"solve",id:req.id});
            } else finish(data);
          };
          w.postMessage({...req,type:"prepare"});
        })"""
        rows=[]
        def native_count(cnf,digits,mode):
            p=root/"native-input.cnf";p.write_text(cnf)
            try:
                result=subprocess.run([str(native),"--digits",str(digits),"--solver",mode,"--timeout-ms","2000",str(p)],capture_output=True,text=True,timeout=10,check=True)
                return result.stdout.strip()
            finally:p.unlink()
        def run(cnf,digits=30,mode="auto",timeout=2000,stop=False):
            return page.evaluate(evaluate,dict(id=len(rows)+1,cnf=cnf,digits=digits,mode=mode,timeout=timeout,stop=stop))
        for digits in (30,60,100,130):
            for mode in ("auto","general"):
                for label,cnf,sat,re,im in cases:
                    r=run(cnf,digits,mode)
                    assert r["type"]=="count",r
                    assert r["stdout"]==native_count(cnf,digits,mode),r
                    lines=r["stdout"].splitlines()
                    assert lines[0]==("s SATISFIABLE" if sat else "s UNSATISFIABLE"),r
                    a,b=lines[1].split()[3:5]
                    assert Decimal(a)==Decimal(re) and Decimal(b)==Decimal(im),r
                    assert f"digits={digits}" in r["stderr"] and f"bits={dict(zip((30,60,100,130),(100,200,333,432)))[digits]}" in r["stderr"],r
                    rows.append(dict(case=label,digits=digits,mode=mode,result=r))
        tiny="p cnf 1 1\nc t wmc\nc p weight 1 1e-400 0 0\n1 0\n"
        for digits in (30,60,100,130):
            for mode in ("auto","general"):
                r=run(tiny,digits,mode)
                assert r["type"]=="count" and r["stdout"]==native_count(tiny,digits,mode),r
                assert Decimal(r["stdout"].splitlines()[1].split()[3]) != 0,r
                rows.append(dict(case="tiny_nonzero",digits=digits,mode=mode,result=r))
        for label,cnf,timeout,code,kind in (("invalid","not a CNF",2000,2,"INVALID_INPUT"),("timeout",cases[0][1],0,4,"TIMEOUT")):
            r=run(cnf,timeout=timeout)
            assert r["type"]=="failure" and r["status"]==code and not r["stdout"] and kind in r["stderr"],r
            rows.append(dict(case=label,result=r))
        r=run(cases[0][1],stop=True);assert r["type"]=="stopped",r
        rows.append(dict(case="stop_before_solve",result=r))
        r=run(cases[0][1]);assert r["type"]=="count",r
        rows.append(dict(case="fresh_worker_after_stop",result=r))
        (root/"browser-results.json").write_text(json.dumps({"browser":browser.version,"rows":rows},indent=2)+"\n")
        browser.close()
        print("W1B_BROWSER_WORKER_PASS cases=32 failures=2 stop=1 reentry=1")
finally:
    server.shutdown()
