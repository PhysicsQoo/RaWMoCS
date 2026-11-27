# Python native API — development candidate

The optional PyO3 extension delegates to the same production MPFR library as
the CLI. No Python solver, implicit GPMC substitution, or equivalence verdict.
The 0.3.0a1 package is experimental, not a published PyPI release.

From a clean source checkout with Rust, GMP/MPFR build tools and Python:

```sh
python -m pip install maturin==1.8.3
maturin build --release --interpreter python --compatibility linux
python -m pip install crates/rawmocs-python/target/wheels/*.whl
python -m unittest discover -s python/tests -v
```

The command above targets a local Linux wheel only; the first CI gate uses
an explicit output directory. Dependency locking, sdist completeness,
redistribution/license review and other OS/architecture qualification are
separate admission steps.

```python
import rawmocs
result = rawmocs.solve_file("input.cnf", digits=100, mode="general")
print(result.sat, result.as_decimal(), result.route)
print(result.real_binary, result.diagnostics)
```

`solve` accepts str or UTF-8 bytes; `solve_file` accepts str or PathLike.
Digits 30/60/100/130 mean 100/200/333/432 working MPFR bits. Decimal
transport and lossless stored binary representations are both returned.
Neither is a certified exact mathematical WMC. Conversion to binary64
complex is explicit and lossy. SAT-zero remains distinct from UNSAT.

A solver failure raises SolveError with kind, reason and diagnostics,
never a count. Python argument type errors remain normal Python exceptions.
The native call releases the GIL during parsing and solving. A fresh
CancellationToken can be cancelled from another Python thread; cancellation
is cooperative, not a hard interrupt. Reference mode checks at its boundary.
The in-process binding does not provide hard time, OOM or RSS isolation.
Use a supervised CLI worker when such isolation is required.
No QuPRS default change or solver speedup is claimed.

Initial contract evidence: independent dyadic arithmetic, CLI/text/file
parity, typed failure, SAT-zero, pre-cancellation, concurrent independent
requests and route diagnostics. In-flight Python cancellation, native
stdio isolation, non-dyadic MPFR round-trip and packaging provenance remain
separate acceptance gates until measured.
