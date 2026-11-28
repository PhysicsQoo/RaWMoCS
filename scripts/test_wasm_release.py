import json,unittest
from wasm_release import read_zip,zip_bytes,validate_pin,verify_review,verify_distribution,verify_receipt
from release_source import digest
class Contract(unittest.TestCase):
    def setUp(self):
        self.r={"version":"0.5.0","revision":"a"*40,"wasm":{"run":1,"id":2,"digest":"sha256:"+"b"*64,"review_sha256":"c"*64}}
    def test_pin(self):
        validate_pin(self.r["wasm"])
        for bad in ({"run":True},{"digest":"wrong"},{"review_sha256":"wrong"},{"extra":1}):
            with self.assertRaises(ValueError):validate_pin(self.r["wasm"]|bad)
    def test_zip_reproducible_and_rejects_traversal(self):
        self.assertEqual(zip_bytes({"b":b"2","a":b"1"}),zip_bytes({"a":b"1","b":b"2"}))
        self.assertEqual(read_zip(zip_bytes({"a":b"1"})),{"a":b"1"})
        with self.assertRaises(ValueError):read_zip(zip_bytes({"../escape":b"bad"}))
    def test_distribution_notices(self):
        files={"rawmocs.js":b"js","worker.js":b"worker","rawmocs.wasm":b"\x00asm",
               "THIRD-PARTY-NOTICES.txt":b"https://github.com/PhysicsQoo/RaWMoCS/releases/tag/v0.5.0",
               "REBUILD.md":b"relink","licenses/rawmocs/LICENSE":b"MIT",
               "licenses/toolchain/inventory.json":b"{}","licenses/toolchain/emscripten/LICENSE":b"MIT",
               "licenses/toolchain/emscripten/system/lib/libc/musl/COPYRIGHT":b"MIT"}
        runtime=("rawmocs.js","rawmocs.wasm","worker.js")
        m={"version":"0.5.0","distribution_sha256":{n:digest(files[n]) for n in runtime},
           "license_sha256":{n:digest(v) for n,v in files.items() if n not in runtime}}
        verify_distribution(m,files)
        for name in ("THIRD-PARTY-NOTICES.txt","REBUILD.md","licenses/toolchain/emscripten/LICENSE"):
            bad=files.copy();del bad[name]
            with self.assertRaises(ValueError):verify_distribution(m,bad)
        with self.assertRaises(ValueError):verify_distribution(m,files|{"REBUILD.md":b"wrong"})
        with self.assertRaises(ValueError):verify_distribution(m|{"version":"0.5.1"},files)
    def test_routine_receipt_never_admitted(self):
        q={"status":"WASM_ROUTINE_BROWSER_PASS","revision":self.r["revision"],"version":self.r["version"],"target":"wasm32-unknown-emscripten","offline_vendor_builds":1}
        with self.assertRaisesRegex(ValueError,"WASM receipt identity"):
            verify_receipt(self.r,{"receipt.json":json.dumps(q).encode()},b"",{},b"")
        q["status"]="UNIFIED_WASM_CANDIDATE_PASS"
        q.update(steps={},reproducible_same_environment=False,offline_vendor_replay=False,build_differences={})
        with self.assertRaisesRegex(ValueError,"WASM offline replay or reproducibility missing"):
            verify_receipt(self.r,{"receipt.json":json.dumps(q).encode()},b"",{},b"")
    def test_review_exact(self):
        review={"status":"APPROVED_FOR_REDISTRIBUTION","version":"0.5.0","revision":"a"*40,"candidate_digest":self.r["wasm"]["digest"],"review_reference":"test fixture","scope":"binary-corresponding-source-licenses"}
        raw=json.dumps(review).encode();self.r["wasm"]["review_sha256"]=digest(raw)
        verify_review(self.r,raw)
        for bad in ({"status":"HOLD"},{"revision":"d"*40},{"candidate_digest":"wrong"},{"review_reference":""}):
            raw=json.dumps(review|bad).encode();self.r["wasm"]["review_sha256"]=digest(raw)
            with self.assertRaises(ValueError):verify_review(self.r,raw)
if __name__=="__main__":unittest.main()
