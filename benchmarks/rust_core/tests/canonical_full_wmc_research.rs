#![cfg(feature = "canonical-mul-research")]
//! Full Complex WMC oracle: always enumerate Boolean assignments independently.
//! This is not a self-comparison of candidate and baseline implementations.
use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::{
    solve_with_canonical_mul_research, solve_with_limits, solve_with_normalized_input_research,
    CanonicalMulResearchError, Gaussian, Limits, Problem, Weights,
};

fn fraction(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn normalized(r: &BigRational) -> BigRational {
    BigRational::new(r.numer().clone(), r.denom().clone())
}

fn canonical(g: &Gaussian) -> Gaussian {
    Gaussian::new(normalized(&g.real), normalized(&g.imag))
}

fn assert_canonical_fields(value: &Gaussian) {
    let expected = canonical(value);
    assert_eq!(value.real.numer(), expected.real.numer());
    assert_eq!(value.real.denom(), expected.real.denom());
    assert_eq!(value.imag.numer(), expected.imag.numer());
    assert_eq!(value.imag.denom(), expected.imag.denom());
}

fn mul_reference(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

fn add_reference(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(&a.real + &b.real, &a.imag + &b.imag)
}

fn enumerate(p: &Problem) -> (bool, Gaussian) {
    let n = p.variables();
    assert!(n <= 6, "bounded independent oracle");
    let mut satisfiable = false;
    let mut count = Gaussian::zero();
    for mask in 0..(1_usize << n) {
        let satisfied = p.clauses().iter().all(|clause| {
            clause.iter().any(|&lit| {
                let var = lit.unsigned_abs() as usize - 1;
                let chosen = (mask & (1_usize << var)) != 0;
                if lit > 0 {
                    chosen
                } else {
                    !chosen
                }
            })
        });
        if !satisfied {
            continue;
        }
        satisfiable = true;
        let mut product = Gaussian::one();
        for (i, weights) in p.weights().iter().enumerate() {
            let chosen = if (mask & (1_usize << i)) != 0 {
                &weights.positive
            } else {
                &weights.negative
            };
            product = mul_reference(&product, &canonical(chosen));
        }
        count = add_reference(&count, &product);
    }
    (satisfiable, count)
}

fn compare(p: &Problem, both_canonical: bool) {
    let (expected_sat, expected_count) = enumerate(p);
    for cap in [0_usize, 1_usize, 100_usize] {
        let limits = Limits {
            cache_entries: cap,
            ..Limits::default()
        };
        let baseline = solve_with_limits(p, limits).expect("bounded baseline solver");
        let candidate = solve_with_canonical_mul_research(p, limits)
            .expect("bounded, valid-denominator candidate");
        let boundary = solve_with_normalized_input_research(p, limits)
            .expect("bounded, valid-denominator normalized-input reference");
        assert_eq!(baseline.satisfiable, expected_sat, "baseline SAT cap={cap}");
        assert_eq!(
            candidate.satisfiable, expected_sat,
            "candidate SAT cap={cap}"
        );
        assert_eq!(
            canonical(&baseline.count),
            expected_count,
            "baseline WMC cap={cap}"
        );
        assert_eq!(
            canonical(&candidate.count),
            expected_count,
            "candidate WMC cap={cap}"
        );
        assert_eq!(
            boundary.satisfiable, expected_sat,
            "normalized-input SAT cap={cap}"
        );
        assert_eq!(
            canonical(&boundary.count),
            expected_count,
            "normalized-input WMC cap={cap}"
        );
        assert_canonical_fields(&boundary.count);
        assert_eq!(boundary.stats.nodes, candidate.stats.nodes);
        assert_eq!(boundary.stats.cache_hits, candidate.stats.cache_hits);
        assert_eq!(
            boundary.stats.decompositions,
            candidate.stats.decompositions
        );
        assert_canonical_fields(&candidate.count);
        assert_eq!(canonical(&candidate.count), canonical(&baseline.count));
        // Numeric bit-length stats can differ for externally malformed
        // noncanonical representations; Boolean search behavior must not.
        assert_eq!(candidate.stats.nodes, baseline.stats.nodes);
        assert_eq!(candidate.stats.cache_hits, baseline.stats.cache_hits);
        assert_eq!(
            candidate.stats.decompositions,
            baseline.stats.decompositions
        );
        if both_canonical {
            assert_eq!(candidate.stats, baseline.stats, "canonical input stats");
            assert_eq!(boundary.stats, baseline.stats, "canonical boundary stats");
        }
    }
}

fn raw(n: i64, d: i64) -> BigRational {
    BigRational::new_raw(n.into(), d.into())
}

#[test]
fn cancellation_zero_is_still_satisfiable_and_unsat_is_distinct() {
    let cancel = Problem::new(
        1,
        vec![],
        vec![Weights::new(
            Gaussian::integers(1, 0),
            Gaussian::integers(-1, 0),
        )],
    )
    .expect("valid SAT but zero problem");
    let (sat, count) = enumerate(&cancel);
    assert!(sat);
    assert_eq!(count, Gaussian::zero());
    compare(&cancel, true);

    let unsat = Problem::new(
        1,
        vec![vec![1], vec![-1]],
        vec![Weights::new(Gaussian::one(), Gaussian::one())],
    )
    .expect("valid UNSAT problem");
    assert!(!enumerate(&unsat).0);
    compare(&unsat, true);
}

#[test]
fn noncanonical_valid_denominators_are_normalized_at_entry() {
    let weights = vec![
        Weights::new(
            Gaussian::new(raw(2, 4), raw(-3, 6)),
            Gaussian::new(raw(0, 7), raw(1, -2)),
        ),
        Weights::new(
            Gaussian::new(raw(6, 12), raw(9, -3)),
            Gaussian::new(raw(1, 1), raw(3, 9)),
        ),
        Weights::new(
            Gaussian::new(raw(-10, 20), raw(0, 5)),
            Gaussian::new(raw(3, 5), raw(-2, 6)),
        ),
    ];
    for clauses in [
        vec![],
        vec![vec![1, 2], vec![-1, 3]],
        vec![vec![1], vec![2], vec![-3]],
        vec![vec![1, 2], vec![-1, 2], vec![1, -3], vec![-1, -3]],
    ] {
        let p = Problem::new(3, clauses, weights.clone()).expect("valid problem");
        compare(&p, false);
    }
}

#[test]
fn reject_zero_denominator_before_arithmetic() {
    let p = Problem::new(
        1,
        vec![],
        vec![Weights::new(
            Gaussian::new(raw(1, 0), fraction(0, 1)),
            Gaussian::one(),
        )],
    )
    .expect("legacy constructor only checks CNF structure");
    let err = solve_with_canonical_mul_research(&p, Limits::default()).unwrap_err();
    assert!(matches!(
        err,
        CanonicalMulResearchError::ZeroDenominator {
            variable: 1,
            positive: true,
            component: "real"
        }
    ));
    let boundary_err = solve_with_normalized_input_research(&p, Limits::default()).unwrap_err();
    assert_eq!(
        boundary_err, err,
        "same denominator validation for both research arms"
    );
}

#[test]
fn signed_complex_521_and_607_bit_exact_oracle() {
    let n: BigInt = (BigInt::from(1_u8) << 521_usize) + 12345;
    let d: BigInt = (BigInt::from(1_u8) << 607_usize) + 98765;
    let ratio = BigRational::new(n.clone(), d.clone());
    let neg = BigRational::new(-d, n);
    let p = Problem::new(
        2,
        vec![vec![1, 2]],
        vec![
            Weights::new(
                Gaussian::new(ratio.clone(), neg.clone()),
                Gaussian::new(neg.clone(), fraction(0, 1)),
            ),
            Weights::new(
                Gaussian::new(neg.clone(), ratio.clone()),
                Gaussian::new(-ratio, -neg),
            ),
        ],
    )
    .expect("large rational problem");
    compare(&p, true);
}

#[test]
fn deterministic_four_variable_cnf_and_signed_complex_weights() {
    for seed in 0_i64..64 {
        let mut clauses = vec![vec![1, 2], vec![-2, 3], vec![-3, 4]];
        if seed % 2 == 0 {
            clauses.push(vec![-1, -4]);
        }
        if seed % 3 == 0 {
            clauses.push(vec![1, -3, 4]);
        }
        let weights = (0_i64..4)
            .map(|i| {
                let q = seed + i * 11;
                Weights::new(
                    Gaussian::new(fraction(q % 11 - 5, 7 + i), fraction(q % 13 - 6, 11 + i)),
                    Gaussian::new(fraction(q % 17 - 8, 13 + i), fraction(q % 19 - 9, 17 + i)),
                )
            })
            .collect();
        let p = Problem::new(4, clauses, weights).expect("valid deterministic case");
        compare(&p, true);
    }
}
