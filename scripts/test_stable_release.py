import unittest
from stable_release import BASE_SHA,index_state,intent,validate_request,verify_run,NAMES,STEPS,REPO
from copy import deepcopy
from unittest.mock import patch
from stable_release import verify_exact_draft, preflight
from release_source import digest
class StableReleaseTests(unittest.TestCase):
    def setUp(self):
        self.v="0.4.1";self.sha="a"*64;self.rev="b"*40
        self.base={"vers":"0.4.0","cksum":BASE_SHA,"yanked":False}
        self.r={"version":self.v,"revision":self.rev,"admission_run":123,"notes":"releases/v0.4.1.md","artifacts":{k:{"id":i+1,"digest":"sha256:"+self.sha} for i,k in enumerate(NAMES)},"wasm":{"run":123,"id":3,"digest":"sha256:"+self.sha,"review_sha256":self.sha}}
    def test_registry_new_and_resume(self):
        rows=[self.base,{"vers":"0.4.0-rc.1","cksum":"c"*64,"yanked":False}]
        self.assertEqual(index_state(rows,self.v,self.sha),"READY")
        self.assertEqual(index_state(rows+[{"vers":self.v,"cksum":self.sha,"yanked":False}],self.v,self.sha),"MATCH")
    def test_registry_rejects_conflicts(self):
        for row in ({"vers":self.v,"cksum":"d"*64,"yanked":False},{"vers":self.v,"cksum":self.sha,"yanked":True},self.base):
            with self.assertRaises(ValueError):index_state([self.base,row],self.v,self.sha)
        with self.assertRaises(ValueError):index_state([],self.v,self.sha)
    def test_intent(self):
        intent("workflow_dispatch",{"inputs":{"registry_mode":"stable-trusted"}},self.rev)
        intent("push",{"ref":"refs/heads/main","after":self.rev,"head_commit":{"id":self.rev,"message":"[publish-stable]"}},self.rev)
        for mode in ("stable-preflight","trusted","none"):
            with self.assertRaises(ValueError):intent("workflow_dispatch",{"inputs":{"registry_mode":mode}},self.rev)
        with self.assertRaises(ValueError):intent("push",{"ref":"refs/heads/main","after":self.rev,"head_commit":{"id":self.rev,"message":"ordinary"}},self.rev)
    def test_request(self):
        validate_request(self.r)
        for key,val in (("version","0.4.1-rc.1"),("version","0.4.0"),("revision","main"),("notes","../x"),("admission_run",True),("artifacts",{})):
            r=self.r|{key:val}
            with self.assertRaises(ValueError):validate_request(r)
    def test_admission_identity(self):
        d={"id":123,"head_sha":self.rev,"repository":{"full_name":REPO},"head_repository":{"full_name":REPO},"path":".github/workflows/stable-package-admission.yml","head_branch":"release/v0.4.1","event":"push","status":"completed","conclusion":"success"}
        jobs={"total_count":len(STEPS),"jobs":[{"name":k,"status":"completed","conclusion":"success","steps":[{"name":s,"conclusion":"success"} for s in STEPS[k]]} for k in STEPS]}
        verify_run(self.r,d,jobs)
        bad=deepcopy(jobs);bad["jobs"][-1]["conclusion"]="skipped"
        with self.assertRaises(ValueError):verify_run(self.r,d,bad)
        bad=deepcopy(jobs);bad["jobs"][0]["name"]=NAMES["crate"]
        with self.assertRaises(ValueError):verify_run(self.r,d,bad)
        with self.assertRaises(ValueError):validate_request(self.r|{"wasm":self.r["wasm"]|{"run":124}})
        for k,v in (("head_sha","c"*40),("conclusion","failure"),("head_branch","other"),("event","pull_request")):
            with self.assertRaises(ValueError):verify_run(self.r,d|{k:v},jobs)
        j=deepcopy(jobs);j["jobs"][0]["steps"][0]["conclusion"]="skipped"
        with self.assertRaises(ValueError):verify_run(self.r,d,j)
    def test_exact_draft_rejects_wrong_identity_or_bytes(self):
        assets={"package":b"verified"}
        d={"tag_name":"v"+self.v,"draft":True,"prerelease":False,"name":"RaWMoCS v"+self.v,
           "author":{"login":"github-actions[bot]"},"target_commitish":"main","body":"notes",
           "assets":[{"name":"package","digest":"sha256:"+digest(b"verified"),"state":"uploaded"}]}
        with patch("stable_release.Path.read_text",return_value="notes"):
            verify_exact_draft(d,self.r,assets)
            for bad in ({"author":{"login":"other"}},{"target_commitish":"other"},{"body":"bad"},
                        {"assets":[]},{"prerelease":True},{"draft":False}):
                with self.assertRaises(ValueError):verify_exact_draft(d|bad,self.r,assets)
    def test_registry_ready_forbids_existing_tag_or_release(self):
        with patch("stable_release.checked_state",return_value="READY"), patch("stable_release.tag_commit",return_value=None), patch("stable_release.releases",return_value=[]):
            self.assertEqual(preflight(self.r,{}),"READY")
        with patch("stable_release.checked_state",return_value="READY"), patch("stable_release.tag_commit",return_value=self.rev), patch("stable_release.releases",return_value=[]):
            with self.assertRaises(ValueError):preflight(self.r,{})
if __name__=="__main__":unittest.main()
