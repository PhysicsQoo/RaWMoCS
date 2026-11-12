//! Research-only exact Gauss three-product candidate against the independent
//! established four-product formula. No production arithmetic change.
use num_rational::BigRational;
use num_traits::{One, Zero};
use rawmocs_core::Gaussian;

fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn three(a: &Gaussian, b: &Gaussian) -> Gaussian {
    let ac = &a.real * &b.real;
    let bd = &a.imag * &b.imag;
    let cross = (&a.real + &a.imag) * (&b.real + &b.imag);
    Gaussian::new(&ac - &bd, cross - &ac - &bd)
}

fn four(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

#[test]
fn three_products_are_exact_for_signed_and_cancelling_factors() {
    let zero = BigRational::zero();
    let one = BigRational::one();
    let cases = [
        Gaussian::new(zero.clone(), zero.clone()),
        Gaussian::new(one.clone(), zero.clone()),
        Gaussian::new(zero.clone(), one.clone()),
        Gaussian::new(-one.clone(), one.clone()),
        Gaussian::new(rational(17, 19), rational(-23, 29)),
        Gaussian::new(rational(-31, 37), rational(41, 43)),
        Gaussian::new(rational(101, 103), rational(-101, 103)),
    ];
    for (i, a) in cases.iter().enumerate() {
        for (j, b) in cases.iter().enumerate() {
            assert_eq!(three(a, b), four(a, b), "case {i} x {j}");
            assert_eq!(three(a, b), a.mul(b), "production case {i} x {j}");
        }
    }
    for seed in 0_i64..1024 {
        let a = Gaussian::new(
            rational((37 * seed) % 101 - 50, seed % 29 + 1),
            rational((17 * seed) % 89 - 44, seed % 31 + 1),
        );
        let b = Gaussian::new(
            rational((43 * seed) % 97 - 48, seed % 37 + 1),
            rational((29 * seed) % 83 - 41, seed % 41 + 1),
        );
        assert_eq!(three(&a, &b), four(&a, &b), "seed {seed}");
    }
}

#[test]
fn three_products_are_exact_for_large_numerators_and_denominators() {
    let large = (num_bigint::BigInt::from(1_u8) << 521) + 12345;
    let larger = (num_bigint::BigInt::from(1_u8) << 607) + 98765;
    let a = Gaussian::new(
        BigRational::new(large.clone(), larger.clone()),
        BigRational::new(-larger.clone(), large.clone()),
    );
    let b = Gaussian::new(
        BigRational::new(larger.clone(), large.clone()),
        BigRational::new(large, larger),
    );
    assert_eq!(three(&a, &b), four(&a, &b));
    assert_eq!(three(&b, &a), four(&b, &a));
}
