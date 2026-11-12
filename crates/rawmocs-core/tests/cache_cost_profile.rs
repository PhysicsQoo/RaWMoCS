#![cfg(feature = "cache-cost-profile")]

use rawmocs_core::{
    solve_with_cache_payload_budget, solve_with_limits, CachePayloadBudget, Gaussian, Limits,
    Problem, Weights,
};

fn example() -> Problem {
    // Both selector branches leave the same nontrivial connected residual.
    let clauses = vec![
        vec![1, 2, 3],
        vec![-1, 2, 3],
        vec![1, 3, 4],
        vec![-1, 3, 4],
        vec![1, 4, 5],
        vec![-1, 4, 5],
        vec![1, 5, 2],
        vec![-1, 5, 2],
    ];
    let weights = (0..5)
        .map(|_| Weights::new(Gaussian::integers(2, 1), Gaussian::integers(1, -1)))
        .collect();
    Problem::new(5, clauses, weights).unwrap()
}

#[test]
fn instrumented_cache_transactions_are_consistent_and_semantics_unchanged() {
    let problem = example();
    let off = solve_with_limits(
        &problem,
        Limits {
            cache_entries: 0,
            ..Limits::default()
        },
    )
    .unwrap();
    let on = solve_with_limits(&problem, Limits::default()).unwrap();
    let tight = solve_with_cache_payload_budget(
        &problem,
        Limits::default(),
        CachePayloadBudget {
            scope_items: 0,
            clause_items: 0,
            literal_items: 0,
            value_bits: 0,
        },
    )
    .unwrap();
    assert!(off.satisfiable && on.satisfiable && tight.satisfiable);
    assert_eq!(off.count, on.count);
    assert_eq!(off.count, tight.count);

    let off_cost = &off.stats.cache_cost;
    assert_eq!(off.stats.cache_hits, 0);
    assert_eq!(off_cost.key_build_calls, 0);
    assert_eq!(off_cost.lookup_calls, 0);
    assert_eq!(off_cost.insert_calls, 0);
    assert_eq!(off_cost.admission_calls, 0);

    let on_cost = &on.stats.cache_cost;
    assert!(on.stats.cache_hits > 0);
    assert_eq!(on_cost.key_build_calls, on.stats.nodes);
    assert_eq!(on_cost.lookup_calls, on_cost.key_build_calls);
    assert_eq!(on_cost.hit_clone_calls, on.stats.cache_hits);
    assert_eq!(on_cost.insert_calls, on.stats.cache_entries as u64);
    assert_eq!(on_cost.admission_calls, on_cost.insert_calls);
    assert!(on_cost.key_build_ns > 0);
    assert!(on_cost.lookup_ns > 0);

    let tight_cost = &tight.stats.cache_cost;
    assert_eq!(tight.stats.cache_entries, 0);
    assert_eq!(tight.stats.cache_hits, 0);
    assert!(tight.stats.cache_payload_rejections > 0);
    assert_eq!(tight_cost.key_build_calls, tight.stats.nodes);
    assert_eq!(tight_cost.lookup_calls, tight_cost.key_build_calls);
    assert_eq!(tight_cost.insert_calls, 0);
    assert_eq!(tight_cost.admission_calls, tight.stats.cache_payload_rejections);
}
