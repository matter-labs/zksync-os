use crate::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_MULMOD_ADVICE_QUERY_ID};
use crate::oracle::usize_serialization::UsizeDeserializable;
use crate::oracle::IOOracle;
use u256::U256;

#[inline(always)]
fn read_limbs_from_oracle_response(it: &mut impl ExactSizeIterator<Item = usize>) -> [u64; 4] {
    [
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 0"),
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 1"),
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 2"),
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 3"),
    ]
}

/// Oracle-backed U256 division with remainder.
///
/// Delegation cost: 5 calls (2 MUL + 1 ADD + 1 SUB + 1 EQ).
/// All temporaries are constructed from raw limbs (zero delegation cost).
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

    let q_limbs = read_limbs_from_oracle_response(&mut it);
    let r_limbs = read_limbs_from_oracle_response(&mut it);

    // Verify: q * d + r == n, no 256-bit overflow, r < d.
    //
    // Construct both halves from raw limbs (free) instead of cloning (1 MEMCOPY each).
    // Use widening_mul_assign_into (2 MUL) instead of widening_mul_assign (1 MEMCOPY + 2 MUL).
    let mut check_lo = U256::from_limbs(q_limbs);
    let mut check_hi = U256::from_limbs(q_limbs);
    check_lo.widening_mul_assign_into(&mut check_hi, divisor_or_remainder); // MUL_LOW + MUL_HIGH

    let remainder = U256::from_limbs(r_limbs);
    let carry = check_lo.overflowing_add_assign(&remainder); // ADD

    // Combined: !carry && check_hi == 0 && check_lo == dividend
    core::ops::BitXorAssign::bitxor_assign(&mut check_lo, dividend_or_quotient);
    core::ops::BitOrAssign::bitor_assign(&mut check_lo, &check_hi);
    assert!(!carry && check_lo.is_zero()); // EQ

    // r < d via subtraction borrow (construct from limbs to avoid clone)
    let mut r_check = U256::from_limbs(r_limbs);
    let borrow = r_check.overflowing_sub_assign(divisor_or_remainder); // SUB
    assert!(borrow);

    // Write output from raw limbs (no delegation — just stack moves)
    *dividend_or_quotient = U256::from_limbs(q_limbs);
    *divisor_or_remainder = remainder;
}

/// Oracle-backed U256 mulmod: computes `(a * b) % m`.
///
/// Delegation cost: 10 calls (6 MUL + 2 ADD + 1 SUB + 1 EQ),
/// plus 1 conditional ADD for carry propagation.
/// All temporaries are constructed from raw limbs (zero delegation cost).
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

    let q_lo_limbs = read_limbs_from_oracle_response(&mut it);
    let q_hi_limbs = read_limbs_from_oracle_response(&mut it);
    let r_limbs = read_limbs_from_oracle_response(&mut it);

    // Verify: q*m + r == a*b, no 512-bit overflow, r < m.
    //
    // All temporaries constructed from raw limbs (free) instead of clone (MEMCOPY).
    // widening_mul_assign_into (2 MUL) instead of widening_mul_assign (3 = MEMCOPY + 2 MUL).

    // (p0_lo, p0_hi) = widening_mul(q_lo, m)
    let mut p0_lo = U256::from_limbs(q_lo_limbs);
    let mut p0_hi = U256::from_limbs(q_lo_limbs);
    p0_lo.widening_mul_assign_into(&mut p0_hi, modulus_or_result); // 2 MUL

    // (p1_lo, p1_hi) = widening_mul(q_hi, m)
    let mut p1_lo = U256::from_limbs(q_hi_limbs);
    let mut p1_hi = U256::from_limbs(q_hi_limbs);
    p1_lo.widening_mul_assign_into(&mut p1_hi, modulus_or_result); // 2 MUL

    // check_lo = p0_lo + r
    let remainder = U256::from_limbs(r_limbs);
    let c1 = p0_lo.overflowing_add_assign(&remainder); // ADD

    // check_hi = p0_hi + p1_lo + c1
    let c2a = p0_hi.overflowing_add_assign(&p1_lo); // ADD
    let c2b = if c1 {
        p0_hi.overflowing_add_assign(&U256::one()) // ADD (conditional)
    } else {
        false
    };
    assert!(!(c2a | c2b));

    // (ab_lo, ab_hi) = widening_mul(a, b) — construct from operand limbs, no clone
    let a_limbs = *a.as_limbs();
    let mut ab_lo = U256::from_limbs(a_limbs);
    let mut ab_hi = U256::from_limbs(a_limbs);
    ab_lo.widening_mul_assign_into(&mut ab_hi, b); // 2 MUL

    // Combined: p1_hi == 0 && p0_lo == ab_lo && p0_hi == ab_hi
    core::ops::BitXorAssign::bitxor_assign(&mut p0_lo, &ab_lo);
    core::ops::BitXorAssign::bitxor_assign(&mut p0_hi, &ab_hi);
    core::ops::BitOrAssign::bitor_assign(&mut p0_lo, &p0_hi);
    core::ops::BitOrAssign::bitor_assign(&mut p0_lo, &p1_hi);
    assert!(p0_lo.is_zero()); // 1 EQ

    // r < m (construct from limbs to avoid clone)
    let mut r_check = U256::from_limbs(r_limbs);
    let borrow = r_check.overflowing_sub_assign(modulus_or_result); // SUB
    assert!(borrow);

    *modulus_or_result = remainder;
}
