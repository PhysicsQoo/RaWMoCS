# WMC-safe equivalent-literal elimination from original Weighted CNF

**Status: research-only, opt-in; formal production routing not changed.**

## Input and algebraic proof

This branch uses only the **original Weighted CNF bytes**, as emitted
by QuPRS and consumed unchanged by original GPMC. QuPRS owns QASM,
Path-sum, RR and CNF encoding; RaWMoCS does not import alternative
native ANF or perform any circuit preprocessing.

The research Rust MPFR option `--weighted-equivalence` constructs the
binary implication graph from the original CNF: each (a OR b)
contributes (!a => b) and (!b => a). Strongly connected literals imply
exact logical equivalence. Two literals with the same SCC must have
identical truth values in **every satisfying assignment**. When a
variable and its negation are in one SCC, the source formula is
Boolean UNSAT (not merely a complex WMC cancellation).

For each SCC pair representing a signed-equivalence class, retain its
smallest original variable r. Substitute every member v as either r or
!r, as certified by SCC orientation. Delete only Boolean-tautological
or duplicate clauses **after this equivalence**, and exclude eliminated
members from the solver's active variable scope.

The critical WMC correction: if class K has members with signs s_v,
set the representative's true/false complex literal weights to

    W_r(true)  = product over v in K of W_v(truth(v)|r=true)
    W_r(false) = product over v in K of W_v(truth(v)|r=false)

using the same MPFR precision and four-separated-products complex
arithmetic as the research GPMC-parity solver. The removed variables
are *not* multiplied as independent free variables again. Each
complete original satisfying assignment maps bijectively to exactly
one reduced model and has the same product of signed complex weights.
This covers nonuniform, negative, zero and complex weights; no
division, normalization-by-zero or SAT-only existential rewrite.

Graph SCC uses iterative Kosaraju. Caps: at most 4096 variables,
50000 CNF clauses, 150000 implication edges. On admission failure
return the **original full CNF, original weights and original scope**;
the existing original Frequency / Cache DPLL remains unchanged unless
the research flag is given. The optional preprocessing is charged
inside native solver WallTime; no hidden QASM/RR cost is assigned to
the RaWMoCS algorithm.

## Independent science

- Small complete model-set and signed complex weighted truth-table
  oracle at 100/333 MPFR bits, mixed regular/nonregular CNF and
  variable-equivalence chains including negated relationships.
- SAT with mathematically cancelled 0+0i WMC remains SAT, including
  eliminated variable compensation.
- SCC contradiction produces Boolean UNSAT with zero WMC;
  negative and resource-cap controls return unchanged original.
- Other independent baseline MPFR and rust-core science / CodSpeed
  identifiers stay unchanged.

## Seven-trial original same-Weighted-CNF paired evidence (2026-11-16)

[Completed GPMC / Rust native matched-CNF run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43829375234)
**SUCCESS**. Raw 7-trial model, source CNF hashes, observed RSS,
Full Complex WMC real/imaginary components, SAT statuses, search/cache
statistics and negative records are archived in GitHub Actions
artifact **14208196818**, named
original-quprs-weighted-cnf-equivalence-gpmc-three-arm.

Original unmodified QuPRS source SHA:
0b28736651b3740ed3b201edd2efdef3da541e0d.
Original unmodified GPMC C++ source SHA:
89bbc89d0de918e7a5385740355df55765039230.
Every arm consumes the identical QuPRS-generated Weighted CNF bytes
for each circuit: no native ANF, differing Tseitin encoding, or
QASM-level changes in the Rust solver. GPMC -mode=1 -prec=30/100
uses the corresponding original C++ MPFR 100/333-bit contract;
Rust both arms use 100/333-bit MPFR with the original four separately
rounded complex products. QASM parse, Path-sum and the fixed source
fixture exporter run **outside native solver subprocess WallTime**.

All six completed source/precision cohorts (GPMC, baseline Rust,
Rust + equivalent literal elimination; 7 fresh rotated trials
per arm) passed source-matched SAT and both Full Complex WMC component
numerical agreement at matched precision. Large circuit cases have
**cross-solver numerical agreement**, not independent exhaustive
mathematical WMC certification; independent exhaustive small-CNF
proof tests cover the model-bijective transformation.

