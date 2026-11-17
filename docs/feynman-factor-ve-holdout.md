# Independent Feynman holdout for signed-Complex Factor Variable Elimination

Status: research-only independent Draft; no changes to main, public
production Rust solver, QuPRS adapter/default GPMC, original QASM,
or emitted CNF weights.

## Motivation

[Original MQT Factor-VE PoC PR #110](https://github.com/PhysicsQoo/RaWMoCS/pull/110)
completed 14/14 original MQT QASM→pinned QuPRS Weighted CNF
source×precision cohorts, each seven rotated independent native
processes. Same native 100-bit-equivalent QNN4 source took GPMC
14.319ms, original Rust Inline 27.256ms, exact Factor-VE **7.672ms**.
At 333-bit equivalent, GPMC14.996 / Rust Inline28.665 / FactorVE
**8.238ms**. VQE4 likewise benefited. But W-state4, QAOA7 and
the small one-DPLL-node sources Grover4, QWalk3 and QPE-exact8
regressed. Full-process WallTime includes factor planning; this is
a conditional algorithmic improvement, NOT a universal default.

To check out-of-family generality, evaluate the *unchanged*
Factor-VE algorithm on the predeclared independent QuPRS Feynman
source corpus, especially the long HWB6 and censored Grover5 cases.
Do not tune factors, bag width or resource caps on these inputs.

## Exact same-source controls

Pinned QuPRS SHA 0b28736651b3740ed3b201edd2efdef3da541e0d;
pinned original GPMC C++ SHA
89bbc89d0de918e7a5385740355df55765039230.
Use original unmodified source Feynman QASM HWB6, Barenco-Toffoli5,
ModMult55, Grover5, exported with unchanged pinned QuPRS
WmcManager. Verify source QASM exists and original emitted GPMC
Weighted CNF bytes and input SHA. Freeze every weighted CNF once
before native timing, same byte stream/complex literal weights.

Arms: original GPMC C++ mode=1, unchanged Rust Inline Full MPFR,
identical Rust research process with opt-in exact factor variable
elimination. GPMC precision 30/100 decimal digits matches the
pinned Rust MPFR 100/333-bit conversion. Compare Boolean SAT
independently from both full signed Complex WMC components.
GPMC-Rust large-source numeric agreement is *not* an independent
mathematical exact WMC oracle; PR #110's tiny brute-force dyadic
Gaussian/SAT models provide that separate check.

Each of six HWB6/Barenco5/ModMult55×precision cohorts requires
seven order-rotated fresh native processes. One high-cost Grover5
trial per precision is preserved, whether successful or censored.
Child process time limit 30 seconds, 2 GiB virtual address space
(not strict RSS), original same 1M Rust node limit on HWB6/Grover5.
No representation change and no extra budget granted only to FactorVE.

Preserve original CNF SHA/source QASM SHA, every trial's complete
native WallTime, observed peak RSS, Boolean SAT, both signed
Complex WMC fields, stdout digest, factor VE completed/fallback,
max bucket arity, factor entries/products, as well as DPLL
node/cache/component counts if the original solver is used on
fallback. A *capped VE* must have precisely the same original
Rust Trail DPLL/cache/component topology as unchanged Inline,
while its attempted preprocessing work stays inside full WallTime.
Successful VE has no DPLL nodes and must be measured using
factor-product work and elapsed time, never a deceptive
zero-node comparison.

## Go / No-Go

Six source×precision cohorts must qualify with correct SAT and
both signed Complex WMC components. Failure/censoring is kept
as a scientific negative. Grover5 with GPMC completing and
Rust candidates censored remains a hard-case gap; do not
delete it or infer solved status. A cap-induced fallback on
HWB6 means this static factor algorithm did not address it,
regardless of its benefits on small MQT instances.

Only after independent out-of-family wins and no important
regressions should a new Router be considered. In that
subsequent phase, select between existing Inline and exact
FactorVE using source-CNF structure and a count-safe criterion
with a controlled, bounded planning overhead; do not hard-code
MQT or Feynman case names in production. Any user-visible
QuPRS solver default remains GPMC until separately promoted
under source-identical same-precision tests.
