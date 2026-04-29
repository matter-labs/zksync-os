use crate::oracle::query_ids::U256_MULMOD_ADVICE_QUERY_ID;
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

/// Oracle-backed U256 mulmod: computes `(a * b) % m`.
///
/// Delegation cost: 10 calls (6 MUL + 2 ADD + 1 SUB + 1 EQ),
/// plus 1 conditional ADD for carry propagation.
#[inline]
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

    let mut p0_lo = U256::from_limbs(q_lo_limbs);
    let mut p0_hi = U256::from_limbs(q_lo_limbs);
    p0_lo.widening_mul_assign_into(&mut p0_hi, modulus_or_result);

    let mut p1_lo = U256::from_limbs(q_hi_limbs);
    let mut p1_hi = U256::from_limbs(q_hi_limbs);
    p1_lo.widening_mul_assign_into(&mut p1_hi, modulus_or_result);

    let remainder = U256::from_limbs(r_limbs);
    let c1 = p0_lo.overflowing_add_assign(&remainder);

    let c2a = p0_hi.overflowing_add_assign(&p1_lo);
    let c2b = if c1 {
        p0_hi.overflowing_add_assign(&U256::one())
    } else {
        false
    };
    assert!(!(c2a | c2b));

    let a_limbs = *a.as_limbs();
    let mut ab_lo = U256::from_limbs(a_limbs);
    let mut ab_hi = U256::from_limbs(a_limbs);
    ab_lo.widening_mul_assign_into(&mut ab_hi, b);

    core::ops::BitXorAssign::bitxor_assign(&mut p0_lo, &ab_lo);
    core::ops::BitXorAssign::bitxor_assign(&mut p0_hi, &ab_hi);
    core::ops::BitOrAssign::bitor_assign(&mut p0_lo, &p0_hi);
    core::ops::BitOrAssign::bitor_assign(&mut p0_lo, &p1_hi);
    assert!(p0_lo.is_zero());

    let mut r_check = U256::from_limbs(r_limbs);
    let borrow = r_check.overflowing_sub_assign(modulus_or_result);
    assert!(borrow);

    *modulus_or_result = remainder;
}
