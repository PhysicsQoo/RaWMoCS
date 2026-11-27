"""Native full signed-complex WMC, backed by the shared Rust MPFR solver."""
from dataclasses import dataclass
from decimal import Decimal
from importlib.metadata import version
from os import PathLike, fspath
from typing import Any

from . import _native

__version__ = version("rawmocs")
CancellationToken = _native.NativeCancellationToken
__all__ = ["solve", "solve_file", "SolveResult", "SolveError", "MpfrBinary", "CancellationToken"]


class SolveError(Exception):
    """A failed solve has no numerical result."""

    def __init__(self, kind: str, reason: str, diagnostics: dict[str, Any]):
        self.kind = kind
        self.reason = reason
        self.diagnostics = diagnostics
        super().__init__(f"{kind}: {reason}")


@dataclass(frozen=True)
class MpfrBinary:
    """Lossless stored MPFR value, not a certified exact mathematical count."""

    significand: str
    exponent2: int
    precision_bits: int
    negative_zero: bool


@dataclass(frozen=True)
class SolveResult:
    sat: bool
    real: str
    imag: str
    digits: int
    precision_bits: int
    route: str
    solve_ns: int
    real_binary: MpfrBinary
    imag_binary: MpfrBinary
    diagnostics: dict[str, Any]

    def as_decimal(self) -> tuple[Decimal, Decimal]:
        """Convert the CLI-compatible decimal transport without float coercion."""
        return Decimal(self.real), Decimal(self.imag)

    def as_complex(self) -> complex:
        """Explicitly lossy binary64 conversion; not suitable for certification."""
        return complex(float(self.real), float(self.imag))


def _invoke(function, source, digits, mode, timeout_ms, large_general, token):
    if type(digits) is not int or digits not in (30, 60, 100, 130):
        raise SolveError("INVALID_INPUT", "unsupported digits", {})
    if not isinstance(mode, str):
        raise TypeError("mode must be str")
    if timeout_ms is not None and (type(timeout_ms) is not int or not 0 <= timeout_ms <= 86_400_000):
        raise SolveError("INVALID_INPUT", "invalid timeout_ms", {})
    if type(large_general) is not bool:
        raise TypeError("large_general must be bool")
    try:
        data = function(source, digits, mode, timeout_ms, large_general, token)
    except _native.NativeSolveError as exc:
        raise SolveError(*exc.args) from None
    for name in ("real_binary", "imag_binary"):
        data[name] = MpfrBinary(**data[name])
    return SolveResult(**data)


def solve(cnf: str | bytes, *, digits: int = 30, mode: str = "auto",
          timeout_ms: int | None = None, large_general: bool = False,
          token: CancellationToken | None = None) -> SolveResult:
    """Solve UTF-8 weighted CNF from memory; no subprocess."""
    if isinstance(cnf, bytes):
        try:
            cnf = cnf.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise SolveError("INVALID_INPUT", "CNF is not UTF-8", {}) from exc
    if not isinstance(cnf, str):
        raise TypeError("cnf must be str or bytes")
    return _invoke(_native.solve_text, cnf, digits, mode, timeout_ms, large_general, token)


def solve_file(path: str | PathLike[str], *, digits: int = 30, mode: str = "auto",
               timeout_ms: int | None = None, large_general: bool = False,
               token: CancellationToken | None = None) -> SolveResult:
    """Solve an existing weighted CNF path using the Rust file reader."""
    path = fspath(path)
    if not isinstance(path, str):
        raise TypeError("path must be a string or string-valued PathLike")
    return _invoke(_native.solve_file, path, digits, mode, timeout_ms, large_general, token)
