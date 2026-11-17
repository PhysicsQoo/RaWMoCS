# Independent exact Factor-VE test on original Feynman source Weighted CNF

Status: **research-only**, bounded original-CNF Feynman cross-family holdout
of the exact signed Complex sum-product Factor-VE implemented in
[MQT Factor-VE PR #110](https://github.com/PhysicsQoo/RaWMoCS/pull/110).
No production RaWMoCS core, QuPRS adapter or GPMC default is modified.

## Why this experiment

The earlier original MQT 14/14 cohort native A/B showed:
- QNN4/100: GPMC 14.319 ms, Rust Inline 27.256 ms,
  exact Factor-VE 7.672 ms; VQE4/100: 12.672/11.297/7.137 ms.
- W-state4, QAOA7, Grover4, QWalk3, QPE-exact8 have **slower**
  Factor-VE than Inline, even when VE successfully completed.
- Neither lower DPLL nodes (Factor-VE reports 0 by design) nor
  low raw product-count alone implies a full-process advantage.

The next inference cannot rely on *only* the seven MQT circuits.
The predeclared independent original Feynman sources are HWB6,
Barenco5, ModMult55 and Feynman Grover5, all emitted by the
unchanged pinned QuPRS WMC encoder from the original QASM.
This explicitly tests the hard cases in which prior Rust DPLL trails
lagged GPMC or exceeded a finite node budget.

## Reproducibility and fair equivalence

- Pin original QuPRS commit
  0b28736651b3740ed3b201edd2efdef3da541e0d and original
  GPMC C++ commit 89bbc89d0de918e7a5385740355df55765039230.
- Generate exactly one original Weighted CNF per QASM before
  solver timing, with original literal weights, and keep the SHA
  of both original source QASM and emitted CNF. Do not substitute
  hand-converted QASM, a different path-sum normal form, SAT-only
  CNF, or a small synthetic fixture for a held-out circuit.
- Three native child arms consume **the same exact input bytes**:
  unchanged GPMC mode=1, unchanged Rust Inline Full exact MPFR
  weighted trail, and Rust Inline Full with only
  --factor-variable-elimination enabled. GPMC decimal precision
  30/100 uses Rust 100/333-bit MPFR per pinned digits-to-bits.
- Each of three completed source×precision main families
  (HWB6, Barenco5, ModMult55) is run with seven fresh,
  order-rotated processes *at each precision*, giving six
  required qualified cohorts. Feynman Grover5 runs an explicit
  costly single trial at each precision, preserving censored
  negative evidence rather than multiplying hopeless cases.
- Each child receives 30 seconds and 2 GiB *address space*.
  GNU-time peak RSS is observed separately; do not call this
  an explicit 2GiB RSS cap.
- SAT is independent of signed Complex WMC. Both real and imag
  full result components must agree numerically with original
  GPMC at the same MPFR effective precision (rational
  complex-scaled difference). This is not a mathematical
  independent full WMC oracle for large formulas; independent
  small exhaustive signed Gaussian/dyadic model oracles are
  tested in the Factor-VE Rust code.
- Preserve all complete and censored process records, exit
  status, source and stdout hash, number of DPLL nodes,
  cache hits, component decompositions, Factor-VE *factor
  product visits*, width, cells, cap reason, factor planning
  time, native full WallTime p50/p90, and peak RSS.
- A partially capped/failed VE attempt must fall back to the
  old exact Rust Trail engine without using partial algebraic
  values. The new attempt's CPU/memory work is nevertheless
  INCLUDED in full native WallTime.

## Decision gate

Correctness and all **six original Feynman main cohorts** must
pass; do not lower the threshold to obscure hard source failures.
An algorithmic win requires material *complete-process* WallTime
reduction against unchanged Inline and preferably unchanged
GPMC in a Feynman family. The presence of a cap/fallback is
not an algorithmic success, even if the fallback finishes.
If Factor-VE has only sparse positive cases, a future
**structural router** must be evaluated on new holdout circuits
without using source names or result-peeking; a router that
runs expensive VE and then falls back does not qualify.
The Grover5 censor remains a first-class regression criterion.

Research-only Draft; production main and QuPRS GPMC remain unchanged.

## Quality gate independent from speed

Upstream PR #110's native MQT job passed while a separate
Strict Clippy science job flagged the previous *primal-centroid*
manual-division guard and the Factor-VE bucket tuple type
complexity. This PR fixes those two style/diagnostic failures
without changing any algorithmic result or loosening Clippy.
Final-head science, format and original-source native runs
are independently required.
