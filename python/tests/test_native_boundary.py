"""Focused native boundary checks; no large science corpus duplication."""
import subprocess
import sys
import unittest
from fractions import Fraction
from decimal import Decimal

import rawmocs


def stored(value):
    return Fraction(int(value.significand)) * Fraction(2) ** value.exponent2


def nearest_binary(value, bits):
    """Independent nearest-even rounding of a nonzero rational."""
    sign = -1 if value < 0 else 1
    value = abs(value)
    exponent = value.numerator.bit_length() - value.denominator.bit_length()
    if value < Fraction(2) ** exponent:
        exponent -= 1
    unit = Fraction(2) ** (exponent - bits + 1)
    scaled = value / unit
    q, r = divmod(scaled.numerator, scaled.denominator)
    if 2 * r > scaled.denominator or (2 * r == scaled.denominator and q % 2):
        q += 1
    return sign * q * unit


class NativeBoundary(unittest.TestCase):
    def test_non_dyadic_stored_value(self):
        source = "p cnf 1 1\nc t wmc\nc p weight 1 0.1 -0.3 0\n1 0\n"
        for digits, bits in [(30, 100), (60, 200), (100, 333), (130, 432)]:
            for mode in ("general", "auto"):
                with self.subTest(digits=digits, mode=mode):
                    result = rawmocs.solve(source, digits=digits, mode=mode)
                    self.assertTrue(result.sat)
                    for binary, decimal, expected in (
                        (result.real_binary, result.real, Fraction(1, 10)),
                        (result.imag_binary, result.imag, Fraction(-3, 10)),
                    ):
                        self.assertEqual(binary.precision_bits, bits)
                        self.assertEqual(stored(binary), nearest_binary(expected, bits))
                        self.assertEqual(nearest_binary(Fraction(Decimal(decimal)), bits), stored(binary))

    def test_native_stdio_isolation(self):
        script = """
import rawmocs
wide = 'p cnf 10 1\\nc t wmc\\n1 2 3 4 5 6 7 8 9 10 0\\n'
assert rawmocs.solve(wide).diagnostics['fallback'] is not None
assert rawmocs.solve(wide, mode='general', large_general=True).diagnostics['streaming_source'] is not None
try:
    rawmocs.solve(wide, mode='prejoin')
except rawmocs.SolveError as exc:
    assert exc.kind == 'ROUTE_REJECTED'
else:
    raise AssertionError('expected rejection')
"""
        run = subprocess.run([sys.executable, "-c", script], capture_output=True, timeout=20)
        self.assertEqual(run.returncode, 0, run.stderr.decode(errors="replace"))
        self.assertEqual(run.stdout, b"")
        self.assertEqual(run.stderr, b"")

    def test_cross_thread_cancellation_and_gil(self):
        # A bounded subprocess prevents a regression from hanging the test runner.
        # This observes a pending Python call, not an instrumented Rust checkpoint.
        script = """
import rawmocs
import threading
import time
from concurrent.futures import ThreadPoolExecutor
source = 'p cnf 2 500000\\nc t wmc\\n' + '1 2 0\\n' * 500000
token = rawmocs.CancellationToken()
started = threading.Event()
def solve():
    started.set()
    return rawmocs.solve(source, mode='general', large_general=True, token=token)
with ThreadPoolExecutor(max_workers=1) as pool:
    future = pool.submit(solve)
    assert started.wait(2)
    time.sleep(0.005)
    assert not future.done(), 'fixture completed before cancellation'
    token.cancel()
    try:
        future.result(timeout=5)
    except rawmocs.SolveError as exc:
        assert exc.kind == 'CANCELLED', exc.kind
    else:
        raise AssertionError('expected cancellation')
assert rawmocs.solve('p cnf 1 0\\nc t wmc\\n').sat
"""
        run = subprocess.run([sys.executable, "-c", script], capture_output=True, timeout=15)
        self.assertEqual(run.returncode, 0, run.stderr.decode(errors="replace"))
        self.assertEqual(run.stdout, b"")
        self.assertEqual(run.stderr, b"")
