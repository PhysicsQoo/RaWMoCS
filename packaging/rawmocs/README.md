# RaWMoCS

Full signed-complex weighted model counting via the Rust library and the
`rawmocs` command line program. Both use the same native MPFR solver.
Finite-precision output is not a certified error interval.

```rust
use rawmocs::{solve_text, SolveOptions};
let count = solve_text("p cnf 1 1\nc t wmc\n1 0\n", SolveOptions::default())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The portable reference core is an internal implementation detail. No separately
published RaWMoCS dependency is required. Native GMP/MPFR builds require a
supported Rust toolchain and native build dependencies.

Source: https://github.com/PhysicsQoo/RaWMoCS
