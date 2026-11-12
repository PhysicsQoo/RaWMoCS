//! Independent count identities for connected and disconnected residuals.
use rawmocs_core::{solve, solve_with_limits, Gaussian, Limits, Problem, Weights};

fn ones(n: usize) -> Vec<Weights> {
    (0..n).map(|_| Weights::new(Gaussian::one(), Gaussian::one())).collect()
}

#[test]
fn equivalence_chain_stays_single_component() {
    for n in [8_usize, 16, 32, 64] {
        let mut clauses = Vec::new();
        for v in 1..n as i32 {
            clauses.push(vec![-v, v + 1]);
            clauses.push(vec![v, -(v + 1)]);
        }
        let problem = Problem::new(n, clauses, ones(n)).unwrap();
        let report = solve(&problem).unwrap();
        assert!(report.satisfiable);
        assert_eq!(report.count, Gaussian::integers(2, 0));
        assert_eq!(report.stats.decompositions, 0, "connected {n}-var chain");
    }
}

#[test]
fn independent_pairs_still_decompose_exactly() {
    for n in [8_usize, 16, 32, 64] {
        let mut clauses = Vec::new();
        for v in (1..n as i32).step_by(2) {
            clauses.push(vec![v, v + 1]);
            clauses.push(vec![-v, v + 1]);
        }
        let problem = Problem::new(n, clauses, ones(n)).unwrap();
        let counted = solve(&problem).unwrap();
        let no_cache = solve_with_limits(&problem, Limits {
            cache_entries: 0,
            ..Limits::default()
        }).unwrap();
        assert!(counted.satisfiable);
        assert_eq!(counted.count, Gaussian::integers(1_i64 << (n / 2), 0));
        assert_eq!(counted.count, no_cache.count);
        assert!(counted.stats.decompositions > 0, "disconnected {n}-var pairs");
        assert_eq!(no_cache.stats.cache_entries, 0);
    }
}
