"""Actual Chromium, exact analytical WMC cases, no external solver."""
import functools,http.server,json,sys,threading,decimal,hashlib
from pathlib import Path
from playwright.sync_api import sync_playwright
W=Path(sys.argv[1]);D=decimal.Decimal
class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(Handler,directory=str(W/"site")))
threading.Thread(target=server.serve_forever,daemon=True).start()
cases=[
("complex","p cnf 1 1\nc t wmc\nc p weight 1 -0.5 0.25 0\n1 0\n",True,"-0.5","0.25"),
("sat_zero","p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",True,"0","0"),
("unsat","p cnf 1 2\nc t wmc\n1 0\n-1 0\n",False,"0","0"),
("unweighted","p cnf 2 1\nc t wmc\n1 2 0\n",True,"3","0")]
rows=[]
try:
    with sync_playwright() as p:
        browser=p.chromium.launch();page=browser.new_page()
        page.goto(f"http://127.0.0.1:{server.server_port}/")
        page.wait_for_function("document.querySelector('#state').textContent==='Ready'")
        for name,cnf,sat,re,im in cases:
            for digits in (30,60,100,130):
                page.locator("#cnf").fill(cnf);page.locator("#digits").select_option(str(digits));page.locator("#run").click()
                page.wait_for_function("document.querySelector('#state').textContent!=='Running'",timeout=12000)
                assert page.locator("#state").inner_text()=="Completed",page.locator("#state").inner_text()
                r=json.loads(page.locator("#result").inner_text())
                assert r["sat"]==sat and D(r["real"])==D(re) and D(r["imag"])==D(im),(name,digits,r)
                assert r["precision_digits"]==digits and r["input_sha256"]==hashlib.sha256(cnf.encode()).hexdigest()
                rows.append(dict(case=name,digits=digits,result=r))
        page.locator("#cnf").fill("invalid");page.locator("#run").click()
        page.wait_for_function("document.querySelector('#state').textContent!=='Running'",timeout=12000)
        assert page.locator("#state").inner_text().startswith("SOLVER_FAILURE")
        assert not page.locator("#result").inner_text() and page.locator("#download").is_disabled()
        page.locator("#cnf").fill(cases[0][1])
        page.evaluate("()=>{document.querySelector('#run').click();document.querySelector('#stop').click()}")
        assert page.locator("#state").inner_text()=="CANCELLED"
        assert page.locator("#download").is_disabled()
        page.locator("#run").click()
        page.wait_for_function("document.querySelector('#state').textContent==='Completed'",timeout=12000)
        assert not page.locator("#download").is_disabled()
        browser.close()
        print("STANDALONE_PLAYGROUND_SCOPED_PASS cases=16 failure=1 cancellation=1 reentry=1")
finally:
    (W/"smoke.json").write_text(json.dumps(rows,indent=2)+"\n")
    server.shutdown()
