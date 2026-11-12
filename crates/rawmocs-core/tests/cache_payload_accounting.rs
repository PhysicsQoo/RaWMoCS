//! Retained cache payload diagnostics are not a process memory limit.
use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::{solve_with_limits, Gaussian, Limits, Problem, Weights};

fn fraction(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

#[test]
fn cache_payload_counters_have_exact_single_entry_contract() {
    let expected = Gaussian::new(fraction(1, 3), fraction(-1, 5));
    let problem = Problem::new(
        1,
        vec![vec![1]],
        vec![Weights::new(expected.clone(), Gaussian::zero())],
    )
    .unwrap();
    let bounded = solve_with_limits(
        &problem,
        Limits {
            cache_entries: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(bounded.satisfiable);
    assert_eq!(bounded.count, expected);
    assert_eq!(bounded.stats.cache_entries, 1);
    assert_eq!(bounded.stats.cache_scope_items, 1);
    assert_eq!(bounded.stats.cache_clause_items, 1);
    assert_eq!(bounded.stats.cache_literal_items, 1);
    assert_eq!(bounded.stats.cache_value_bits, 7);
    assert_eq!(bounded.stats.max_cached_value_bits, 7);

    let no_cache = solve_with_limits(
        &problem,
        Limits {
            cache_entries: 0,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(no_cache.satisfiable);
    assert_eq!(no_cache.count, bounded.count);
    assert_eq!(no_cache.stats.cache_entries, 0);
    assert_eq!(no_cache.stats.cache_scope_items, 0);
    assert_eq!(no_cache.stats.cache_clause_items, 0);
    assert_eq!(no_cache.stats.cache_literal_items, 0);
    assert_eq!(no_cache.stats.cache_value_bits, 0);
    assert_eq!(no_cache.stats.max_cached_value_bits, 0);
}

#[test]
fn rational_bits_track_growth_but_do_not_claim_allocated_bytes() {
    let numerator = BigInt::from(1_u32) << 256;
    let exact = Gaussian::new(
        BigRational::new(numerator, 3.into()),
        BigRational::from_integer(0.into()),
    );
    let problem = Problem::new(
        1,
        vec![vec![1]],
        vec![Weights::new(exact.clone(), Gaussian::one())],
    )
    .unwrap();
    let result = solve_with_limits(
        &problem,
        Limits {
            cache_entries: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(result.count, exact);
    assert_eq!(result.stats.cache_entries, 1);
    assert!(result.stats.cache_value_bits >= 260);
    assert_eq!(result.stats.cache_value_bits, result.stats.max_cached_value_bits);
}

#[test]
fn cache_payload_metrics_remain_capped_and_do_not_change_count() {
    let problem = Problem::new(
        4,
        vec![vec![1, 2], vec![-1, 2], vec![3, 4], vec![-3, 4]],
        vec![
            Weights::new(Gaussian::integers(1, 1), Gaussian::integers(2, -1)),
            Weights::new(Gaussian::one(), Gaussian::one()),
            Weights::new(Gaussian::integers(-1, 0), Gaussian::integers(0, 1)),
            Weights::new(Gaussian::one(), Gaussian::one()),
        ],
    )
    .unwrap();
    let baseline = solve_with_limits(
        &problem,
        Limits {
            cache_entries: 128,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(baseline.satisfiable);
    for cap in [0, 1, 2, 4, 8] {
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
        assert!(report.stats.max_cached_value_bits <= report.stats.cache_value_bits);
        if report.stats.cache_entries == 0 {
            assert_eq!(report.stats.cache_scope_items, 0);
            assert_eq!(report.stats.cache_clause_items, 0);
            assert_eq!(report.stats.cache_literal_items, 0);
            assert_eq!(report.stats.cache_value_bits, 0);
        } else {
            assert!(report.stats.cache_scope_items > 0);
            assert!(report.stats.cache_value_bits > 0);
        }
    }
}
