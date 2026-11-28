import json,unittest
from wasm_release import read_zip,zip_bytes,validate_pin,verify_review
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
    def test_review_exact(self):
        review={"status":"APPROVED_FOR_REDISTRIBUTION","version":"0.5.0","revision":"a"*40,"candidate_digest":self.r["wasm"]["digest"],"review_reference":"test fixture","scope":"binary-corresponding-source-licenses"}
        raw=json.dumps(review).encode();self.r["wasm"]["review_sha256"]=digest(raw)
        verify_review(self.r,raw)
        for bad in ({"status":"HOLD"},{"revision":"d"*40},{"candidate_digest":"wrong"},{"review_reference":""}):
            raw=json.dumps(review|bad).encode();self.r["wasm"]["review_sha256"]=digest(raw)
            with self.assertRaises(ValueError):verify_review(self.r,raw)
if __name__=="__main__":unittest.main()
