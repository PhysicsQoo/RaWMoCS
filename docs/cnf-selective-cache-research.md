# Selective exact weight-class component-cache admission

## Measured precursor: real node savings vs native overhead

[PR #97's source-identical original QuPRS CNF full-native study](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43987062796)
shows exact signed weighted component memoization saves actual
DPLL search nodes on each completed source case, with matched
100/333-bit complex MPFR semantics:
- HWB6: 81,594 → 73,424 nodes (~10.0% fewer), but full
  100-bit WallTime **510.703 → 518.210ms regression** (GPMC 177.433ms)
  and 333-bit **542.218 → 548.207ms regression**.
- Barenco5: 2,212 → 1,740 nodes (~21.3% fewer),
  100-bit 8.612 → 8.350ms and 333-bit 9.177 → 8.752ms.
- ModMult55: 4,269 → 3,879 nodes (~9.1% fewer),
  100-bit 17.852 → 17.664ms and 333-bit 18.922 → 18.431ms.
- Original Grover5: unmodified GPMC completed; all Rust bounded
  arms, including weight-class cache, were node-censored.

44 Rust tests, strict Clippy and canonical rustfmt passed.
The [full native artifact](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43987062796/artifacts/14285583557)
contains seven rotated fresh process trials, p50/p90/RSS,
SAT, real+imag WMC and incomplete/censored outcomes.
These results do **not** qualify the cache for production
because long-case full WallTime is still slightly worse.

## Isolated next hypothesis

The full weight-class cache spends O(residual-key literal count)
on every node, including large residuals where equivalent
subcomponents are unlikely. Preserve the exact weighted equivalence
relation in the cache but only localize scopes of at most k variables.

The previous all-scope mode `--trail-weight-class-cache` is an
unchanged control. Two independent new research flags extend it:
```
--trail-weight-class-cache --trail-cache-scope-max=16
--trail-weight-class-cache --trail-cache-scope-max=64
```
They require all prior Fused/Prehash/Compact/Parent-ID Trail
prerequisites. Without either max flag, the previous all-scope
implementation and semantics remain unchanged. All flags are
research-only, off by default.

The new key uses exactly the source topology and full
positive/negative complex MPFR weight-class vector under
monotone scope relabeling, but includes a leading **zero
domain-separation tag** in `key.scope`. Since a raw source
scope has only original variable IDs in `1..=n`, the new
tag prevents a normalized key from colliding with a raw
key in the *same* HashMap. Full Hashbrown key equality
is still mandatory on collisions, not hash-only matching.

When the scope length exceeds the bound, use the original
source key (with its original IDs) directly; no per-node
variable relabeling. When it is within the bound, use the
domain-separated signed MPFR class-relabelled key. Never
change source CNF, Boolean SAT status, DPLL pivot order,
weights, numerical precision, unit propagation or component
decomposition.

Track counts of localized and raw keys separately, plus
DPLL nodes and cache hits. This is **not** a theorem that
small components always pay off: the threshold is an empirical
solver-router candidate and must be rejected if the native
cost doesn't improve.

## Reproducible acceptance

- Independent old WMC and original Fused solver agreement on
  synthetic disconnected signed weighted complex components
  at 100/333 bits. Compare bounded cache 16/64 to all-scope
  cache and raw source control. A scope of length greater
  than the threshold **must** use unmodified IDs. Exact
  SAT-and-zero, UNSAT, cache capacity and randomized dyadic
  tests require complete signed complex count equality.
  The stamp-guarded local map and leading zero guarantee
  correct identities even when mixed.
- Eleven-arm seven-fresh-process original CNF study:
  GPMC unchanged at SHA `89bbc89d0de918e7a5385740355df55765039230`,
  original QuPRS emitted source SHA
  `0b28736651b3740ed3b201edd2efdef3da541e0d`,
  the eight earlier Rust/GPMC arms plus full weight-class
  cache, `scoped16` and `scoped64`. Same CNF bytes per
  case, 100/333-bit precision matched to GPMC 30/100 decimals,
  30s/2GiB AS guard, original HWB6/Barenco5/ModMult55 and
  hard Grover5. Report complete full native process
  p50/p90 WallTime, observed peak RSS, SAT and **both**
  Complex WMC components, Cache hits, DPLL nodes,
  source SHA and all censored runs; do not rank incomplete
  source results.
- Old Parent/Compact/Prehash/Fused controls must retain
  exact byte-identical stdout, node/cache/component parity.
  New conditional scopes may alter cached nodes and last
  bits of MPFR rounding; require correct SAT and full
  signed WMC numerical agreement at the same precision,
  including on the original GPMC output.
- Six new original-source CodSpeed Simulation benchmarks
  add `scoped16/scoped64` for each HWB6/Barenco5/ModMult55,
  retaining the previous 33 source-original and 68
  formal-core identities unchanged. Use flamegraphs to
  isolate key-normalization costs, never as a substitute
  for actual process WallTime.

**Status: isolated Draft, stacked on PR #97, not production.
Promotion requires repeatable high-cost source-native WallTime
and RSS benefit at both precision settings. If the gate has
no real gain, record it as a negative conditional-routing
experiment and preserve original Fused as control.**
