//! Research-only exact BigRational multiplication with a proven canonical output.
//!
//! This is deliberately outside rawmocs-core. The input contract is strict:
//! each fraction must already have a positive denominator and coprime
//! numerator/denominator. Do not use this function on unchecked raw fractions.

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;

/// Source-level canonicality check for research admission and negative cases.
///
/// This construction is intentionally expensive and MUST NOT be included in
/// release-mode benchmark spans as a hidden proof cost.
pub fn is_canonical_fraction(value: &BigRational) -> bool {
    if value.denom() <= &BigInt::from(0_i32) {
        return false;
    }
    let reference = BigRational::new(value.numer().clone(), value.denom().clone());
    value.numer() == reference.numer() && value.denom() == reference.denom()
}

/// Multiply *canonical* rational inputs, with no redundant final GCD.
///
/// Let the inputs be a/b and c/d, b,d>0, gcd(|a|,b)=gcd(|c|,d)=1.
/// Set g1=gcd(a,d), g2=gcd(b,c); A=a/g1, D=d/g1, B=b/g2, C=c/g2.
/// Then gcd(A,B)=gcd(A,D)=gcd(C,B)=gcd(C,D)=1, hence
/// gcd(A*C,B*D)=1 and B*D>0. The 0/1 cases follow since a
/// canonical zero numerator has denominator 1.
///
/// IMPORTANT: malformed noncanonical inputs violate this proof. Do not use
/// this experimental candidate in production without an input invariant.
pub fn mul_canonical_cross_cancelled(lhs: &BigRational, rhs: &BigRational) -> BigRational {
    debug_assert!(is_canonical_fraction(lhs), "noncanonical left operand");
    debug_assert!(is_canonical_fraction(rhs), "noncanonical right operand");
    let g1 = lhs.numer().gcd(rhs.denom());
    let g2 = lhs.denom().gcd(rhs.numer());
    let numerator = (lhs.numer() / &g1) * (rhs.numer() / &g2);
    let denominator = (lhs.denom() / &g2) * (rhs.denom() / &g1);
    BigRational::new_raw(numerator, denominator)
}
