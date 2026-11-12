//! Exact sparse-complex multiplication compared with an independent four-product oracle.
use num_rational::BigRational;
use rawmocs_core::Gaussian;

fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn classic(lhs: &Gaussian, rhs: &Gaussian) -> Gaussian {
    Gaussian::new(
        &lhs.real * &rhs.real - &lhs.imag * &rhs.imag,
        &lhs.real * &rhs.imag + &lhs.imag * &rhs.real,
    )
}

#[test]
fn sparse_rational_factors_match_general_formula_in_both_orders() {
    for seed in 0_i64..512 {
        let mixed = Gaussian::new(
            rational((5 * seed) % 37 - 18, seed % 13 + 1),
            rational((11 * seed) % 41 - 20, seed % 17 + 1),
        );
        let pure_real = Gaussian::new(
            rational((7 * seed) % 31 - 15, seed % 19 + 1),
            BigRational::from_integer(0.into()),
        );
        let pure_imaginary = Gaussian::new(
            BigRational::from_integer(0.into()),
            rational((13 * seed) % 43 - 21, seed % 23 + 1),
        );
        for sparse in [&pure_real, &pure_imaginary] {
            assert_eq!(mixed.mul(sparse), classic(&mixed, sparse), "right seed={seed}");
            assert_eq!(sparse.mul(&mixed), classic(sparse, &mixed), "left seed={seed}");
        }
        assert_eq!(
            pure_real.mul(&pure_imaginary),
            classic(&pure_real, &pure_imaginary),
            "two sparse factors seed={seed}"
        );
        assert_eq!(
            pure_imaginary.mul(&pure_real),
            classic(&pure_imaginary, &pure_real),
            "two sparse factors reversed seed={seed}"
        );
    }
}

#[test]
fn sparse_factors_preserve_cancellation_and_exact_sign() {
    let i = Gaussian::integers(0, 1);
    let minus_i = Gaussian::integers(0, -1);
    let negative = Gaussian::integers(-7, 0);
    assert_eq!(i.mul(&i), Gaussian::integers(-1, 0));
    assert_eq!(i.mul(&minus_i), Gaussian::one());
    assert_eq!(negative.mul(&i), Gaussian::integers(0, -7));
    assert_eq!(i.mul(&negative), Gaussian::integers(0, -7));
    assert_eq!(negative.mul(&minus_i), Gaussian::integers(0, 7));
}
