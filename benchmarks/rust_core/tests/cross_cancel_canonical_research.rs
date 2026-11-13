//! Independent exact rational cross-cancellation oracle and negative controls.
use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_benchmarks::cross_cancel_research::{
    is_canonical_fraction, mul_canonical_cross_cancelled,
};

fn ratio(n: i64, d: i64) -> BigRational {
    BigRational::new(BigInt::from(n), BigInt::from(d))
}

fn check_pair(a: &BigRational, b: &BigRational) {
    assert!(is_canonical_fraction(a), "left input must be canonical");
    assert!(is_canonical_fraction(b), "right input must be canonical");
    let actual = mul_canonical_cross_cancelled(a, b);
    // Independent schoolbook oracle: no pre-cancellation and independent
    // BigRational::new normalization. Compare canonical FIELDS, not just value.
    let expected = BigRational::new(
        a.numer() * b.numer(),
        a.denom() * b.denom(),
    );
    assert_eq!(actual.numer(), expected.numer(), "exact numerator");
    assert_eq!(actual.denom(), expected.denom(), "positive canonical denominator");
    assert_eq!(actual, a * b, "production num-rational agreement");
    assert!(is_canonical_fraction(&actual));
}

#[test]
fn signed_zero_one_and_divisibility_edges() {
    let inputs = [
        ratio(0, 1),
        ratio(1, 1),
        ratio(-1, 1),
        ratio(2, 3),
        ratio(-2, 3),
        ratio(3, -7),
        ratio(13, 17),
        ratio(-17, 13),
        ratio(8, 12),
        ratio(12, 8),
        ratio(123, 456),
        ratio(-987, 654),
    ];
    for a in &inputs {
        for b in &inputs {
            check_pair(a, b);
        }
    }
}

#[test]
fn deterministic_exact_sign_and_cancellation_corpus() {
    for i in 0_i64..2048 {
        let a = ratio((17 * i % 1009) - 504, (19 * i % 997) + 1);
        let b = ratio((23 * i % 1013) - 506, (29 * i % 991) + 1);
        check_pair(&a, &b);
        check_pair(&a, &(-b.clone()));
        let other = ratio((31 * i % 983) + 1, -((37 * i % 977) + 1));
        check_pair(&a, &other);
    }
}

#[test]
fn large_521_and_607_bit_operands() {
    let n: BigInt = (BigInt::from(1_u8) << 521_usize) + 12345;
    let d: BigInt = (BigInt::from(1_u8) << 607_usize) + 98765;
    let p: BigInt = (BigInt::from(1_u8) << 521_usize) + 6;
    let q: BigInt = (BigInt::from(1_u8) << 607_usize) + 2;
    let cases = [
        BigRational::new(n.clone(), d.clone()),
        BigRational::new(-d.clone(), n.clone()),
        BigRational::new(q, p),
        BigRational::new(-n, d),
    ];
    for a in &cases {
        for b in &cases {
            check_pair(a, b);
        }
    }
}

#[test]
fn reject_uncanonical_input_precondition() {
    let bad_reducible = BigRational::new_raw(BigInt::from(2), BigInt::from(4));
    let bad_sign = BigRational::new_raw(BigInt::from(1), BigInt::from(-3));
    let bad_zero = BigRational::new_raw(BigInt::from(0), BigInt::from(2));
    assert!(!is_canonical_fraction(&bad_reducible));
    assert!(!is_canonical_fraction(&bad_sign));
    assert!(!is_canonical_fraction(&bad_zero));
    let valid = ratio(5, 7);
    assert!(is_canonical_fraction(&valid));
    // The research-only candidate enforces this precondition in debug/test.
    if cfg!(debug_assertions) {
        assert!(std::panic::catch_unwind(|| {
            mul_canonical_cross_cancelled(&bad_reducible, &valid)
        }).is_err());
    }
}
