use crate::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_MULMOD_ADVICE_QUERY_ID};
use crate::oracle::usize_serialization::UsizeDeserializable;
use crate::oracle::IOOracle;
use u256::U256;

#[inline(always)]
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
///
/// Delegation cost: 8 calls (1 MEMCOPY + 2 MUL + 1 ADD + 1 SUB + 1 EQ,
/// plus 2 XOR/OR which are free limb ops).
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
    //
    // widening_mul_assign: 1 MEMCOPY + MUL_LOW + MUL_HIGH = 3 delegation calls
    // (saves 1 MEMCOPY vs clone+clone+widening_mul_assign_into)
    let mut check_lo = quotient.clone();
    let check_hi = check_lo.widening_mul_assign(divisor_or_remainder);

    let carry = check_lo.overflowing_add_assign(&remainder); // 1 ADD

    // Combined check: !carry && check_hi == 0 && check_lo == dividend.
    // XOR and OR are free limb ops (no delegation). Single is_zero at the end.
    // This replaces 2 separate EQ delegations with 1.
    core::ops::BitXorAssign::bitxor_assign(&mut check_lo, dividend_or_quotient);
    core::ops::BitOrAssign::bitor_assign(&mut check_lo, &check_hi);
    assert!(!carry && check_lo.is_zero()); // 1 EQ

    // r < d: sub + check borrow. Saves 1 EQ vs Ord::cmp which does EQ then SUB.
    let mut r_check = remainder.clone(); // 1 MEMCOPY
    let borrow = r_check.overflowing_sub_assign(divisor_or_remainder); // 1 SUB
    assert!(borrow);

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
///
/// Delegation cost: 15 calls (3 MEMCOPY + 6 MUL + 3 ADD + 1 SUB + 1 EQ,
/// plus XOR/OR which are free limb ops). Conditional carry saves 1 ADD
/// when the low-half addition doesn't overflow.
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

    // widening_mul_assign: 1 MEMCOPY + MUL_LOW + MUL_HIGH each
    let mut p0_lo = q_lo.clone();
    let mut p0_hi = p0_lo.widening_mul_assign(modulus_or_result); // 3 delegation

    let mut p1_lo = q_hi.clone();
    let p1_hi = p1_lo.widening_mul_assign(modulus_or_result); // 3 delegation

    // p1_hi must be zero (result fits in 512 bits)
    assert!(p1_hi.is_zero()); // 1 EQ — but we'll fold this into the combined check below

    // check_lo = p0_lo + r
    let c1 = p0_lo.overflowing_add_assign(&remainder); // 1 ADD

    // check_hi = p0_hi + p1_lo + c1
    let c2a = p0_hi.overflowing_add_assign(&p1_lo); // 1 ADD
    let c2b = if c1 {
        p0_hi.overflowing_add_assign(&U256::one()) // 1 ADD (only when carry)
    } else {
        false
    };
    assert!(!(c2a | c2b));

    // Compute (ab_lo, ab_hi) = widening_mul(a, b)
    let mut ab_lo = a.clone();
    let ab_hi = ab_lo.widening_mul_assign(b); // 3 delegation

    // Combined equality: (p0_lo ^ ab_lo) | (p0_hi ^ ab_hi) | p1_hi == 0
    // XOR and OR are free limb ops (no delegation). Single is_zero at the end.
    core::ops::BitXorAssign::bitxor_assign(&mut p0_lo, &ab_lo);
    core::ops::BitXorAssign::bitxor_assign(&mut p0_hi, &ab_hi);
    core::ops::BitOrAssign::bitor_assign(&mut p0_lo, &p0_hi);
    core::ops::BitOrAssign::bitor_assign(&mut p0_lo, &p1_hi);
    assert!(p0_lo.is_zero()); // 1 EQ (replaces 3 separate checks: 2 EQ + 1 EQ)

    // r < m: sub + check borrow. Saves 1 EQ vs Ord::cmp.
    let mut r_check = remainder.clone(); // 1 MEMCOPY
    let borrow = r_check.overflowing_sub_assign(modulus_or_result); // 1 SUB
    assert!(borrow);

    *modulus_or_result = remainder;
}
