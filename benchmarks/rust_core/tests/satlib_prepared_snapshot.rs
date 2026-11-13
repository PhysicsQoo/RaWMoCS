#![cfg(feature = "canonical-mul-research")]
//! QOO-220: the prepared object owns a canonical snapshot of the ORIGINAL
//! SATLIB Boolean CNF and the ORIGINAL weights. It must never be reused as
//! though it represented a different set of literal weights.
//! The reference oracle directly sums pinned SAT assignments with four
//! scalar products; it does not use DPLL, the cache or Gaussian::mul.
use num_rational::BigRational;
use rawmocs_benchmarks::satlib_provenance::{fixture, independent_full_wmc};
use rawmocs_core::{
    solve_with_limits, CanonicalMulResearchError, Gaussian, Limits,
    PreparedCanonicalProblem, Problem, Weights,
};

fn raw(n: i64, d: i64) -> BigRational {
    BigRational::new_raw(n.into(), d.into())
}

fn with_replaced_first_weight(original: &Problem, weight: Weights) -> Problem {
    let mut w = original.weights().to_vec();
    w[0] = weight;
    Problem::new(original.variables(), original.clauses().to_vec(), w)
        .expect("same pinned SATLIB CNF, new independent weight assignment")
}

fn canonical_fields(g: &Gaussian) {
    for r in [&g.real, &g.imag] {
        let normalized = BigRational::new(r.numer().clone(), r.denom().clone());
        assert_eq!(r.numer(), normalized.numer(), "canonical numerator");
        assert_eq!(r.denom(), normalized.denom(), "canonical denominator");
    }
}

fn compare(
    name: &str,
    original: &Problem,
    replacement: &Problem,
    cached_original: &PreparedCanonicalProblem,
    cached_replacement: &PreparedCanonicalProblem,
) {
    let (old_sat, old_exact) = independent_full_wmc(name, original);
    let (new_sat, new_exact) = independent_full_wmc(name, replacement);
    assert!(old_sat && new_sat, "weight changes may not change the Boolean CNF");
    assert_ne!(old_exact, new_exact, "declared weights must change exact WMC");

    for cap in [0_usize, 10_000_usize] {
        let limits = Limits {
            cache_entries: cap,
            ..Limits::default()
        };
        let old_baseline = solve_with_limits(original, limits).unwrap();
        let new_baseline = solve_with_limits(replacement, limits).unwrap();
        assert_eq!(old_baseline.count, old_exact);
        assert_eq!(new_baseline.count, new_exact);
        assert_eq!(old_baseline.satisfiable, old_sat);
        assert_eq!(new_baseline.satisfiable, new_sat);

        // Interleave A/B solves: the prepared state must be a snapshot, not
        // a pointer into mutable/replaced caller-owned problem weights.
        for _ in 0..4 {
            let b = cached_replacement.solve_with_limits(limits).unwrap();
            let a = cached_original.solve_with_limits(limits).unwrap();
            assert_eq!(a.count, old_exact, "prepared A retains original weights");
            assert_eq!(b.count, new_exact, "prepared B sees only new weights");
            assert_eq!(a.satisfiable, old_sat);
            assert_eq!(b.satisfiable, new_sat);
            assert_eq!(a.stats, old_baseline.stats);
            assert_eq!(b.stats, new_baseline.stats);
            canonical_fields(&a.count);
            canonical_fields(&b.count);
        }
    }
}

#[test]
fn signed_complex_weight_mutation_requires_separate_prepare() {
    for name in ["satlib_uf20_01", "satlib_uf20_02"] {
        let source = fixture(name);
        let old_prepared = PreparedCanonicalProblem::prepare(&source).unwrap();
        assert!(
            old_prepared.estimated_retained_payload_bytes() > 0,
            "prepared input owns storage even before the first solve"
        );
        let old_bytes = old_prepared.estimated_retained_payload_bytes();

        let changed = with_replaced_first_weight(
            &source,
            Weights::new(
                Gaussian::integers(7, 3),
                Gaussian::integers(-1, -2),
            ),
        );
        let new_prepared = PreparedCanonicalProblem::prepare(&changed).unwrap();
        compare(name, &source, &changed, &old_prepared, &new_prepared);
        assert_eq!(
            old_prepared.estimated_retained_payload_bytes(),
            old_bytes,
            "working on a second problem cannot mutate already-prepared storage"
        );
    }
}

#[test]
fn noncanonical_signed_weight_updates_are_checked_and_snapshotted() {
    for name in ["satlib_uf20_01", "satlib_uf20_02"] {
        let source = fixture(name);
        let prepared_old = PreparedCanonicalProblem::prepare(&source).unwrap();

        // Deliberately unreduced values, negative denominator, zero numerator.
        // The fresh B preparation must canonicalize all four components.
        let changed = with_replaced_first_weight(
            &source,
            Weights::new(
                Gaussian::new(raw(6, -8), raw(-10, 15)),
                Gaussian::new(raw(0, 5), raw(4, 8)),
            ),
        );
        let prepared_new = PreparedCanonicalProblem::prepare(&changed).unwrap();
        compare(name, &source, &changed, &prepared_old, &prepared_new);
    }
}

#[test]
fn invalid_new_weight_does_not_corrupt_existing_prepared_snapshot() {
    for name in ["satlib_uf20_01", "satlib_uf20_02"] {
        let source = fixture(name);
        let prepared_old = PreparedCanonicalProblem::prepare(&source).unwrap();
        let bad = with_replaced_first_weight(
            &source,
            Weights::new(
                Gaussian::new(raw(1, 0), BigRational::from_integer(0.into())),
                Gaussian::one(),
            ),
        );
        let err = PreparedCanonicalProblem::prepare(&bad).unwrap_err();
        assert_eq!(
            err,
            CanonicalMulResearchError::ZeroDenominator {
                variable: 1,
                positive: true,
                component: "real",
            },
        );
        let exact = independent_full_wmc(name, &source);
        let good = prepared_old.solve_with_limits(Limits::default()).unwrap();
        assert_eq!(good.satisfiable, exact.0);
        assert_eq!(good.count, exact.1);
        canonical_fields(&good.count);
    }
}
