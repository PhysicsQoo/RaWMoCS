# Matched-MPFR bounded residual Cache capacity on original Feynman CNFs

## Scope and reproducibility

This is an **opt-in, research-only** study. Production `rawmocs-core`,
the published GPMC C++ solver and the QuPRS SolverAdapter/default remain
unchanged. The MPFR research executable retains its original **10,000-entry
Cache limit** unless an explicit finite `--cache-limit=25000`,
`--cache-limit=50000` or `--cache-limit=100000` argument is provided.

- Original QuPRS exporter source SHA:
  `0b28736651b3740ed3b201edd2efdef3da541e0d`
- Original unchanged GPMC C++ source SHA:
  `89bbc89d0de918e7a5385740355df55765039230`
- [Qualified original Feynman GitHub Actions run #43675631633](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43675631633)
- Full raw artifact: `original-feynman-long-source-scout`,
  including `cache-capacity-ab.json`, original QASM/CNF/phase digests,
  individual native samples and censored status
- Same original QuPRS PathSum/WmcManager-emitted weighted CNF bytes,
  source-locked input SHA, GPMC/Rust **100 or 333 binary MPFR bits**,
  individually rounded complex four-product arithmetic (no FMA)
- Unchanged chronological DPLL, indexed unit propagation after 8192
  visited nodes, fast binary assignment, borrowed full-equality
  `hashbrown` Cache and shared residual storage
- Each of the five arms was run in five rotating, independent native
  processes per original source and precision, on the **same Ubuntu
  runner**, with 30-second and 2-GiB address-space bounds.
- Hardware/process WallTime includes startup, parsing, search, MPFR and
  printing. Peak RSS is from native `/usr/bin/time`.

The comparison allows **DPLL nodes, cache hits and component splits to
change**, since additional exact Cache entries can legitimately avoid
repeated work. Each arm's structural counters must nonetheless be
deterministic across its trials. All Rust arms must have **byte-identical
native stdout** and the same Boolean SAT outcome, and each must agree
numerically with the unchanged GPMC solver at its matched precision.
This is not an independent exhaustive complex Full WMC oracle for these
large sources, and does not establish original quantum phase fidelity.

## Qualified same-runner native medians

All six source × MPFR-width groups passed the scientific admission.
The isolated MPFR Rust suite reported **16 tests passed**.

| Original source | Bits | GPMC C++ ms | Rust 10k ms | Rust 25k ms | Rust 50k ms | Rust 100k ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| HWB6, 297 vars / 827 clauses | 100 | 226.706 | 1451.774 | 1216.305 | **1214.330** | 1216.034 |
| HWB6 | 333 | 253.055 | 1559.552 | 1275.761 | **1157.446** | 1157.418 |
| Barenco Toffoli5, 97 vars | 100 | 15.678 | 26.001 | 24.659 | 24.669 | 24.613 |
| Barenco Toffoli5 | 333 | 15.831 | 26.086 | 25.935 | 26.049 | 26.121 |
| Modular Multiplier55, 91 vars | 100 | 15.403 | 34.852 | 34.982 | 34.906 | 34.982 |
| Modular Multiplier55 | 333 | 16.008 | 37.143 | 36.919 | 36.625 | 36.825 |

**Original HWB6** uses a fixed finite 1,000,000-DPLL-node cap.
The 10k mode visits **273,928** nodes at either precision; the 25k
mode visits **151,245**; both 50k and 100k visit **94,209**.
This is a **65.6% node reduction** with 50k admission, reflecting
fewer searches on previously unseen residuals, not a numerical shortcut.

Relative to the original indexed-10k Rust mode, HWB6 50k lowers
median native WallTime by **16.4% at 100 bits** and
**25.8% at 333 bits**. The 25k mode lowers it by
**16.2%** and **18.2%**, respectively.
The unchanged GPMC still leads the Rust 50k mode by
approximately **5.36× at 100 bits** and **4.57× at 333 bits**.
These are five-process single-runner medians, not a multi-CPU
statistical significance claim.

### Memory cost and Pareto frontier

| Original HWB6 | GPMC RSS KiB | Rust 10k RSS KiB | Rust 25k RSS KiB | Rust 50k RSS KiB | Rust 100k RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 bits | 18312 | 71816 | 158844 | **227352** | 227344 |
| 333 bits | 19328 | 72564 | 160552 | **230108** | 230076 |

Increasing Cache to 25k costs approximately **2.2×** the original
Rust peak RSS; 50k costs approximately **3.2×**. The 100k mode
does not reduce nodes or WallTime beyond 50k and has effectively
the same RSS, indicating a plateau on this source, **not** proof
that the same plateau holds for other CNFs.

Barenco and ModMult nodes are unchanged across quotas
(4,325 and 5,699 respectively); their submillisecond timing
differences must not be marketed as stable speedups.

## Decision and next question

Retain the finite 25k/50k/100k CLI options **research-only**.
Keep the default at 10k until reproducible cross-corpus evidence
supports a better memory/time policy. On this original HWB6 cohort,
25k and 50k are useful **Pareto candidates**: 25k conserves roughly
68 MiB of memory compared with 50k at 100 bits but is substantially
slower at 333 bits; 50k minimizes the tested long-case WallTime
without any gain from raising the limit to 100k.

Next investigate **cache admission selectivity and residual payload
allocation**, not simply larger quotas: reduce the ~150 MiB RSS
increase while preserving HWB6's 94k-node trajectory and identical
complex WMC. Require further original QuPRS and non-QuPRS long
holdouts, 100/333-bit numerical parity, native WallTime/RSS and all
censored/UNKNOWN outcomes. The original Grover5 source remains
**censored at 1M nodes** with no valid time rank.
