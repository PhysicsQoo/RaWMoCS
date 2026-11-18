# Source-exact scoped DSU for weighted Trail AND/OR component extraction

## Evidence and target

The source-locked five-arm Component-local Factor-VE study
(https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44238130202)
passed SAT and signed-complex WMC checks, but only reduced original HWB6
DPLL nodes from 73424 to 73416 after eight VE attempts and 1300 factor
products. Full 100-bit WallTime remained 614.068 ms versus unchanged GPMC
228.054 ms. More factor probes are not justified.

The existing Trail engine calls its exact DSU independent-component
extractor across many recursive nodes. That original extractor
initializes a DSU indexed by **all original source variables**,
even when a residual scope has only a few still-live variables.
This research tests the narrower allocation / initialization hypothesis
without changing the algebra, source CNF, pivot choices, cache keys,
component ordering or signed-complex numeric operations.

## Exact opt-in algorithm

New `--trail-scoped-component-dsu` research flag (disabled by default):

- Within the original source Trail search and only at the existing
  disconnected-component extraction step, mark the current active
  variables in a reusable source-indexed epoch array and assign each a
  local compact index in the original active variable order.
- Initialize only `active.len()` DSU parents instead of
  `problem.n + 1`. For each unassigned literal of every still-live
  clause, union its current local representative using exactly the
  same source-clause and literal scan order as the old algorithm.
- Translate the representative back to its original source variable ID
  before grouping in the same ordered map. Original component order
  and within-group variable order must not change, so search,
  weight multiplication, exact memo/cache keys, and MPFR evaluation
  order remain identical.
- Detect unexpected active-scope membership and revert to the original
  full indexed DSU, not a guessed component partition.
- No change to `--trail-post-unit-cache`, other independently
  experimental factor/Cutset modes, default RaWMoCS, QuPRS or GPMC.
  Do not let a candidate performance claim depend on different CNF
  bytes or a different full-run precision.
- Report `component_dsu_calls` and actual initialized DSU parent
  `component_dsu_slots`; the counters are not WallTime or
  proof of faster weighted model counting.

## Correctness and native acceptance

1. **Independent checks** against unchanged Trail in 100- and
   333-bit MPFR: distinct signed-complex disconnected CNFs,
   SAT-but-zero-compatible cases, connected graphs, exact cache
   capacities 0, 1 and 128. Additionally, fifty deterministic
   signed dyadic random residual CNFs per precision and two
   cache capacities. Require same Boolean SAT, identical complex
   real and imaginary results, exactly equal DPLL nodes, cache hits,
   decompositions and fully restored reversible trail.
2. **Strict original-source A/B:** Original pinned QuPRS Weighted
   CNF generated once per Feynman case. Same machine and same
   source-locked original GPMC C++ executable, no compiler or
   input substitutions. Six arms: GPMC, original Rust Inline,
   root dense Factor-VE, prior residual-VE, prior component-local
   VE, and new opt-in Scoped DSU. Seven order-rotated fresh
   processes per usual source and 100/333-bit-equivalent precision,
   GPMC 30/100 decimal digits. Keep 30s and 2 GiB per child.
3. **Semantic hard gate:** In every matched trial SAT and both
   signed complex WMC fields agree with original GPMC under
   the established numeric policy. New Scoped DSU must preserve
   exactly original Inline DPLL nodes, exact cache hits,
   decomposition counts and DSU call count. Initialized DSU
   slots must never increase.
4. **Performance gate:** Retain p50/p90 complete process WallTime,
   measured peak RSS, source SHA, all raw trial results,
   Grover5 censored negatives. A reduction in initialized parent
   slots does not prove WallTime improvement; a hard-case
   research GO requires meaningful same-batch end-to-end
   time and RSS improvement versus GPMC *and* no cross-family
   regression. Otherwise keep this as opt-in/no-go.
5. Separate independent original MQT holdout is mandatory before
   any production feature or solver router decision.

This is a falsifiable microarchitecture change and should not be
mistaken for new AND/OR algebra or a new solving breakthrough.
All private tracking identifiers stay outside public repository
code, branch titles, PR descriptions and CI metadata.
