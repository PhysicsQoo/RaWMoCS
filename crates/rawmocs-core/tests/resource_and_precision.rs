use num_rational::BigRational;
use rawmocs_core::{
    solve, solve_with_limits, Gaussian as G, Limits, Problem, SolveError, Weights as W,
};

fn g(real: i64, imag: i64) -> G { G::integers(real, imag) }
fn w() -> W { W::new(G::one(), G::one()) }

#[test]
fn exact_noninteger_rational_complex_cancellation() {
    let third = BigRational::new(1.into(), 3.into());
    let fifth = BigRational::new(1.into(), 5.into());
    let pos = G::new(third.clone(), fifth.clone());
    let neg = G::new(-third.clone(), -fifth.clone());
    let problem = Problem::new(1, vec![], vec![W::new(pos, neg)]).unwrap();
    let result = solve(&problem).unwrap();
    assert!(result.satisfiable);
    assert_eq!(result.count, G::zero());
}

#[test]
fn exact_fraction_forced_then_free_and_zero_weight() {
    let half = BigRational::new(1.into(), 2.into());
    let third = BigRational::new(1.into(), 3.into());
    let weights = vec![
        W::new(G::new(half.clone(), third.clone()), g(0, 0)),
        W::new(g(1, 0), g(0, 1)),
    ];
    let problem = Problem::new(2, vec![vec![1]], weights).unwrap();
    let expected = G::new(&half - &third, &half + &third);
    assert_eq!(solve(&problem).unwrap().count, expected);
}

#[test]
fn independent_cache_budget_zero_and_one_match_full_cache() {
    let problem = Problem::new(
        4, vec![vec![1, 2], vec![-1, 2], vec![3, 4], vec![-3, 4]],
        vec![W::new(g(1, 1), g(2, -1)), w(), W::new(g(-1, 0), g(0, 1)), w()]
    ).unwrap();
    let baseline = solve(&problem).unwrap();
    assert!(baseline.stats.decompositions > 0);
    for cap in [0, 1, 2, 4, 10] {
        let result = solve_with_limits(&problem, Limits {
            cache_entries: cap, ..Limits::default()
        }).unwrap();
        assert_eq!(result.count, baseline.count);
        assert_eq!(result.satisfiable, baseline.satisfiable);
        assert!(result.stats.cache_entries <= cap);
    }
}

#[test]
fn node_and_depth_limits_fail_without_partial_count() {
    let problem = Problem::new(2, vec![vec![1, 2]],
        vec![w(), w()]).unwrap();
    assert_eq!(solve_with_limits(&problem, Limits {
        nodes: 0, ..Limits::default()
    }), Err(SolveError::NodeBudgetExceeded));
    assert_eq!(solve_with_limits(&problem, Limits {
        depth: 0, ..Limits::default()
    }), Err(SolveError::DepthBudgetExceeded));
    assert_eq!(solve_with_limits(&problem, Limits {
        depth: 1, ..Limits::default()
    }), Err(SolveError::DepthBudgetExceeded));
    assert!(solve_with_limits(&problem, Limits {
        depth: 10, nodes: 100, cache_entries: 0
    }).is_ok());
}
