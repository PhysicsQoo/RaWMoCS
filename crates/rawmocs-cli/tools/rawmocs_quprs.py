"""Opt-in RaWMoCS adapter for the scoped QuPRS Python solver interface.

Compatible with the pinned QuPRS v0.17.3 transport tested by release CI.
No built-in solver override, implicit registration, or fallback. The default
Linux address-space cap is not a physical RSS or kernel OOM classifier.
"""
from contextlib import contextmanager
import math
import os
from pathlib import Path
import shutil

from QuPRS.utils.precision_count import PrintedCount, _number_and_unit
from QuPRS.utils.solver_adapter import (
    SolverAdapter, SolverCapabilities, SolverResultError, registered_solver_adapter,
)

__all__ = ["RawAdapter", "create_adapter", "registered_rawmocs"]
SUPPORTED_DIGITS = (30, 60, 100, 130)

class RawAdapter(SolverAdapter):
    def format_weights(self, real, imag):
        return f"{real} {imag} 0", "1.0 0.0 0"

    @staticmethod
    def parse_protocol(stdout):
        lines = stdout.splitlines()
        if len(lines) != 2 or lines[0] not in ("s SATISFIABLE", "s UNSATISFIABLE"):
            raise SolverResultError("RaWMoCS incomplete or invalid SAT status")
        f = lines[1].split()
        if len(f) != 5 or f[:3] != ["c", "r", "mpfr"] or any("/" in x for x in f[3:]):
            raise SolverResultError("RaWMoCS invalid count record")
        try:
            real, ru = _number_and_unit(f[3])
            imag, iu = _number_and_unit(f[4])
        except ValueError as e:
            raise SolverResultError(str(e)) from e
        sat = lines[0] == "s SATISFIABLE"
        if not sat and (real or imag):
            raise SolverResultError("RaWMoCS UNSAT with nonzero count")
        return sat, PrintedCount(real, imag, ru, iu, "rawmocs-mpfr")

    def parse_output_precise(self, stdout):
        return self.parse_protocol(stdout)[1]

    def parse_output(self, stdout):
        p = self.parse_output_precise(stdout)
        try:
            re, im = float(p.real), float(p.imag)
        except OverflowError as e:
            raise SolverResultError("count outside finite binary64 range") from e
        if not math.isfinite(re) or not math.isfinite(im):
            raise SolverResultError("count outside finite binary64 range")
        return complex(re, im)


def create_adapter(binary, *, digits=60, timeout_ms=None, address_space_bytes=3221225472):
    """Construct a scoped external adapter; registration is a separate action.

    The caller's QuPRS timeout supervises the whole external invocation.
    timeout_ms optionally controls RaWMoCS's internal search deadline.
    Pass address_space_bytes=None only with an appropriate external resource
    supervisor or for an explicitly unbounded diagnostic. The default
    requires Linux prlimit. OOM attribution requires separate kernel evidence.
    """
    if type(digits) is not int or digits not in SUPPORTED_DIGITS:
        raise ValueError("digits must be 30, 60, 100 or 130")
    if timeout_ms is not None and (type(timeout_ms) is not int or not 0 <= timeout_ms <= 86400000):
        raise ValueError("timeout_ms must be an integer in 0..86400000")
    binary = Path(binary).expanduser().resolve()
    args = ("--digits", str(digits))
    if timeout_ms is not None:
        args += ("--timeout-ms", str(timeout_ms))
    executable = binary
    if address_space_bytes is not None:
        if type(address_space_bytes) is not int or address_space_bytes <= 0:
            raise ValueError("address_space_bytes must be a positive integer or None")
        if not os.sys.platform.startswith("linux"):
            raise RuntimeError("default address-space supervision requires Linux")
        prlimit = shutil.which("prlimit")
        if prlimit is None:
            raise RuntimeError("prlimit is required for address-space supervision")
        executable = Path(prlimit).resolve()
        args = (f"--as={address_space_bytes}", "--", str(binary), *args)

    @contextmanager
    def resource():
        yield executable

    return RawAdapter("rawmocs", args, SolverCapabilities("pair"), resource)


@contextmanager
def registered_rawmocs(binary, **options):
    """Register only inside this context. The existing GPMC default is unchanged."""
    with registered_solver_adapter(create_adapter(binary, **options)) as adapter:
        yield adapter
