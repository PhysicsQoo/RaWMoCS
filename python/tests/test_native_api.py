import os
import subprocess
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from decimal import Decimal
from fractions import Fraction
from pathlib import Path

import rawmocs

SAT_ZERO = "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n"
UNSAT = "p cnf 1 2\nc t wmc\n1 0\n-1 0\n"
SIGNED = "p cnf 1 0\nc t wmc\nc p weight 1 -0.125 0.25 0\nc p weight -1 0.5 -0.75 0\n"
WIDE = "p cnf 10 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n"


def stored(value):
    return Fraction(int(value.significand)) * Fraction(2) ** value.exponent2


class NativeContract(unittest.TestCase):
    def test_precision_routes_and_oracle(self):
        for digits, bits in [(30, 100), (60, 200), (100, 333), (130, 432)]:
            for mode in ["auto", "general", "prejoin", "auto-frontier", "general-separator", "auto-separator", "reference"]:
                with self.subTest(digits=digits, mode=mode):
                    result = rawmocs.solve(SIGNED, digits=digits, mode=mode)
                    self.assertTrue(result.sat)
                    self.assertEqual(result.precision_bits, bits)
                    self.assertEqual(result.as_decimal(), (Decimal("0.375"), Decimal("-0.5")))
                    self.assertEqual(stored(result.real_binary), Fraction(3, 8))
                    self.assertEqual(stored(result.imag_binary), Fraction(-1, 2))

    def test_sat_zero_is_not_unsat(self):
        a, b = rawmocs.solve(SAT_ZERO), rawmocs.solve(UNSAT)
        self.assertTrue(a.sat)
        self.assertFalse(b.sat)
        self.assertEqual(a.as_decimal(), b.as_decimal())

    def test_file_bytes_and_cli(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "signed.cnf"
            path.write_bytes(SIGNED.encode())
            for digits in (30, 60, 100, 130):
                text = rawmocs.solve(SIGNED.encode(), digits=digits, mode="general")
                file = rawmocs.solve_file(path, digits=digits, mode="general")
                self.assertEqual(text.real_binary, file.real_binary)
                self.assertEqual(text.imag_binary, file.imag_binary)
                binary = os.environ.get("RAWMOCS_TEST_CLI")
                if binary:
                    process = subprocess.run([binary, "--digits", str(digits), "--solver", "general", str(path)], capture_output=True, text=True, check=True, timeout=15)
                    self.assertEqual(process.stdout.splitlines()[0], "s SATISFIABLE")
                    self.assertEqual(process.stdout.splitlines()[1].split()[3:], [file.real, file.imag])

    def test_failure_classification(self):
        cases = [(b"\xff", {}, "INVALID_INPUT"), ("bad", {}, "INVALID_INPUT"),
                 (SIGNED, {"digits": 31}, "INVALID_INPUT"),
                 (SIGNED, {"mode": "bad"}, "INVALID_INPUT"),
                 ("p cnf 1 0\nc t pwmc\n", {}, "UNSUPPORTED_INPUT"),
                 ("p cnf 4097 0\nc t wmc\n", {}, "RESOURCE_LIMIT"),
                 (WIDE, {"mode": "prejoin"}, "ROUTE_REJECTED"),
                 (SIGNED, {"timeout_ms": 0}, "TIMEOUT")]
        for source, options, kind in cases:
            with self.subTest(kind=kind), self.assertRaises(rawmocs.SolveError) as ctx:
                rawmocs.solve(source, **options)
            self.assertEqual(ctx.exception.kind, kind)
            self.assertFalse(hasattr(ctx.exception, "real"))

    def test_cancellation_and_reentry(self):
        token = rawmocs.CancellationToken()
        token.cancel()
        self.assertTrue(token.is_cancelled())
        with self.assertRaises(rawmocs.SolveError) as ctx:
            rawmocs.solve(SIGNED, token=token)
        self.assertEqual(ctx.exception.kind, "CANCELLED")
        with ThreadPoolExecutor(max_workers=4) as workers:
            results = list(workers.map(rawmocs.solve, [SIGNED, SAT_ZERO, UNSAT, SIGNED] * 4))
        self.assertEqual([x.sat for x in results], [True, True, False, True] * 4)

    def test_diagnostics(self):
        general = rawmocs.solve(SIGNED, mode="general")
        self.assertIsNotNone(general.diagnostics["general"])
        fallback = rawmocs.solve(WIDE)
        self.assertEqual(fallback.route, "general")
        self.assertIsNotNone(fallback.diagnostics["fallback"])


if __name__ == "__main__":
    unittest.main()
