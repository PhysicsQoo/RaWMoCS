import json,unittest
from assemble import verify_metadata,verify_payload,validate_pin,digest,bind_release_footer
from wasm_release import zip_bytes,BINARY
class Contract(unittest.TestCase):
    def test_footer(self):
        html='<a href="__RAWMOCS_RELEASE_URL__">Release</a><a href="THIRD-PARTY-NOTICES.txt">Licenses</a>'
        self.assertIn("/releases/tag/v0.5.0",bind_release_footer(html,"0.5.0"))
        with self.assertRaises(ValueError):bind_release_footer(html,"../bad")
        with self.assertRaises(ValueError):bind_release_footer("<footer/>","0.5.0")
    def test_unconfigured(self):
        with self.assertRaises(ValueError):validate_pin({"status":"UNCONFIGURED"})
    def test_published_exact_release_only(self):
        v="0.5.0";rev="a"*40;binary={"rawmocs.js":b"js","worker.js":b"worker","rawmocs.wasm":b"\x00asm"}
        binary.update({"THIRD-PARTY-NOTICES.txt":b"https://github.com/PhysicsQoo/RaWMoCS/releases/tag/v0.5.0","REBUILD.md":b"rebuild","licenses/rawmocs/LICENSE":b"MIT","licenses/toolchain/inventory.json":b"{}","licenses/toolchain/emscripten/LICENSE":b"MIT","licenses/toolchain/emscripten/system/lib/libc/musl/COPYRIGHT":b"MIT"})
        blob=zip_bytes(binary);h="b"*64;dn=f"rawmocs-{v}-wasm.zip";sn=f"rawmocs-{v}-wasm-source.zip"
        m={"solver":"RaWMoCS","version":v,"revision":rev,"status":"released","distribution_sha256":{n:digest(x) for n,x in binary.items() if n in BINARY},"license_sha256":{n:digest(x) for n,x in binary.items() if n not in BINARY},"distribution_archive":dn,"distribution_zip_sha256":digest(blob),"source_archive":sn,"source_zip_sha256":h,"review_sha256":h,"crate_sha256":h}
        raw=json.dumps(m).encode()
        pin={"version":v,"revision":rev,"release_id":10,"distribution_zip_sha256":digest(blob),"provenance_sha256":digest(raw)}
        hashes={dn:digest(blob),sn:h,"wasm-provenance.json":digest(raw),"wasm-review.json":h,"wasm-SHA256SUMS":h,f"rawmocs-{v}.crate":h}
        release={"id":10,"tag_name":"v"+v,"draft":False,"prerelease":False,"published_at":"now","author":{"login":"github-actions[bot]"},"assets":[{"name":n,"digest":"sha256:"+x,"state":"uploaded"} for n,x in hashes.items()]}
        assets=verify_metadata(pin,release,rev)
        self.assertEqual(verify_payload(pin,assets,raw,blob)[1],binary)
        for bad in ({"draft":True},{"prerelease":True},{"id":11},{"published_at":None},{"assets":[]}):
            with self.assertRaises(ValueError):verify_metadata(pin,release|bad,rev)
        with self.assertRaises(ValueError):verify_metadata(pin,release,"c"*40)
        with self.assertRaises(ValueError):verify_payload(pin,assets,raw,blob+b"wrong")
if __name__=="__main__":unittest.main()
