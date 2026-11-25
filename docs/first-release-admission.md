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

Candidate `3473429bee9069f4e9538f5dd19a39cc696fe9ba` completed
[CodSpeed CI 45591456271](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45591456271):
original QPE8 200-bit one-second preflight and Simulation output/route
parity. [CodSpeed run](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6b06b3733af1412b6f9ef627)
records 85.7 ms GPMC and 37.2 ms RaWMoCS modeled CPU time; both have
system-call-dominance warnings, so neither is a wall-time estimate.
Flamegraphs inspected: RaWMoCS factor-prejoin solve 33.5 ms inclusive
(90.07% of modeled CPU time); rational multiply 17.3 ms inclusive
(46.42%); GPMC countModels 54.7 ms inclusive (63.75%). No kernel change
from this short-proxy diagnosis. The exact-head native repeat remains
separate from the completed native evidence below.

Candidate `a016e606b62dd3b6bfadb1c131f9cea06c8d3808` completed
[science CI 45588469064](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45588469064):
11/11 jobs, 21/21 complete witnessed native pairs with RaWMoCS faster
in all pairs. GPMC/RaWMoCS p50 seconds: QPE inexact 22 (432 bits)
79.659689560/3.761025841 (21.18x), Grover 6 (432 bits)
25.043931268/1.328089750 (18.86x), GF2^7 (333 bits)
62.124545559/2.382324483 (26.08x). These are case-specific results,
not a general CNF performance claim. New short Simulation gate uses
original QPE8 only (200 bits), one-second native preflight per arm,
five-minute Simulation step and independent witness/output/plan parity.
Flamegraph inspection and packaging remain required.

Candidate `9c82851baf07154953745b6ce3b2bd9efbe25036` passed
[science CI 45586377302](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45586377302):
8/8 jobs, 17 CLI tests, kernel OOM delta 0 to 1 with non-OOM SIGKILL
negative control, six byte-identical regenerated source CNFs and seven
independent interval witnesses. The expiring historical transfer job is
retired from mandatory CI; retain its script and prior receipts for audit.

Prior candidate `2123f1327a5be8e5fe1904210621fe75fbb6d945` passed
[science CI 45585168839](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45585168839):
7/7 jobs, 17 CLI tests, deep 600-variable case with 599 observed frames,
input-open external timeout/cancellation controls, 7 transfer cohorts and
scoped QuPRS integration. No kernel OOM evidence from that candidate.

Prior candidate `36ed2a568ef21b15657e8789e94bebbea44bdfe2` passed
[science CI 45584033902](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45584033902):
7/7 jobs, 16 CLI tests, 7 source transfer cohorts, 3 scoped QuPRS
Python encoder cases and 10 controls. Changes below require fresh exact-HEAD
acceptance; historical PASS is not a substitute.

- [ ] General MPFR DPLL fallback, source-derived chronological baseline,
  independent exact-oracle parity (signed weights, free variables, duplicate
  and tautological clauses, UNSAT and SAT-zero), and a wide valid CNF with
  more than 32 variables completing through auto routing.
- [ ] Structural admission refusal selects general MPFR under a shared
  deadline; internal or numerical failure does not silently reroute.
  Explicit prejoin-only refusal remains a diagnostic failure.
- [ ] Distinct execution failure categories and empty stdout; supervisor
  classifies external timeout, cancellation and memory failure separately.
  Source/resource limits remain explicit, not numeric results.
- [ ] Heap-resident search continuations: preserve positive/negative and
  component aggregation order, check independent exact-oracle corpus at
  four precisions, and complete a 600-variable weighted wide-clause instance
  with observed peak above 512 frames and analytical count one.
- [ ] External lifecycle: real executable blocked on source input,
  supervisor deadline and explicit cancellation, no numeric output.
  This is not active-search cancellation latency or kernel OOM evidence.
- [ ] Configurable shared solver deadline: zero-budget actual CLI failure,
  positive-budget output parity, and analytical larger-CNF family checks
  (component split, signed cancellation, imaginary-only count, UNSAT).

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
  expiring CI artifacts. QuPRS revision `0b28736651b3740ed3b201edd2efdef3da541e0d`;
  byte-identical research bridge and two independent oracle files under
  `crates/rawmocs-cli/tools/repro/` from `379b47242cbd9b4a1e597afc6377cbaf0ac13bce`.
  Verify source Git blobs, exact CNF SHA256, and seven numeric witnesses;
  historical transfer gate passed before retirement.
- [ ] Kernel OOM admission on isolated Linux cgroup v2 with read-back
  memory.max=48 MiB, zero swap and memory.events.local oom_kill delta.
  Require actual solver SIGKILL and empty stdout; negative externally
  SIGKILLed real solver must not increment oom_kill. No claim for other OS.
- [ ] QuPRS adapter integration: actual exported CNF, process exit handling,
  numerical guard, SAT/zero/unknown separation, timeout, cancellation and
  resource supervisor. Never convert execution failure to a definitive verdict.
  Distribute the opt-in Python adapter as reusable rawmocs_quprs.py, exercise
  the actual imported module at all four precisions and zero internal budget,
  and verify registry restoration without replacing default GPMC. QuPRS's
  PrintedCount does not retain SAT; the explicit protocol parser does.
  Native QuPRS default integration/Rust FFI are outside v0.1.0 scope.
- [ ] Fresh seven-trial order-rotated matched GPMC comparison at the same
  precision on representative QuPRS long cases, with p50/p90/RSS and
  independent real/imag witness. Bounded Simulation/Flamegraph regression
  for the executable; simulation must not be conflated with wall time.
  Initial native matrix: QPE inexact 22 (432 bits), Grover 6 (432 bits),
  GF2^7 (333 bits). Same-run regenerated CNF and independent reference,
  GPMC revision 89bbc89d0de918e7a5385740355df55765039230, 120s and 3 GiB
  per arm. Preserve censored rows, publish ratios only for seven complete
  witnessed pairs. No speed floor inferred from past research binaries.
- [ ] Cross-platform scope and packaging declared only for validated targets.
  Initial executable CI is Linux x86-64; portable reference tests do not
  imply MPFR executable portability. Initial packaging is source-only:
  deterministic Git archive, SHA256SUMS, clean extracted-tree locked
  cargo install and installed executable smoke/negative controls on Linux
  x86-64. No redistributed executable or offline build claim; linked-library
  license review and other platform executable CI are separate admission.
- [ ] Release notes disclose resource budgets and unsupported input semantics,
  review all gates at exact candidate SHA, then merge/tag/release.

No tag or public release is authorized by a partial gate. A successful
prejoin transfer alone does not establish QuPRS adapter compatibility,
complete numerical error certification, or competitive general WMC
performance. Preserve censored and negative evidence.
