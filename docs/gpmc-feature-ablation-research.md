# Source-identical GPMC C++ algorithm feature ablation

**Status:** Draft research only. Do not change the GPMC C++ source, RaWMoCS solver, QuPRS CNF generator, official production defaults, or input encoding.

## Motivation

The pinned original GPMC C++ source (SHA `89bbc89d0de918e7a5385740355df55765039230`) derives from Glucose/SharpSAT and includes a preprocessor, FlowCutter-assisted tree decomposition and backjumping. Its Config.cc defaults enable equivalent-literal elimination, variable elimination, clause strengthening, tree decomposition and backjumping. The previous seven-process source-identical HWB6 100-bit experiment had GPMC 177.146 ms, RaWMoCS Inline 504.793 ms, and projected Post-Unit 522.222 ms. We need to isolate algorithmic mechanisms before investing further in constant-factor Rust micro-optimizations.

## Original-source isolation

Pin QuPRS `0b28736651b3740ed3b201edd2efdef3da541e0d` and original GPMC above; generate each Weighted CNF once, then feed identical file bytes, signed literal weights and decimal precision to the same GPMC binary in every fresh-process arm. The controls are:

| Arm | Additional flags |
| --- | --- |
| `gpmc_default` | none |
| `no_equivalence` | `-no-pp_ee` |
| `no_variable_elimination` | `-no-pp_ve` |
| `no_clause_strengthening` | `-no-pp_cs` |
| `no_tree_decomposition` | `-no-td` |
| `no_backjump` | `-no-bj` |
| `no_preprocessing` | `-no-pp_ee -no-pp_ve -no-pp_cs` |
| `no_preprocess_no_td` | previous three + `-no-td` |

The no_preprocessing shorthand disables these three documented stages, not every internal simplification. Disabling backjumping does not necessarily disable clause learning. The multi-flag arms test interactions, not standalone causal effects.

## Acceptance

- Original Feynman HWB6, Barenco5, ModMult55 and Grover5 source files; SHA-256 of every emitted CNF and source QASM.
- Original GPMC mode 1 with 30/100 decimal digits. Only compare arms inside the *same* precision and native Linux CI process cohort; 100/333-bit Rust results are a separately established counterpart, not directly measured in this GPMC-only ablation.
- Seven rotated fresh processes per completed source×precision cohort; one recorded costly Grover5 trial; 30-second timeout and 2 GiB address-space limit per execution; full-process p50/p90 WallTime and observed GNU time peak RSS, not hard RSS cap.
- Preserve all successes, failures, censored runs, stderr tails, signed Complex WMC real/imag, Boolean SAT status, stdout digests and source hashes. Completed toggle arms must agree numerically on *both* WMC components and SAT with unchanged GPMC on the same input and decimal precision. Failure to agree is a correctness blocker.
- This controlled GPMC ablation is black-box mechanism evidence; do not infer precise internal function costs, or represent it as a RaWMoCS speedup or independent exact WMC certificate.

## Algorithm routing

If equivalent-literal elimination matters, extend exact signed-weight-preserving substitutions. If variable elimination matters, develop bounded complex-weight-compensated algebraic elimination, never bare SAT-only rewrites. If Tree Decomposition matters, investigate separator-aware decomposition after parsing the same CNF, distinct from previously regressive naive min-fill. If backjumping matters, prove count-safe learned-clause and backjump semantics before implementation. If no toggle is decisive, focus on exact residual canonicalization and component cache cost.

Any future Rust solver improvement must be opt-in, independently tested on SAT, SAT-with-zero-WMC, UNSAT and full signed Complex WMC, and compared against unchanged GPMC with the same CNF, precision and resource policy. No production upgrade from this research alone.
