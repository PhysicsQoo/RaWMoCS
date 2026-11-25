# First release admission: v0.1.0 candidate

## Scope

The first deliverable is a source-agnostic full signed-complex
weighted CNF executable specialized for QuPRS workloads. Preserve the exact
reference core and the existing scientific baseline. Do not merge the stacked
research branch or promote unrelated experiments, competition heuristics or
diagnostic command modes into this release.

Candidate base: `c747abe904de537d781f23bcf65998a6539ad3ed`.
Factor-prejoin provenance: research commit
`379b47242cbd9b4a1e597afc6377cbaf0ac13bce`, file
`benchmarks/mpfr_research/src/factor_prejoin.rs`, original blob
`9736654ce04bc0709f4787425494d95a5b3f9f39`.
The port preserves the solver/planner/MPFR kernel and tests; removes the
research command dispatcher and unused pair snapshot diagnostic, exposes
the internal decimal parser and solve entrypoints. The new boundary,
fallback and executable need independent release acceptance.

## Admission gates

- [ ] General MPFR DPLL fallback, source-derived chronological baseline,
  independent exact-oracle parity (signed weights, free variables, duplicate
  and tautological clauses, UNSAT and SAT-zero), and a wide valid CNF with
  more than 32 variables completing through auto routing.
- [ ] Structural admission refusal selects general MPFR under a shared
  deadline; internal or numerical failure does not silently reroute.
  Explicit prejoin-only refusal remains a diagnostic failure.
- [ ] Distinct execution failure categories and empty stdout; supervisor
  classifies external timeout, cancellation and memory failure separately.
  Stack/source/resource limits remain explicit, not numeric results.

- [ ] Existing reference workspace and original QuPRS A0 science checks.
- [ ] Release executable tests (all copied kernel tests, signed-complex
  independent reference parity, empty scope, malformed input, bounds).
- [ ] CLI SAT, SAT-zero, UNSAT; typed nonzero failures with empty stdout.
- [ ] Source-locked seven cohorts / six original cases spanning QPE exact
  and inexact, QNN, Grover, W-state, and Feynman GF2. Verify the original
  independent 550-bit interval receipt and source hashes, same working
  precision, prior research result and plan signature. This is transfer
  qualification, not a fresh GPMC performance measurement.
- [ ] Commit the generated dependency lock, rerun locked from clean checkout,
  check formatting and strict lint without changing kernel semantics.
- [ ] Durable reproducibility: regenerate admitted source and independent
  references from pinned upstream sources, rather than rely permanently on
  expiring CI artifacts.
- [ ] QuPRS adapter integration: actual exported CNF, process exit handling,
  numerical guard, SAT/zero/unknown separation, timeout, cancellation and
  resource supervisor. Never convert execution failure to a definitive verdict.
- [ ] Fresh seven-trial order-rotated matched GPMC comparison at the same
  precision on representative QuPRS long cases, with p50/p90/RSS and
  independent real/imag witness. Bounded Simulation/Flamegraph regression
  for the executable; simulation must not be conflated with wall time.
- [ ] Cross-platform scope and packaging declared only for validated targets.
  Initial executable CI is Linux x86-64; portable reference tests do not
  imply MPFR executable portability.
- [ ] Release notes disclose resource budgets and unsupported input semantics,
  review all gates at exact candidate SHA, then merge/tag/release.

No tag or public release is authorized by a partial gate. A successful
prejoin transfer alone does not establish QuPRS adapter compatibility,
complete numerical error certification, or competitive general WMC
performance. Preserve censored and negative evidence.
