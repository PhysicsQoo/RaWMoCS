//! Six stable native benchmark identities; fixtures are built outside timed spans.
use rawmocs_benchmarks::scalability::{fixture as medium_fixture, Family};
use rawmocs_benchmarks::{arithmetic_operands, fixture, Workload};
use rawmocs_core::{solve, solve_with_limits, Gaussian, Limits, Problem};
use std::sync::OnceLock;

static FORCED: OnceLock<Problem> = OnceLock::new();
static CONNECTED: OnceLock<Problem> = OnceLock::new();
static DISJOINT: OnceLock<Problem> = OnceLock::new();
static CACHE: OnceLock<Problem> = OnceLock::new();
static OPERANDS: OnceLock<(Gaussian, Gaussian)> = OnceLock::new();
static IDENTITY: OnceLock<Gaussian> = OnceLock::new();
static ZERO: OnceLock<Gaussian> = OnceLock::new();
static CHAIN16: OnceLock<Problem> = OnceLock::new();
static CHAIN32: OnceLock<Problem> = OnceLock::new();
static CHAIN64: OnceLock<Problem> = OnceLock::new();
static PAIRS16: OnceLock<Problem> = OnceLock::new();
static PAIRS32: OnceLock<Problem> = OnceLock::new();
static PAIRS64: OnceLock<Problem> = OnceLock::new();

fn forced() -> &'static Problem {
    FORCED.get_or_init(|| fixture(Workload::ForcedFree))
}
fn connected() -> &'static Problem {
    CONNECTED.get_or_init(|| fixture(Workload::Connected))
}
fn disjoint() -> &'static Problem {
    DISJOINT.get_or_init(|| fixture(Workload::Disjoint))
}
fn cache() -> &'static Problem {
    CACHE.get_or_init(|| fixture(Workload::Cache))
}
fn operands() -> &'static (Gaussian, Gaussian) {
    OPERANDS.get_or_init(arithmetic_operands)
}
fn identity() -> &'static Gaussian {
    IDENTITY.get_or_init(Gaussian::one)
}
fn zero() -> &'static Gaussian {
    ZERO.get_or_init(Gaussian::zero)
}

fn main() {
    let _ = (forced(), connected(), disjoint(), cache(), operands(), identity(), zero());
    for size in [16, 32, 64] {
        let _ = (chain(size), pair_components(size));
    }
    divan::main();
}

#[divan::bench]
fn complex_rational_multiply() {
    let (a, b) = operands();
    divan::black_box(divan::black_box(a).mul(divan::black_box(b)));
}

#[divan::bench]
fn full_wmc_forced_free() {
    divan::black_box(solve(divan::black_box(forced())).expect("bounded fixture"));
}

#[divan::bench]
fn full_wmc_connected_search() {
    divan::black_box(solve(divan::black_box(connected())).expect("bounded fixture"));
}

#[divan::bench]
fn full_wmc_disjoint_components() {
    divan::black_box(solve(divan::black_box(disjoint())).expect("bounded fixture"));
}

#[divan::bench]
fn full_wmc_cache_enabled() {
    divan::black_box(solve(divan::black_box(cache())).expect("bounded fixture"));
}

#[divan::bench]
fn full_wmc_cache_disabled() {
    divan::black_box(
        solve_with_limits(
            divan::black_box(cache()),
            Limits {
                cache_entries: 0,
                ..Limits::default()
            },
        )
        .expect("bounded fixture"),
    );
}

fn chain(variables: usize) -> &'static Problem {
    let slot = match variables {
        16 => &CHAIN16,
        32 => &CHAIN32,
        64 => &CHAIN64,
        _ => panic!("only fixed predeclared sizes"),
    };
    slot.get_or_init(|| medium_fixture(Family::EqualityChain, variables))
}

fn pair_components(variables: usize) -> &'static Problem {
    let slot = match variables {
        16 => &PAIRS16,
        32 => &PAIRS32,
        64 => &PAIRS64,
        _ => panic!("only fixed predeclared sizes"),
    };
    slot.get_or_init(|| medium_fixture(Family::IndependentPairs, variables))
}

// Additive series: original six benchmark identities are unchanged.
// Source generation and analytical oracle evaluation remain outside measured spans.
#[divan::bench(args = [16_usize, 32_usize, 64_usize])]
fn medium_equivalence_chain(variables: usize) {
    divan::black_box(solve(divan::black_box(chain(variables))).expect("bounded medium chain"));
}

#[divan::bench(args = [16_usize, 32_usize, 64_usize])]
fn medium_independent_components(variables: usize) {
    divan::black_box(
        solve(divan::black_box(pair_components(variables))).expect("bounded medium components"),
    );
}

// Same-runner, same-operand reference: the established four-product formula.
// A separately named benchmark keeps existing benchmark identities unchanged.
fn reference_four_products(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

#[divan::bench]
fn complex_rational_four_product_reference() {
    let (a, b) = operands();
    divan::black_box(reference_four_products(
        divan::black_box(a),
        divan::black_box(b),
    ));
}

// These measured operations intentionally exclude fixture and identity creation.
#[divan::bench]
fn complex_rational_multiply_by_one() {
    let (a, _) = operands();
    divan::black_box(divan::black_box(a).mul(divan::black_box(identity())));
}

#[divan::bench]
fn complex_rational_four_product_by_one_reference() {
    let (a, _) = operands();
    divan::black_box(reference_four_products(
        divan::black_box(a),
        divan::black_box(identity()),
    ));
}

#[divan::bench]
fn complex_rational_multiply_by_zero() {
    let (a, _) = operands();
    divan::black_box(divan::black_box(a).mul(divan::black_box(zero())));
}

#[divan::bench]
fn complex_rational_four_product_by_zero_reference() {
    let (a, _) = operands();
    divan::black_box(reference_four_products(
        divan::black_box(a),
        divan::black_box(zero()),
    ));
}
