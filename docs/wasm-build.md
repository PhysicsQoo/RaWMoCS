# Unified Rust and WASM build

The canonical solver is the same staged public Rust package used for the native distribution. `scripts/wasm/build.py <new-work-directory>` stages the current checkout, copies the committed public lock, builds the native CLI, then builds the same solver twice for wasm32-unknown-emscripten. A disposable manifest and lock overlay enables the pinned GMP/MPFR cross-compilation feature without modifying native manifests. There is no independent solver or version.

The existing Rust science workflow runs the WASM candidate job only for pull requests changing the WASM recipe/staging/workflow or explicit manual `wasm` dispatch. Ordinary native solver edits and main pushes do not repeat the double cross-build; release admission must explicitly qualify the exact release source. Chromium checks native parity, independent exact dyadic counts, tiny nonzero weights, failure and lifecycle. The candidate includes source and dependency/license inventory. Reproducibility is within the pinned environment; this is not cross-browser, hard OOM, or legal redistribution certification.

Candidate artifacts are not public releases. Promotion requires exact-source admission, corresponding source/license review and an extension of the existing trusted source-release policy. No retroactive release asset mutation. Both Playgrounds must consume the same versioned released WASM asset once admitted. QuPRS integration and comparative experiments are outside routine RaWMoCS CI.

Historical W1/W1b/W1c probes and W2 consumer work are accessible in Git history/experiment branches; do not copy obsolete probe implementations to main. Old version-specific release evidence and recovery policy remain intact until their actual callers and active release guards are audited.
