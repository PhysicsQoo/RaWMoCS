//! Cache-off must not allocate cache keys or alter exact Full WMC semantics.
use rawmocs_core::{solve, solve_with_limits, Gaussian, Limits, Problem, Weights};

fn w(positive: (i64, i64), negative: (i64, i64)) -> Weights {
    Weights::new(
        Gaussian::integers(positive.0, positive.1),
        Gaussian::integers(negative.0, negative.1),
    )
}

#[test]
fn cache_disabled_counts_equal_bounded_cached_counts() {
    let n = 16_usize;
    let mut clauses = Vec::new();
    for x in (1_i32..=n as i32).step_by(2) {
        clauses.push(vec![x, x + 1]);
        clauses.push(vec![-x, x + 1]);
    }
    let weights = (0..n)
        .map(|i| {
            let k = i as i64;
            w((k % 5 - 2, k % 3 - 1), (k % 7 - 3, (k + 1) % 4 - 2))
        })
        .collect();
    let problem = Problem::new(n, clauses, weights).unwrap();
    let baseline = solve(&problem).unwrap();
    assert!(baseline.satisfiable);
    assert!(baseline.stats.decompositions > 0);
    for cap in [0, 1, 2, 8, 128] {
        let report = solve_with_limits(
            &problem,
            Limits {
                cache_entries: cap,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(report.satisfiable, baseline.satisfiable);
        assert_eq!(report.count, baseline.count);
        assert!(report.stats.cache_entries <= cap);
        if cap == 0 {
            assert_eq!(report.stats.cache_hits, 0);
            assert_eq!(report.stats.cache_entries, 0);
        }
    }
}

#[test]
fn no_cache_still_distinguishes_sat_zero_and_unsat() {
    let weights = vec![w((1, 0), (2, 0)), w((1, 0), (-1, 0))];
    let satisfiable_zero = Problem::new(2, vec![vec![1]], weights.clone()).unwrap();
    let unsatisfiable = Problem::new(2, vec![vec![1], vec![-1]], weights).unwrap();
    let cap = Limits {
        cache_entries: 0,
        ..Limits::default()
    };
    let a = solve_with_limits(&satisfiable_zero, cap).unwrap();
    let b = solve_with_limits(&unsatisfiable, cap).unwrap();
    assert!(a.satisfiable);
    assert!(!b.satisfiable);
    assert_eq!(a.count, Gaussian::zero());
    assert_eq!(b.count, Gaussian::zero());
    assert_eq!(a.stats.cache_entries, 0);
    assert_eq!(b.stats.cache_entries, 0);
}
