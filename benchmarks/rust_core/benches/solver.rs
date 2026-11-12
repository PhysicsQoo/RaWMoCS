//! Six stable native benchmark identities; fixtures are built outside timed spans.
use rawmocs_benchmarks::{arithmetic_operands, fixture, Workload};
use rawmocs_core::{solve, solve_with_limits, Gaussian, Limits, Problem};
use std::sync::OnceLock;

static FORCED: OnceLock<Problem> = OnceLock::new();
static CONNECTED: OnceLock<Problem> = OnceLock::new();
static DISJOINT: OnceLock<Problem> = OnceLock::new();
static CACHE: OnceLock<Problem> = OnceLock::new();
static OPERANDS: OnceLock<(Gaussian, Gaussian)> = OnceLock::new();

fn forced() -> &'static Problem { FORCED.get_or_init(|| fixture(Workload::ForcedFree)) }
fn connected() -> &'static Problem { CONNECTED.get_or_init(|| fixture(Workload::Connected)) }
fn disjoint() -> &'static Problem { DISJOINT.get_or_init(|| fixture(Workload::Disjoint)) }
fn cache() -> &'static Problem { CACHE.get_or_init(|| fixture(Workload::Cache)) }
fn operands() -> &'static (Gaussian, Gaussian) { OPERANDS.get_or_init(arithmetic_operands) }

fn main() {
    let _ = (forced(), connected(), disjoint(), cache(), operands());
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
    divan::black_box(solve_with_limits(divan::black_box(cache()),
        Limits { cache_entries: 0, ..Limits::default() }).expect("bounded fixture"));
}
