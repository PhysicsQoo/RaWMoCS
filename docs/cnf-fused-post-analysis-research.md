# Fused post-unit frequency and component analysis (research only)

**Status:** opt-in Draft experiment, stacked on the independently measured projected post-unit residual cache. No production Rust core, QuPRS adapter, default pivot, GPMC source, original CNF bytes, or MPFR precision change.

## Measured motivation

The [17-arm same-source native study](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43998613666) uses seven rotated fresh Linux processes for each completed original QuPRS Weighted CNF cohort, with identical emitted CNF bytes, original GPMC C++ (source `89bbc89d0de918e7a5385740355df55765039230`), QuPRS source `0b28736651b3740ed3b201edd2efdef3da541e0d`, and MPFR 30/100 decimal digits versus Rust 100/333 bits.

| Original CNF | Bits | GPMC p50 ms | Inline p50 ms | Old post-unit p50 ms | Projected post-unit p50 ms | Post / projected nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| HWB6 | 100 | 177.146 | 504.793 | 595.270 | 522.222 | 62,312 / 62,312 |
| HWB6 | 333 | 196.115 | 542.106 | 632.791 | 557.839 | 62,312 / 62,312 |
| Barenco5 | 100 | 11.985 | 8.295 | 9.649 | 8.888 | 1,480 / 1,480 |
| ModMult55 | 100 | 12.135 | 17.693 | 22.103 | 19.981 | 3,405 / 3,405 |

All six completed source×precision cohorts have matching SAT and both signed Complex WMC components against original GPMC; the old post-unit and projected controls also have identical stdout SHA, DPLL nodes, cache hits and component decomposition for every completed native trial. Hard Grover5: GPMC completes; every Rust arm remains node-censored. **A 7–12% native reduction versus old post-unit does not establish a win over Inline, nor over GPMC.** Preserve all negative and censored outcomes.

Independent [post-unit CodSpeed Simulation](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afbb2dab7f09d2f51131562) highlighted source clause scanning (canonical compaction and per-node analysis) rather than MPFR multiplication as the leading candidate cost. These Simulation values are diagnostic, not native WallTime ratios.

## Exactly one new experiment

The pre-existing post-unit DPLL counts Boolean-active literal occurrences by scanning every source clause in the canonical `ids` list. It later traverses the same `ids` again to build a union-find DSU for connected components.

Opt-in `--trail-fused-post-analysis` (requiring `--trail-post-unit-projected`, and thus the full prerequisite fused, weighted-class and inline signed complex cache flags) combines these traversals:

1. After the *same* deterministic unit-closure, exact projected cache lookup and cache miss, traverse each live `cid` in the *same order* once.
2. For every unassigned source literal, increment the exact same frequency count **including source clause multiplicity**, and union variable IDs by the same DSU root operation as the independent control.
3. After identifying active variables, form `BTreeMap<root, variables>` in the same active-variable order; retain old `components()` unchanged for the non-opt-in control.
4. Preserve all CNF, signed literal weights, MPFR arithmetic, propagation order, free-variable factors, conflict semantics, branch selection, exact cache key and collision checking, and recursive SAT-vs-zero semantics.

This is a **per-node traversal optimization**. It is not an algorithmic claim about reducing the search tree, a replacement for count-safe clause learning, or a change to the input representation. No improvement is claimed until paired measurements finish.

## Mandatory validation

- Rust science 100/333-bit signed Complex WMC versus independent old solver and prior projected control, covering Boolean UNSAT, SAT-but-zero, disconnected components, unit chains, and nontrivial dyadic weights.
- Cache capacities 0 / 1 / 128 and component class scopes full / 2 / 16; compare SAT, exact real and imaginary MPFR values, DPLL nodes, cache hits, components, post-unit hits/visited/forced, projected key counts, and fully restored trails.
- **18-arm** seven-fresh-process source-identical GPMC comparison under 30s / 2GiB, with 100/333-bit cohorts and preserved Grover5 censor, p50/p90 native WallTime and observed peak RSS. Every completed projected-full versus fused-analysis trial must have **identical complete stdout SHA and all search metrics**. No cross-precision or cross-runner speed ratios.
- Three **new** source-pinned CodSpeed Simulation benchmark identities for HWB6, Barenco5 and ModMult55, preserving all existing identities. Simulation cannot replace native p50/p90.
- Fail closed on any semantic or structural difference; do not merge into production or change QuPRS GPMC default without separate multi-corpus native verification, including MQT holdouts and genuinely long cases.

## Go / No-Go

This hypothesis is worth retaining only if it reduces native full-process WallTime and RSS in representative longer same-CNF instances **without regressing faster Inline controls**, while preserving every correctness and topology gate. A minor Simulation-only win, additional per-node cost, or hard-case node censor does not authorize adoption.
