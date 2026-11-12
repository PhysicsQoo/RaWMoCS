use rawmocs_core::{solve, solve_with_budget, Gaussian as G, Problem, ProblemError, SolveError, Weights as W};

fn w(pr: i64, pi: i64, nr: i64, ni: i64) -> W {
    W::new(G::integers(pr, pi), G::integers(nr, ni))
}

// Independent exhaustive truth-assignment oracle; not based on DPLL.
fn brute(p: &Problem) -> (bool, G) {
    assert!(p.variables() <= 8);
    let mut sum = G::zero();
    let mut sat = false;
    for mask in 0..(1usize << p.variables()) {
        let holds = p.clauses().iter().all(|c| c.iter().any(|&lit| {
            let value = mask & (1usize << (lit.unsigned_abs() as usize - 1)) != 0;
            if lit > 0 { value } else { !value }
        }));
        if !holds { continue; }
        sat = true;
        let mut product = G::one();
        for (i, pair) in p.weights().iter().enumerate() {
            product = product.mul(if mask & (1usize << i) != 0 {
                &pair.positive
            } else {
                &pair.negative
            });
        }
        sum = sum.add(&product);
    }
    (sat, sum)
}
fn check(p: &Problem) {
    let got = solve(p).expect("small CNF must complete");
    assert_eq!((got.satisfiable, got.count), brute(p));
}
#[test]
fn validates_variables_and_weight_count() {
    assert_eq!(Problem::new(1, vec![vec![2]], vec![w(1, 0, 1, 0)]).unwrap_err(),
        ProblemError::InvalidLiteral(2));
    assert_eq!(Problem::new(1, vec![vec![i32::MIN]], vec![w(1, 0, 1, 0)]).unwrap_err(),
        ProblemError::InvalidLiteral(i32::MIN));
    assert_eq!(Problem::new(1, vec![], vec![]).unwrap_err(),
        ProblemError::IncorrectWeightCount);
}
#[test]
fn units_free_variables_and_negative_weights() {
    let p = Problem::new(2, vec![vec![1]], vec![w(0, 1, 2, 0), w(2, 1, -1, 3)]).unwrap();
    assert_eq!(solve(&p).unwrap().count, G::integers(-4, 1));
    check(&p);
}
#[test]
fn sat_zero_is_not_unsatisfiable() {
    let p = Problem::new(1, vec![], vec![w(1, 0, -1, 0)]).unwrap();
    let got = solve(&p).unwrap();
    assert!(got.satisfiable);
    assert_eq!(got.count, G::zero());
}
#[test]
fn zero_branch_factor_is_not_divided_out() {
    let p = Problem::new(2, vec![vec![1, 2]], vec![w(0, 0, 1, 0), w(0, 1, 1, 0)]).unwrap();
    assert_eq!(solve(&p).unwrap().count, G::integers(0, 1));
    check(&p);
}
#[test]
fn conflict_stays_unsat() {
    let p = Problem::new(1, vec![vec![1], vec![-1]], vec![w(0, 0, 1, 0)]).unwrap();
    let got = solve(&p).unwrap();
    assert!(!got.satisfiable);
    assert_eq!(got.count, G::zero());
}
#[test]
fn components_and_tautologies_preserve_count() {
    let p = Problem::new(5, vec![vec![1, 2], vec![-3, 4], vec![5, -5]],
        vec![w(1, 1, 2, 0), w(0, 1, 1, -1), w(2, 2, -1, 0),
             w(1, 0, 0, -1), w(3, 0, 0, 1)]).unwrap();
    assert!(solve(&p).unwrap().stats.decompositions > 0);
    check(&p);
}
#[test]
fn node_budget_fails_closed() {
    let p = Problem::new(0, vec![], vec![]).unwrap();
    assert_eq!(solve_with_budget(&p, 0), Err(SolveError::NodeBudgetExceeded));
}
fn next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn generated_small_full_wmc_matches_independent_exhaustive_oracle() {
    let mut seed = 0x5eed_c0de_1234_5678;
    for vars in 0usize..=4 {
        for _ in 0..100 {
            let pairs = (0..vars).map(|_| {
                let mut n = || (next(&mut seed) % 5) as i64 - 2;
                w(n(), n(), n(), n())
            }).collect();
            let mut clauses = Vec::new();
            for _ in 0..(next(&mut seed) % 6) {
                let length = if vars == 0 { 0 } else { (next(&mut seed) % 4) as usize };
                let mut clause = Vec::new();
                for _ in 0..length {
                    let variable = (next(&mut seed) % vars as u64) as i32 + 1;
                    clause.push(if next(&mut seed) & 1 == 0 { variable } else { -variable });
                }
                clauses.push(clause);
            }
            check(&Problem::new(vars, clauses, pairs).unwrap());
        }
    }
}