| Original QuPRS Weighted CNF | MPFR bits | Original GPMC p50 wall ms | Rust baseline p50 ms | Rust equivalent-literals p50 ms | DPLL nodes baseline to equivalent | p50 observed Rust RSS KiB baseline to equivalent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| HWB6 | 100 | 176.441 | 735.985 | **605.775** | 94,209 to **81,594** | 71,476 to **60,360** |
| HWB6 | 333 | 194.317 | 787.223 | **647.537** | 94,209 to **81,594** | 74,240 to **62,760** |
| Barenco5 | 100 | 11.272 | 18.366 | **8.498** | 4,325 to **2,212** | 5,328 to **4,124** |
| Barenco5 | 333 | 10.904 | 19.337 | **8.914** | 4,325 to **2,212** | 5,524 to **4,236** |
| ModMult55 | 100 | 10.633 | 25.864 | **19.782** | 5,699 to **4,269** | 5,872 to **5,388** |
| ModMult55 | 333 | 11.537 | 27.785 | **21.177** | 5,699 to **4,269** | 6,004 to **5,568** |

**Direct Rust control/opt-in comparisons within the same CI**
show approximately **1.21–1.22x** speedup for HWB6,
**2.16–2.17x** for Barenco5, and **1.30–1.31x** for ModMult55,
all inclusive of weighted SCC detection, complex weight folding,
source CNF parse, original DPLL, output and native child startup.
The reduced DPLL nodes are respectively 13.4%, 48.9% and 25.1% fewer.
Barenco5 is the only completed source where opt-in Rust also beats
unchanged GPMC on the same runner at both precision settings:
8.498 vs 11.272 ms (100-bit) and 8.914 vs 10.904 ms (333-bit).
**This single source does not qualify general Rust solver superiority.**

**Hard source CENSORED**: original Feynman Grover5 weighted CNF
contains 607 source variables and 1,121 clauses; equivalence
SCC extraction eliminates **33 variables and 66 clauses**.
GPMC completed its single 100-bit pilot in 21.980 s
(peak observed RSS 608,928 KiB).
Both baseline and equivalence-enabled Rust exhausted the
original 1-million-DPLL-node budget and returned no complete
Full WMC result. This is **not** a successful Rust run and
cannot be included in ratio rankings; 333-bit Grover5 is
not checked after the 100-bit censored result.

The source-matched input CNF hashes are:
- HWB6: e0381c270963e0715273530e48f159dc05b1cd96c9b85a556741a72a4289471d
- Barenco5: f1fb459a058ba3dc17d2bd538a7e8b34e451383ae3384cb58a624bc8194499c3
- ModMult55: f8c76d883923363a7fcb5e32d6c6d8914f8443af3582fbe3d2e1d9c0b99c35b7
- Grover5: eeb8e82273b1e9dc105e004d73b3e59f1d48bb893fe768bb4034262224a780b0

The GNU-time RSS is an **observed peak**, not a hard memory
control; 2 GiB is the original *virtual address-space* limit.
The native Rust research process has a 50k residual-cache entry
cap; this is not equivalent to GPMC's internal MB cache.
Only seven one-runner trials per completed cohort were observed.
The first three-trial feasibility run
https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43828332888
also passed, with matching search-node counts, but its timings
must not be combined with the seven-trial cohort to inflate n.

### Interpretation and future direction

The gain occurs because source CNF can actually **remove model-bijective
branch variables**, not merely add implied clauses or change a
source QASM representation. By itself this does NOT bridge the
HWB6 gap (GPMC 176.4 ms vs optimized Rust 605.8 ms)
or solve Grover5 (both Rust arms reached the node cap).
Do **not** promote the opt-in policy to production by default.
Next major algorithmic questions are source-CNF-native
separator-aware component decomposition, incremental XOR propagation,
and WMC-safe bounded variable elimination with correct complex
weight/factor compensation. Priority is reducing total retained
residual subproblems, not yet another pivot score or transient
allocation microbenchmark.

## Same-CNF experiment

Workflow: `.github/workflows/cnf-equivalence-paired.yml`.
Runner: `benchmarks/rust_core/tools/quprs_cnf_equivalence_ab.py`.

Pinned QuPRS original CNF corpus from
`0b28736651b3740ed3b201edd2efdef3da541e0d`
(HWB6, Barenco5, ModMult55, Grover5), unchanged GPMC C++ at
`89bbc89d0de918e7a5385740355df55765039230`.
All three solver arms receive identical per-case Weighted CNF bytes:
GPMC C++, Rust Frequency+MPFR control, and Rust Frequency+MPFR
equivalence elimination. Matched 100/333-bit precision; three
order-rotated fresh subprocesses on the same Ubuntu runner for
completed cohorts, 30s and 2GiB virtual address-space guard;
collect full process WallTime, p50/p90, RSS, DPLL/cache/component,
source hashes, SAT and both Full Complex WMC components. Grover5
gets one bounded all-arm pilot at 100 bits even when the Rust baseline
is censored, to avoid suppressing a potential completed candidate.

**Admission decision depends on results:** a smaller CNF after SCC
substitution does not automatically imply faster solver WallTime or
better numerical conditioning. Preserve negative cohorts and
CENSORED node/time/memory outcomes; do not use partial failed
times in speed ratios or change production defaults.