//! Independent exhaustive Full Complex WMC checks for SAT-guided branch arithmetic.
//! SAT=true and WMC=0 must not collapse into UNSAT.
use num_rational::BigRational;
use rawmocs_core::{solve, solve_with_limits, Gaussian, Limits, Problem, Weights};

fn w(pr: i64, pi: i64, nr: i64, ni: i64) -> Weights {
    Weights::new(Gaussian::integers(pr, pi), Gaussian::integers(nr, ni))
}

// This arithmetic oracle does not invoke the implementation's add/mul paths.
fn add(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(&a.real + &b.real, &a.imag + &b.imag)
}
fn mul(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}
fn exhaustive(p: &Problem) -> (bool, Gaussian) {
    assert!(p.variables() <= 5);
    let mut sat = false;
    let mut total = Gaussian::zero();
    for mask in 0..(1_usize << p.variables()) {
        if !p.clauses().iter().all(|clause| {
            clause.iter().any(|lit| {
                let var = lit.unsigned_abs() as usize - 1;
                let assigned = mask & (1_usize << var) != 0;
                if *lit > 0 { assigned } else { !assigned }
            })
        }) {
            continue;
        }
        sat = true;
        let mut product = Gaussian::one();
        for (i, weights) in p.weights().iter().enumerate() {
            let factor = if mask & (1_usize << i) == 0 {
                &weights.negative
            } else {
                &weights.positive
            };
            product = mul(&product, factor);
        }
        total = add(&total, &product);
    }
    (sat, total)
}

fn check(p: Problem) {
    let (sat, count) = exhaustive(&p);
    for cap in [0_usize, 1, 10_000] {
        let got = solve_with_limits(
            &p,
            Limits {
                cache_entries: cap,
                ..Limits::default()
            },
        )
        .expect("bounded exhaustive fixture must solve");
        assert_eq!(got.satisfiable, sat, "SAT with cache cap {cap}");
        assert_eq!(got.count, count, "complex count with cache cap {cap}");
    }
    let default = solve(&p).expect("default solver");
    assert_eq!((default.satisfiable, default.count), (sat, count));
}

#[test]
fn logical_unsat_child_contributes_zero_without_weighted_arithmetic() {
    // Four assignments, exactly (x,y)=(true,true) satisfies.
    // No initial unit clauses; one polarity of the branching variable
    // becomes UNSAT through conflicting derived units.
    let p = Problem::new(
        2,
        vec![vec![1, 2], vec![1, -2], vec![-1, 2]],
        vec![w(2, 1, -3, 4), w(1, -2, 5, 3)],
    )
    .unwrap();
    let got = solve(&p).unwrap();
    assert!(got.stats.nodes > 1, "must visit an actual search branch");
    assert_eq!(got.count, Gaussian::integers(4, -3));
    check(p);
}

#[test]
fn zero_weight_does_not_prove_unsat() {
    // Both x branches are logically SAT; signed weights cancel the count.
    let p = Problem::new(
        2,
        vec![vec![1, 2]],
        vec![w(1, 0, -1, 0), w(1, 0, 0, 0)],
    )
    .unwrap();
    let got = solve(&p).unwrap();
    assert!(got.satisfiable);
    assert_eq!(got.count, Gaussian::zero());
    check(p);
}

#[test]
fn all_branches_unsat_and_two_sat_branches_are_distinct() {
    check(
        Problem::new(
            2,
            vec![
                vec![1, 2],
                vec![1, -2],
                vec![-1, 2],
                vec![-1, -2],
            ],
            vec![w(1, 1, 2, 0), w(-1, 0, 0, 3)],
        )
        .unwrap(),
    );
    check(
        Problem::new(
            2,
            vec![vec![1, 2]],
            vec![w(2, -1, -3, 5), w(1, 4, -2, 3)],
        )
        .unwrap(),
    );
}

fn next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}

#[test]
fn deterministic_signed_complex_cnf_oracle() {
    let mut seed = 0x2026_1113_fea7_0bed;
    for n in 1usize..=5 {
        for _ in 0..128 {
            let weights = (0..n)
                .map(|_| {
                    let mut value = || (next(&mut seed) % 7) as i64 - 3;
                    w(value(), value(), value(), value())
                })
                .collect();
            let mut clauses = Vec::new();
            for _ in 0..(next(&mut seed) % 8) {
                let len = 1 + (next(&mut seed) % 3);
                let mut clause = Vec::new();
                for _ in 0..len {
                    let v = (next(&mut seed) % n as u64) as i32 + 1;
                    clause.push(if next(&mut seed) & 1 == 0 { v } else { -v });
                }
                clauses.push(clause);
            }
            check(Problem::new(n, clauses, weights).unwrap());
        }
    }
}

#[test]
fn large_exact_complex_rationals_in_conflicting_branch() {
    let n: num_bigint::BigInt = (num_bigint::BigInt::from(1_u8) << 521_usize) + 12345;
    let d: num_bigint::BigInt = (num_bigint::BigInt::from(1_u8) << 607_usize) + 98765;
    let huge = Gaussian::new(
        BigRational::new(n.clone(), d.clone()),
        BigRational::new(-d, n),
    );
    check(
        Problem::new(
            2,
            vec![vec![1, 2], vec![1, -2], vec![-1, 2]],
            vec![
                Weights::new(huge.clone(), Gaussian::zero()),
                Weights::new(huge, Gaussian::integers(-1, 1)),
            ],
        )
        .unwrap(),
    );
}
