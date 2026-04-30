use crate::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_MULMOD_ADVICE_QUERY_ID};
use crate::oracle::usize_serialization::UsizeDeserializable;
use crate::oracle::IOOracle;
use u256::U256;

/// Params for U256 div_rem oracle query (pointer-based, like modexp).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256DivRemAdviceParamsGeneric<W> {
    pub dividend_ptr: W,
    pub divisor_ptr: W,
}

pub type U256DivRemAdviceParams = U256DivRemAdviceParamsGeneric<u32>;
pub type U256DivRemAdviceParams64 = U256DivRemAdviceParamsGeneric<u64>;

/// Params for U256 mulmod oracle query (pointer-based, like modexp).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256MulmodAdviceParamsGeneric<W> {
    pub a_ptr: W,
    pub b_ptr: W,
    pub modulus_ptr: W,
}

pub type U256MulmodAdviceParams = U256MulmodAdviceParamsGeneric<u32>;
pub type U256MulmodAdviceParams64 = U256MulmodAdviceParamsGeneric<u64>;

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
#[inline]
pub fn u256_div_rem_with_advice<O: IOOracle>(
    dividend_or_quotient: &mut U256,
    divisor_or_remainder: &mut U256,
    oracle: &mut O,
) {
    assert!(!divisor_or_remainder.is_zero());

    #[cfg(target_pointer_width = "32")]
    let mut it = {
        let params = U256DivRemAdviceParams {
            dividend_ptr: (dividend_or_quotient as *const U256).addr() as u32,
            divisor_ptr: (divisor_or_remainder as *const U256).addr() as u32,
        };
        oracle
            .raw_query(
                U256_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256DivRemAdviceParams).addr() as u32),
            )
            .expect("div_rem oracle query failed")
    };

    #[cfg(target_pointer_width = "64")]
    let mut it = {
        let params = U256DivRemAdviceParams64 {
            dividend_ptr: (dividend_or_quotient as *const U256).addr() as u64,
            divisor_ptr: (divisor_or_remainder as *const U256).addr() as u64,
        };
        oracle
            .raw_query(
                U256_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256DivRemAdviceParams64).addr() as u64),
            )
            .expect("div_rem oracle query failed")
    };

    let q_limbs = read_limbs_from_oracle_response(&mut it);
    let r_limbs = read_limbs_from_oracle_response(&mut it);

    let mut check_lo = U256::from_limbs(q_limbs);
    let mut check_hi = U256::from_limbs(q_limbs);
    check_lo.widening_mul_assign_into(&mut check_hi, divisor_or_remainder);

    let remainder = U256::from_limbs(r_limbs);
    let carry = check_lo.overflowing_add_assign(&remainder);

    core::ops::BitXorAssign::bitxor_assign(&mut check_lo, dividend_or_quotient);
    core::ops::BitOrAssign::bitor_assign(&mut check_lo, &check_hi);
    assert!(!carry && check_lo.is_zero());

    let mut r_check = U256::from_limbs(r_limbs);
    let borrow = r_check.overflowing_sub_assign(divisor_or_remainder);
    assert!(borrow);

    *dividend_or_quotient = U256::from_limbs(q_limbs);
    *divisor_or_remainder = remainder;
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

    #[cfg(target_pointer_width = "32")]
    let mut it = {
        let params = U256MulmodAdviceParams {
            a_ptr: (a as *const U256).addr() as u32,
            b_ptr: (b as *const U256).addr() as u32,
            modulus_ptr: (modulus_or_result as *const U256).addr() as u32,
        };
        oracle
            .raw_query(
                U256_MULMOD_ADVICE_QUERY_ID,
                &((&params as *const U256MulmodAdviceParams).addr() as u32),
            )
            .expect("mulmod oracle query failed")
    };

    #[cfg(target_pointer_width = "64")]
    let mut it = {
        let params = U256MulmodAdviceParams64 {
            a_ptr: (a as *const U256).addr() as u64,
            b_ptr: (b as *const U256).addr() as u64,
            modulus_ptr: (modulus_or_result as *const U256).addr() as u64,
        };
        oracle
            .raw_query(
                U256_MULMOD_ADVICE_QUERY_ID,
                &((&params as *const U256MulmodAdviceParams64).addr() as u64),
            )
            .expect("mulmod oracle query failed")
    };

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
