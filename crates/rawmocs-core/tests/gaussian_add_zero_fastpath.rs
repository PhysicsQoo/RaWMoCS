//! Exact addition identity tests against independent BigRational component sums.
use num_rational::BigRational;
use rawmocs_core::Gaussian;

fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn reference(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(&a.real + &b.real, &a.imag + &b.imag)
}

#[test]
fn zero_addition_fastpath_preserves_exact_rational_arithmetic() {
    let z = Gaussian::zero();
    let values = [
        z.clone(),
        Gaussian::one(),
        Gaussian::integers(0, -1),
        Gaussian::integers(-1, 0),
        Gaussian::new(rational(-17, 19), rational(23, 29)),
        Gaussian::new(rational(31, 37), rational(-41, 43)),
    ];
    for (i, a) in values.iter().enumerate() {
        assert_eq!(a.add(&z), reference(a, &z), "right zero {i}");
        assert_eq!(z.add(a), reference(&z, a), "left zero {i}");
        assert_eq!(a.add(a), reference(a, a), "double {i}");
    }
    for seed in 0_i64..1024 {
        let a = Gaussian::new(
            rational((17 * seed) % 101 - 50, seed % 29 + 1),
            rational((19 * seed) % 103 - 51, seed % 31 + 1),
        );
        let b = Gaussian::new(
            rational((23 * seed) % 107 - 53, seed % 37 + 1),
            rational((29 * seed) % 109 - 54, seed % 41 + 1),
        );
        assert_eq!(a.add(&b), reference(&a, &b), "seed {seed}");
        let negative_a = Gaussian::new(-a.real.clone(), -a.imag.clone());
        assert_eq!(a.add(&negative_a), z, "cancellation {seed}");
    }
}

#[test]
fn large_exact_rational_addition_zero_identity() {
    let n: num_bigint::BigInt = (num_bigint::BigInt::from(1_u8) << 521_usize) + 12345;
    let d: num_bigint::BigInt = (num_bigint::BigInt::from(1_u8) << 607_usize) + 98765;
    let a = Gaussian::new(
        BigRational::new(n.clone(), d.clone()),
        BigRational::new(-d, n),
    );
    let z = Gaussian::zero();
    assert_eq!(a.add(&z), reference(&a, &z));
    assert_eq!(z.add(&a), reference(&z, &a));
}
