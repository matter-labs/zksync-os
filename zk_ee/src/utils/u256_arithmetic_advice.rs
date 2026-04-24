use crate::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_MULMOD_ADVICE_QUERY_ID};
use crate::oracle::usize_serialization::UsizeDeserializable;
use crate::oracle::IOOracle;
use u256::U256;

fn read_u256_from_oracle_response(it: &mut impl ExactSizeIterator<Item = usize>) -> U256 {
    let l0 = <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 0");
    let l1 = <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 1");
    let l2 = <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 2");
    let l3 = <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 3");
    U256::from_limbs([l0, l1, l2, l3])
}

/// Oracle-backed U256 division with remainder.
///
/// Issues `U256_DIV_REM_ADVICE_QUERY_ID` via `IOOracle::raw_query`,
/// reads back (quotient, remainder), verifies `q * d + r == n` with
/// no overflow and `r < d`, then writes results.
///
/// Panics if divisor is zero.
pub fn u256_div_rem_with_advice<O: IOOracle>(
    dividend_or_quotient: &mut U256,
    divisor_or_remainder: &mut U256,
    oracle: &mut O,
) {
    assert!(!divisor_or_remainder.is_zero());

    let input: [u64; 8] = {
        let d = dividend_or_quotient.as_limbs();
        let v = divisor_or_remainder.as_limbs();
        [d[0], d[1], d[2], d[3], v[0], v[1], v[2], v[3]]
    };

    let mut it = oracle
        .raw_query(U256_DIV_REM_ADVICE_QUERY_ID, &input)
        .expect("div_rem oracle query failed");

    let quotient = read_u256_from_oracle_response(&mut it);
    let remainder = read_u256_from_oracle_response(&mut it);

    // Verify: q * d + r == n, no 256-bit overflow, r < d.
    let mut check_lo = quotient.clone();
    let mut check_hi = quotient.clone();
    check_lo.widening_mul_assign_into(&mut check_hi, divisor_or_remainder);

    let carry = check_lo.overflowing_add_assign(&remainder);
    assert!(!carry && check_hi.is_zero());
    assert!(check_lo == *dividend_or_quotient);
    assert!(remainder < *divisor_or_remainder);

    *dividend_or_quotient = quotient;
    *divisor_or_remainder = remainder;
}

/// Oracle-backed U256 mulmod: computes `(a * b) % m`.
///
/// Issues `U256_MULMOD_ADVICE_QUERY_ID` via `IOOracle::raw_query`,
/// reads back (q_lo, q_hi, remainder) where q = q_lo + q_hi * 2^256,
/// verifies `q * m + r == a * b` with no 512-bit overflow and `r < m`,
/// then writes the remainder into `modulus_or_result`.
///
/// Panics if modulus is zero (caller must guard against this).
pub fn u256_mulmod_with_advice<O: IOOracle>(
    a: &mut U256,
    b: &mut U256,
    modulus_or_result: &mut U256,
    oracle: &mut O,
) {
    if modulus_or_result.is_zero() {
        return;
    }

    let input: [u64; 12] = {
        let al = a.as_limbs();
        let bl = b.as_limbs();
        let ml = modulus_or_result.as_limbs();
        [
            al[0], al[1], al[2], al[3], bl[0], bl[1], bl[2], bl[3], ml[0], ml[1], ml[2], ml[3],
        ]
    };

    let mut it = oracle
        .raw_query(U256_MULMOD_ADVICE_QUERY_ID, &input)
        .expect("mulmod oracle query failed");

    let q_lo = read_u256_from_oracle_response(&mut it);
    let q_hi = read_u256_from_oracle_response(&mut it);
    let remainder = read_u256_from_oracle_response(&mut it);

    // Verify: q*m + r == a*b, no 512-bit overflow, r < m.
    //
    // q = q_lo + q_hi * 2^256
    // q*m = q_lo*m + (q_hi*m) << 256
    //
    // (p0_lo, p0_hi) = widening_mul(q_lo, m)
    // (p1_lo, p1_hi) = widening_mul(q_hi, m)
    //
    // q*m + r low 256:  p0_lo + r        (carry c1)
    // q*m + r high 256: p0_hi + p1_lo + c1 (carry c2)
    // overflow:         p1_hi + c2        (must be zero)

    let mut p0_lo = q_lo.clone();
    let mut p0_hi = q_lo.clone();
    p0_lo.widening_mul_assign_into(&mut p0_hi, modulus_or_result);

    let mut p1_lo = q_hi.clone();
    let mut p1_hi = q_hi.clone();
    p1_lo.widening_mul_assign_into(&mut p1_hi, modulus_or_result);

    assert!(p1_hi.is_zero());

    let c1 = p0_lo.overflowing_add_assign(&remainder);
    let c2a = p0_hi.overflowing_add_assign(&p1_lo);
    let c2b =
        p0_hi.overflowing_add_assign_with_carry_propagation(&U256::from_limbs([0, 0, 0, 0]), c1);
    assert!(!(c2a | c2b));

    let mut ab_lo = a.clone();
    let mut ab_hi = a.clone();
    ab_lo.widening_mul_assign_into(&mut ab_hi, b);

    assert!(p0_lo == ab_lo);
    assert!(p0_hi == ab_hi);
    assert!(remainder < *modulus_or_result);

    *modulus_or_result = remainder;
}
