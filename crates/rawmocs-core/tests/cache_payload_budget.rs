//! Logical cache quotas bound retained payload counts, not allocated bytes.
use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::{
    solve_with_cache_payload_budget, solve_with_limits, CachePayloadBudget, Gaussian, Limits,
    Problem, SolveError, Weights,
};

fn unrestricted() -> CachePayloadBudget {
    CachePayloadBudget {
        scope_items: usize::MAX,
        clause_items: usize::MAX,
        literal_items: usize::MAX,
        value_bits: u64::MAX,
    }
}

fn unit_problem() -> Problem {
    let exact = Gaussian::new(
        BigRational::new(1.into(), 3.into()),
        BigRational::new((-1).into(), 5.into()),
    );
    Problem::new(
        1,
        vec![vec![1]],
        vec![Weights::new(exact, Gaussian::zero())],
    )
    .unwrap()
}

#[test]
fn exact_boundary_and_each_independent_quota() {
    let problem = unit_problem();
    let limits = Limits {
        cache_entries: 1,
        ..Limits::default()
    };
    let baseline = solve_with_limits(&problem, limits).unwrap();
    assert_eq!(baseline.stats.cache_entries, 1);
    assert_eq!(baseline.stats.cache_scope_items, 1);
    assert_eq!(baseline.stats.cache_clause_items, 1);
    assert_eq!(baseline.stats.cache_literal_items, 1);
    assert_eq!(baseline.stats.cache_value_bits, 7);
    let accepted = solve_with_cache_payload_budget(
        &problem,
        limits,
        CachePayloadBudget {
            scope_items: 1,
            clause_items: 1,
            literal_items: 1,
            value_bits: 7,
        },
    )
    .unwrap();
    assert_eq!(accepted.count, baseline.count);
    assert_eq!(accepted.stats.cache_entries, 1);
    assert_eq!(accepted.stats.cache_payload_rejections, 0);

    for budget in [
        CachePayloadBudget {
            scope_items: 0,
            ..unrestricted()
        },
        CachePayloadBudget {
            clause_items: 0,
            ..unrestricted()
        },
        CachePayloadBudget {
            literal_items: 0,
            ..unrestricted()
        },
        CachePayloadBudget {
            value_bits: 6,
            ..unrestricted()
        },
    ] {
        let got = solve_with_cache_payload_budget(&problem, limits, budget).unwrap();
        assert_eq!(got.count, baseline.count);
        assert!(got.satisfiable);
        assert_eq!(got.stats.cache_entries, 0);
        assert_eq!(got.stats.cache_payload_rejections, 1);
        assert_eq!(got.stats.cache_scope_items, 0);
        assert_eq!(got.stats.cache_clause_items, 0);
        assert_eq!(got.stats.cache_literal_items, 0);
        assert_eq!(got.stats.cache_value_bits, 0);
    }
}

fn disjoint_problem() -> Problem {
    Problem::new(
        4,
        vec![vec![1, 2], vec![-1, 2], vec![3, 4], vec![-3, 4]],
        vec![
            Weights::new(Gaussian::integers(1, 1), Gaussian::integers(2, -1)),
            Weights::new(Gaussian::one(), Gaussian::one()),
            Weights::new(Gaussian::integers(-1, 0), Gaussian::integers(0, 1)),
            Weights::new(Gaussian::one(), Gaussian::one()),
        ],
    )
    .unwrap()
}

#[test]
fn cumulative_quotas_preserve_full_wmc_and_sat() {
    let problem = disjoint_problem();
    let baseline = solve_with_limits(&problem, Limits::default()).unwrap();
    for cap in [0, 1, 4, 16, 128] {
        let budget = CachePayloadBudget {
            scope_items: cap,
            clause_items: cap,
            literal_items: cap,
            value_bits: cap as u64,
        };
        let got = solve_with_cache_payload_budget(&problem, Limits::default(), budget).unwrap();
        assert_eq!(got.count, baseline.count, "quota {cap}");
        assert_eq!(got.satisfiable, baseline.satisfiable);
        assert!(got.stats.cache_scope_items <= cap);
        assert!(got.stats.cache_clause_items <= cap);
        assert!(got.stats.cache_literal_items <= cap);
        assert!(got.stats.cache_value_bits <= cap as u64);
        assert!(got.stats.max_cached_value_bits <= got.stats.cache_value_bits);
    }
}

#[test]
fn large_exact_rational_is_not_truncated_by_admission() {
    let numer = BigInt::from(1_u8) << 512;
    let count = Gaussian::new(
        BigRational::new(numer, 3.into()),
        BigRational::from_integer(0.into()),
    );
    let problem = Problem::new(
        1,
        vec![vec![1]],
        vec![Weights::new(count.clone(), Gaussian::zero())],
    )
    .unwrap();
    let got = solve_with_cache_payload_budget(
        &problem,
        Limits::default(),
        CachePayloadBudget {
            value_bits: 128,
            ..unrestricted()
        },
    )
    .unwrap();
    assert_eq!(got.count, count);
    assert!(got.satisfiable);
    assert_eq!(got.stats.cache_entries, 0);
    assert_eq!(got.stats.cache_payload_rejections, 1);
}

#[test]
fn sat_zero_and_unsat_remain_distinct_with_zero_quota() {
    let weights = vec![
        Weights::new(Gaussian::one(), Gaussian::one()),
        Weights::new(Gaussian::one(), Gaussian::integers(-1, 0)),
    ];
    let sat_zero = Problem::new(2, vec![vec![1]], weights.clone()).unwrap();
    let unsat = Problem::new(2, vec![vec![1], vec![-1]], weights).unwrap();
    for (problem, expected_sat) in [(&sat_zero, true), (&unsat, false)] {
        let got = solve_with_cache_payload_budget(
            problem,
            Limits::default(),
            CachePayloadBudget {
                scope_items: 0,
                clause_items: 0,
                literal_items: 0,
                value_bits: 0,
            },
        )
        .unwrap();
        assert_eq!(got.satisfiable, expected_sat);
        assert_eq!(got.count, Gaussian::zero());
        assert_eq!(got.stats.cache_entries, 0);
    }
}

#[test]
fn computation_node_and_depth_fail_closed_independently() {
    let problem = disjoint_problem();
    assert_eq!(
        solve_with_cache_payload_budget(
            &problem,
            Limits {
                nodes: 0,
                ..Limits::default()
            },
            unrestricted(),
        ),
        Err(SolveError::NodeBudgetExceeded)
    );
    assert_eq!(
        solve_with_cache_payload_budget(
            &problem,
            Limits {
                depth: 0,
                ..Limits::default()
            },
            unrestricted(),
        ),
        Err(SolveError::DepthBudgetExceeded)
    );
}
