"""Real Chromium versus native counters on the same descriptor and materialized CNF."""
import decimal,functools,http.server,json,os,re,subprocess,sys,tempfile,threading
from pathlib import Path
from playwright.sync_api import sync_playwright
D=decimal.Decimal;decimal.getcontext().prec=160
root,phase,gpmc,raw,evidence=map(Path,sys.argv[1:])
cases=[("identity","0","1","0","1",0,96),("minus","1","2","0","1",0,96),
       ("i","1","4","0","1",0,96),("minus_i","3","4","0","1",0,96),
       ("eighth","1","8","0","1",0,96),("radians","0","1","1","7",1,256),
       ("mixed","1","8","-1","7",1,1024),("tiny","0","1","1","1"+"0"*400,1,1024)]
def source(cn,cd,rn,rd,p,bits):
    return f"p cnf 1 1\nc t wmc\nc p weight 1 1 0 0\nc p weight -1 1 0 0\n1 0\nc p quprs-phase 1 {cn} {cd} {rn} {rd} {p} {bits}\n"
def run(args,tmp):
    return subprocess.run(list(map(str,args)),env={**os.environ,"TMPDIR":str(tmp)},capture_output=True,text=True,timeout=20)
def rawcount(stdout):
    a=stdout.strip().splitlines()
    assert len(a)==2 and a[0]=="s SATISFIABLE" and a[1].startswith("c r mpfr "),stdout
    _,_,_,re_,im=a[1].split();return D(re_),D(im)
def gcount(stdout):
    assert "c o quprs-phase-weights 1" in stdout,stdout
    line=next(x for x in stdout.splitlines() if x.startswith("c s exact "))
    v=re.sub(r"^c s exact (double prec-sci|arb cpx)\s+","",line).replace(" ","").replace("+-","-")
    if not v.endswith("i"):return D(v),D(0)
    v=v[:-1];i=[i for i in range(1,len(v)) if v[i] in "+-" and v[i-1] not in "eE"][-1]
    return D(v[:i]),D(v[i:])
def close(a,b):
    for x,y in zip(a,b):
        assert x.is_finite() and y.is_finite()
        assert x==y if y==0 else abs(x-y)<=abs(y)*D("1e-26"),(a,b)
class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(Handler,directory=str(root)))
threading.Thread(target=server.serve_forever,daemon=True).start()
rows=[]
try:
    with sync_playwright() as p:
        browser=p.chromium.launch(headless=True);page=browser.new_page()
        page.goto(f"http://127.0.0.1:{server.server_port}/index.html")
        js="""async ({file,req})=>new Promise(resolve=>{
          const w=new Worker(file);let done=false;
          const finish=r=>{if(done)return;done=true;clearTimeout(timer);w.terminate();resolve(r)};
          const timer=setTimeout(()=>finish({type:"watchdog"}),15000);
          w.onerror=e=>finish({type:"error",message:e.message});
          w.onmessage=({data})=>{
            if(data.progress)return;
            if(file!=="runner.js" && data.id!==req.id)return finish({type:"wrong-id"});
            if(data.type==="ready")w.postMessage({type:"solve",id:req.id});else finish(data);
          };w.postMessage(req);
        })"""
        def browser_call(file,req):return page.evaluate(js,dict(file=file,req=req))
        for label,*args in cases:
            cnf=source(*args)
            with tempfile.TemporaryDirectory() as tmp:
                t=Path(tmp);f=t/"source.cnf";f.write_text(cnf);m=t/"materialized.cnf"
                a=run([phase,f,m],t);assert a.returncode==0 and "c o quprs-phase-weights 1" in a.stdout,a.stderr
                b=browser_call("phase-worker.js",dict(id=1,cnf=cnf));assert b["type"]=="materialized" and b["ack"]=="c o quprs-phase-weights 1",b
                assert b["cnf"]==m.read_text(),label
                g=run([gpmc,"-mode=1","-prec=30","-cs=64",f],t);assert g.returncode==0,g.stderr
                gb=browser_call("runner.js",dict(engine="gpmc",cnf=cnf));assert "error" not in gb,gb
                expected=gcount(g.stdout);close(gcount(gb["stdout"]),expected)
                for digits in (30,60,100,130):
                    n=run([raw,"--digits",digits,"--solver","auto","--timeout-ms",2000,m],t);assert n.returncode==0,n.stderr
                    wb=browser_call("worker.js",dict(type="prepare",id=2,cnf=b["cnf"],digits=digits,mode="auto",timeout=2000))
                    assert wb["type"]=="count",wb
                    nr=rawcount(n.stdout);wr=rawcount(wb["stdout"]);assert nr==wr,(label,digits,nr,wr)
                    # GPMC is configured at 30 digits; cross-engine relative comparison is scoped to 30 digits.
                    if digits==30:close(wr,expected)
                    if label=="tiny":assert wr[1]!=0
                    rows.append(dict(case=label,digits=digits,native=n.stdout,browser=wb["stdout"],gpmc=g.stdout,ack=b["ack"]))
        good=source(*cases[0][1:])
        for label,cnf in [("missing_placeholder",good.replace("c p weight 1 1 0 0\n","")),("duplicate",good+good.splitlines()[-1]+"\n"),("zero_denominator",source("1","0","0","1",0,96)),("invalid_bits",source("1","2","0","1",0,2048))]:
            b=browser_call("phase-worker.js",dict(id=3,cnf=cnf));assert b["type"]=="error" and "cnf" not in b,b
            rows.append(dict(case=label,result=b))
        browser.close()
        print("W2A_PHASE_PARITY_PASS cases=32 failures=4")
finally:
    (evidence/"parity.json").write_text(json.dumps(rows,indent=2)+"\n")
    server.shutdown()
