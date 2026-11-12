//! Solver-level independent Boolean enumeration for the fused residual scan.
//! This compares exact Full WMC and SAT, not a rounded floating-point result.
use rawmocs_core::{solve_with_limits, Gaussian, Limits, Problem, Weights};

fn sample_weights(variables: usize) -> Vec<Weights> {
    (0..variables)
        .map(|i| {
            let k = i as i64;
            Weights::new(
                Gaussian::integers(k % 5 - 2, (3 * k) % 5 - 2),
                Gaussian::integers((2 * k) % 7 - 3, k % 3 - 1),
            )
        })
        .collect()
}

fn truth_table(problem: &Problem) -> (bool, Gaussian) {
    assert!(problem.variables() <= 12);
    let mut sat = false;
    let mut count = Gaussian::zero();
    for bits in 0..(1_usize << problem.variables()) {
        let satisfies = problem.clauses().iter().all(|clause| {
            clause.iter().any(|&lit| {
                let var = lit.unsigned_abs() as usize - 1;
                let selected = (bits & (1_usize << var)) != 0;
                selected == (lit > 0)
            })
        });
        if !satisfies {
            continue;
        }
        sat = true;
        let mut weight = Gaussian::one();
        for (var, choices) in problem.weights().iter().enumerate() {
            let selected = (bits & (1_usize << var)) != 0;
            let factor = if selected {
                &choices.positive
            } else {
                &choices.negative
            };
            weight = weight.mul(factor);
        }
        count = count.add(&weight);
    }
    (sat, count)
}

#[test]
fn mixed_residuals_match_independent_full_wmc_for_multiple_cache_caps() {
    let families = [
        vec![],
        vec![vec![1], vec![2, 3], vec![-2, 4], vec![5, -6]],
        vec![
            vec![1, 2, 3],
            vec![-1, 2],
            vec![-2, 4],
            vec![-4, 5, 6],
            vec![6, -7],
            vec![-5, -7],
        ],
        vec![vec![1], vec![-1]],
        vec![
            vec![1, 2],
            vec![-1, 2],
            vec![3, 4],
            vec![-3, 4],
            vec![5, 6, 7],
        ],
        vec![vec![-1, 2], vec![-2, 3], vec![-3, 4], vec![4, -5]],
    ];
    for (i, clauses) in families.into_iter().enumerate() {
        let problem = Problem::new(7, clauses, sample_weights(7)).unwrap();
        let (expected_sat, expected_count) = truth_table(&problem);
        for cache_entries in [0, 1, 4, 1024] {
            let result = solve_with_limits(
                &problem,
                Limits {
                    cache_entries,
                    ..Limits::default()
                },
            )
            .unwrap();
            assert_eq!(
                result.satisfiable, expected_sat,
                "family={i} cap={cache_entries}"
            );
            assert_eq!(
                result.count, expected_count,
                "family={i} cap={cache_entries}"
            );
            assert!(result.stats.cache_entries <= cache_entries);
            if cache_entries == 0 {
                assert_eq!(result.stats.cache_hits, 0);
            }
        }
    }
}

#[test]
fn resolved_clauses_keep_remaining_free_weights_exact() {
    let weights = vec![
        Weights::new(Gaussian::integers(2, 1), Gaussian::zero()),
        Weights::new(Gaussian::integers(1, 0), Gaussian::integers(-1, 0)),
        Weights::new(Gaussian::integers(3, 1), Gaussian::integers(2, -1)),
    ];
    let problem = Problem::new(3, vec![vec![1]], weights).unwrap();
    let (sat, count) = truth_table(&problem);
    assert!(sat);
    assert_eq!(count, Gaussian::zero());
    for cache_entries in [0, 8] {
        let result = solve_with_limits(
            &problem,
            Limits {
                cache_entries,
                ..Limits::default()
            },
        )
        .unwrap();
        assert!(result.satisfiable);
        assert_eq!(result.count, count);
    }
}
