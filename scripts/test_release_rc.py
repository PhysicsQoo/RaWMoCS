import unittest,os
from unittest.mock import patch
from release_rc import replace_version,check_rc_package,require_manual_main,V
class RCControls(unittest.TestCase):
    def test_version_only(self):
        raw=b'name = "rawmocs"\nversion = "0.4.0"\n'
        self.assertEqual(replace_version(raw,"rawmocs"),b'name = "rawmocs"\nversion = "0.4.0-rc.1"\n')
        for bad in (raw.replace(b'rawmocs',b'rawmocs-cli'),raw.replace(b'0.4.0',b'0.3.0'),raw+raw):
            with self.assertRaises(ValueError):replace_version(bad,"rawmocs")
    def test_no_nonmain(self):
        for env in ({"GITHUB_REPOSITORY":"PhysicsQoo/RaWMoCS","GITHUB_REF":"refs/heads/main","GITHUB_EVENT_NAME":"push"},{"GITHUB_REPOSITORY":"PhysicsQoo/RaWMoCS","GITHUB_REF":"refs/heads/release/rc-bootstrap","GITHUB_EVENT_NAME":"workflow_dispatch"}):
            with patch.dict(os.environ,env,clear=True),self.assertRaises(ValueError):require_manual_main()
if __name__=="__main__":unittest.main()
