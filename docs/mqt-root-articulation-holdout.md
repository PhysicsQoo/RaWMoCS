# MQT Bench original Weighted CNF: root-only articulation holdout

Status: research-only independent Draft; no production solver, QuPRS
adapter, GPMC default or Rust main change. This is a comparison on
the **same source-locked QuPRS-emitted Weighted CNF bytes**. It does not
claim the MQT Bench project published these CNF files. Source
QASM→Path-sum→CNF generation is outside all native solver timing.

## Prior evidence and testable hypothesis

[Prior original-source PR #105](https://github.com/PhysicsQoo/RaWMoCS/pull/105)
showed a full-depth graph-cut heuristic harmed ModMult55 (DPLL nodes
3,879→6,985), whereas restricting to one root cut reduced nodes
to 3,593 and native complete WallTime by about 10% at both MPFR
precisions. HWB6 and Barenco5 did not pick a root; Feynman Grover5
still failed the Rust node budget. We need independent holdout cases.

Freeze seven previously committed original MQT QASM sources from pinned
QuPRS MQTBench: W-state4, VQE4, QNN4, Grover4, QWalk3, QAOA7,
QPE-exact8. Generate each CNF once via the unchanged pinned QuPRS
WmcManager, using the existing source-locked MQT reference exporter in
a temporary CI checkout. Check source-byte identity, both emitted
dialects and phase sidecars before accepting input.

Earlier, independent original-MQT 100-bit native GPMC / Rust Scratch
results (different CI run, only baseline motivation): W-state4
13.100/9.391 ms; VQE4 13.746/19.358 ms; QNN4 17.220/66.677 ms;
Grover4 5.296/20.002 ms; QWalk3 4.880/4.223 ms; QAOA7
14.336/14.971 ms; QPE-exact8 4.951/5.572 ms. These do NOT
constitute a new root-pivot measurement. QNN4 and Grover4 are
high-gap cases that must not be excluded.

## Exact controls and evidence contract

- Pinned original QuPRS commit
  0b28736651b3740ed3b201edd2efdef3da541e0d; unchanged original
  GPMC C++ commit 89bbc89d0de918e7a5385740355df55765039230.
  No modifications to original CNF, literal weights, or source QASM.
- Three arms: original unchanged GPMC mode=1, original Rust MPFR Inline
  Full, and identical Rust Inline Full with root-only articulation
  (one graph probe allowed at DPLL depth zero). Each Rust arm has
  the same 1,000,000-node budget, 50,000-entry component cache,
  identical source-specific flags, and identical source CNF bytes.
- Native Linux runner: GPMC 30/100 decimal digits, Rust MPFR
  100/333 bits under the pinned conversion; 30s and 2GiB
  address-space limits PER independent child; observed GNU-time
  peak RSS rather than hard RSS cap.
- Seven independent order-rotated fresh processes per completed
  case and precision; first trial censored if any arm fails or
  exceeds 10s to avoid multiplied expensive failures. Store every
  resulting positive, negative and censored record.
- Independently check Boolean SAT from signed Complex WMC real and
  imaginary values using a rational relative-scale witness. Save
  both component deltas, source and CNF SHA, stdout digest,
  DPLL nodes, cache hits, decomposition count, root pivot picked,
  full WallTime p50/p90 and RSS. Large-case numeric agreement
  is NOT a mathematically independent exact Full WMC certificate.
- Root separator probes and selections must each be at most one.
  If no root pivot is selected, search node, cache-hit and component
  decomposition counts must equal the unchanged Rust Inline arm.
- Strict gate: all 14 source×precision cohorts qualify; otherwise
  preserve the failures, fail CI and do not claim global speedup.

## Go/No-Go

Promote neither pivot nor default on one favorable Feynman case.
Require out-of-family source improvements in both complete-process
native WallTime and DPLL nodes without hard-family regressions,
particularly on QNN4/Grover4 and previously censored Feynman Grover5.
If only a minority has actual root cuts or full time regresses,
retain a negative holdout and prioritize bounded comparative cofactor
cost or exact complex-weight-preserving preprocessing. Production
Rust main and QuPRS solver default stay unchanged.
