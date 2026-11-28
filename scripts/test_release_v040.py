import unittest
from release_v040 import verify_run,REPO,NAMES,STEPS
def fixture():
    r={"admission_run":123,"revision":"a"*40}
    d={"id":123,"head_sha":r["revision"],"head_branch":"main","repository":{"full_name":REPO},"head_repository":{"full_name":REPO},"path":".github/workflows/rust-crate-rehearsal.yml","event":"workflow_dispatch","status":"completed","conclusion":"success"}
    jobs=[]
    for n in NAMES:
        s=[{"name":STEPS[n],"conclusion":"success"}]
        if n=="source-version-admission":s.append({"name":"Test v0.4.0 release admission rejection controls","conclusion":"success"})
        jobs.append({"name":n,"status":"completed","conclusion":"success","steps":s})
    return r,d,{"total_count":len(jobs),"jobs":jobs}
class Gate(unittest.TestCase):
    def test_exact(self):verify_run(*fixture())
    def test_reject_wrong_identity(self):
        for k,v in (("head_sha","b"*40),("head_branch","experiment/rust-crate-distribution"),("event","pull_request"),("event","push"),("conclusion","failure")):
            r,d,j=fixture();d[k]=v
            with self.assertRaises(ValueError):verify_run(r,d,j)
    def test_reject_skipped_and_missing_step(self):
        for mode in ("skipped","step"):
            r,d,j=fixture()
            if mode=="skipped":j["jobs"][0]["conclusion"]="skipped"
            else:j["jobs"][0]["steps"]=[]
            with self.assertRaises(ValueError):verify_run(r,d,j)
if __name__=="__main__":unittest.main()
