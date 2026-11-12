//! Independent classical four-product oracle for experimental Gaussian multiplication.
//! Source-pinned A0 and analytical medium-CNF tests provide wider solver coverage.

use num_rational::BigRational;
use rawmocs_core::Gaussian;

fn r(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn reference(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

#[test]
fn experimental_identity_matches_four_product_exactly() {
    for seed in 0_i64..512 {
        let a = Gaussian::new(
            r(seed % 17 - 8, seed % 7 + 1),
            r((3 * seed) % 19 - 9, seed % 11 + 1),
        );
        let b = Gaussian::new(
            r((5 * seed) % 23 - 11, seed % 13 + 1),
            r((7 * seed) % 29 - 14, seed % 17 + 1),
        );
        assert_eq!(a.mul(&b), reference(&a, &b), "seed={seed}");
        assert_eq!(b.mul(&a), reference(&b, &a), "commutative seed={seed}");
    }
}

#[test]
fn zero_and_unit_arithmetic_remain_exact() {
    let zero = Gaussian::zero();
    let one = Gaussian::one();
    let imaginary = Gaussian::new(r(-7, 13), r(11, 17));
    assert_eq!(zero.mul(&imaginary), zero);
    assert_eq!(imaginary.mul(&zero), zero);
    assert_eq!(imaginary.mul(&one), imaginary);
    assert_eq!(one.mul(&imaginary), imaginary);
    assert_eq!(imaginary.mul(&imaginary), reference(&imaginary, &imaginary));
}
