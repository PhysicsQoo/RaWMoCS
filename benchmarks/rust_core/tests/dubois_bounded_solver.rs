#![cfg(feature = "canonical-mul-research")]
//! Source-pinned, proof-backed UNSAT vs UNKNOWN contract, without invoking
//! unbounded default DPLL on the 60-variable/160-clause Dubois20 instance.
//! The source SATLIB weights are deliberately synthetic, not historical WMC.

use rawmocs_benchmarks::dubois_xor_certificate::verify_dubois20_unsat;
use rawmocs_benchmarks::satlib_provenance::fixture;
use rawmocs_core::{
    solve_with_canonical_mul_research, solve_with_limits, CanonicalMulResearchError, Gaussian,
    Limits, PreparedCanonicalProblem, Report, SolveError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    ProvenUnsat,
    UnknownNodeBudget,
    UnknownDepthBudget,
}

fn verdict(result: Result<Report, SolveError>) -> Verdict {
    match result {
        Ok(report) => {
            // No SAT verdict can be accepted on an independently certified
            // UNSAT CNF, regardless of a numerically zero complex WMC.
            assert!(!report.satisfiable, "certified Dubois20 must be UNSAT");
            assert_eq!(report.count, Gaussian::zero(), "UNSAT exact count");
            for component in [&report.count.real, &report.count.imag] {
                let canonical = num_rational::BigRational::new(
                    component.numer().clone(),
                    component.denom().clone(),
                );
                assert_eq!(component.numer(), canonical.numer());
                assert_eq!(component.denom(), canonical.denom());
            }
            Verdict::ProvenUnsat
        }
        Err(SolveError::NodeBudgetExceeded) => Verdict::UnknownNodeBudget,
        Err(SolveError::DepthBudgetExceeded) => Verdict::UnknownDepthBudget,
    }
}

fn gated(result: Result<Report, CanonicalMulResearchError>) -> Verdict {
    match result {
        Ok(report) => verdict(Ok(report)),
        Err(CanonicalMulResearchError::Solve(error)) => verdict(Err(error)),
        Err(CanonicalMulResearchError::ZeroDenominator { .. }) => {
            panic!("well-formed source-pinned rational weights cannot have zero denominators")
        }
    }
}

#[test]
fn source_certified_unsat_must_not_be_inferred_from_budget_errors() {
    let certificate = verify_dubois20_unsat().expect("independent checked XOR contradiction");
    assert_eq!(certificate.variables, 60);
    assert_eq!(certificate.xor_equations, 40);
    assert_eq!(certificate.rhs_xor, 1);

    let source = fixture("satlib_dubois20_unverified");
    let prepared = PreparedCanonicalProblem::prepare(&source).expect("valid synthetic weights");

    // Resource-guarded bounded samples; never an unlimited full DPLL search.
    for nodes in [1_u64, 32_u64, 256_u64] {
        for cache_entries in [0_usize, 10_000_usize] {
            let limits = Limits {
                nodes,
                depth: 128,
                cache_entries,
            };
            let original = verdict(solve_with_limits(&source, limits));
            let prepared_once = verdict(prepared.solve_with_limits(limits));
            let normalized_each_call = gated(solve_with_canonical_mul_research(&source, limits));

            // Arithmetic and boundary preparation may affect CPU and memory,
            // but must not change the *logical* search-status classification.
            assert_eq!(
                prepared_once, original,
                "prepared-vs-original classification nodes={nodes} cache={cache_entries}"
            );
            assert_eq!(
                normalized_each_call, original,
                "gated-vs-original classification nodes={nodes} cache={cache_entries}"
            );
        }
    }
}
