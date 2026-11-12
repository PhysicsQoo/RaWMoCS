//! Independent four-product oracle for exact Gaussian identity fast-paths.
use num_rational::BigRational;
use rawmocs_core::Gaussian;

fn rat(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn classic(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

#[test]
fn exact_zero_and_identity_both_operand_orders() {
    let z = Gaussian::zero();
    let one = Gaussian::one();
    let examples = [
        Gaussian::zero(),
        Gaussian::one(),
        Gaussian::integers(-1, 0),
        Gaussian::integers(0, 1),
        Gaussian::integers(0, -1),
        Gaussian::new(rat(7, 13), rat(-11, 17)),
        Gaussian::new(rat(-29, 31), rat(37, 41)),
    ];
    for a in &examples {
        assert_eq!(a.mul(&one), a.clone());
        assert_eq!(one.mul(a), a.clone());
        assert_eq!(a.mul(&z), z);
        assert_eq!(z.mul(a), z);
        assert_eq!(a.mul(&one), classic(a, &one));
        assert_eq!(one.mul(a), classic(&one, a));
        assert_eq!(a.mul(&z), classic(a, &z));
        assert_eq!(z.mul(a), classic(&z, a));
    }
}

#[test]
fn deterministic_rational_complex_cases_match_original_formula() {
    for seed in 0_i64..512 {
        let a = Gaussian::new(
            rat(seed % 17 - 8, seed % 7 + 1),
            rat((3 * seed) % 19 - 9, seed % 11 + 1),
        );
        let b = Gaussian::new(
            rat((5 * seed) % 23 - 11, seed % 13 + 1),
            rat((7 * seed) % 29 - 14, seed % 17 + 1),
        );
        assert_eq!(a.mul(&b), classic(&a, &b), "seed={seed}");
        assert_eq!(b.mul(&a), classic(&b, &a), "reversed seed={seed}");
    }
}
