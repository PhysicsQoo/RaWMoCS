"""Receipt-level negative controls, not solver mutation."""
import copy
import unittest
from native_transport_light import verify_parity, ARMS

class LightParityTests(unittest.TestCase):
    def setUp(self):
        self.rows = {arm: {"status": "PASS", "stdout_sha256": "a" * 64,
                           "plan_signature": [1, 2, 3],
                           "count": {"sat": True, "real": "1", "imag": "0"}}
                     for arm in ARMS}
    def test_valid(self):
        verify_parity(self.rows)
    def test_missing_arm(self):
        del self.rows["text"]
        with self.assertRaisesRegex(ValueError, "incomplete"):
            verify_parity(self.rows)
    def test_extra_arm(self):
        self.rows["extra"] = copy.deepcopy(self.rows["cli"])
        with self.assertRaisesRegex(ValueError, "incomplete"):
            verify_parity(self.rows)
    def test_failed_arm(self):
        self.rows["file"]["status"] = "FAIL"
        with self.assertRaisesRegex(ValueError, "failed"):
            verify_parity(self.rows)
    def test_changed_count(self):
        self.rows["text"]["count"]["real"] = "2"
        with self.assertRaisesRegex(ValueError, "mismatch"):
            verify_parity(self.rows)
    def test_changed_plan(self):
        self.rows["file"]["plan_signature"][0] = 9
        with self.assertRaisesRegex(ValueError, "mismatch"):
            verify_parity(self.rows)
    def test_changed_protocol(self):
        self.rows["file"]["stdout_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "mismatch"):
            verify_parity(self.rows)
if __name__ == "__main__":
    unittest.main()
