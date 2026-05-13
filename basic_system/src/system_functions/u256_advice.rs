use u256::U256;
use zk_ee::oracle::query_ids::{U256_DIV_REM_ADVICE_QUERY_ID, U256_WIDE_DIV_REM_ADVICE_QUERY_ID};
use zk_ee::oracle::usize_serialization::UsizeDeserializable;
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::{DivRemExt, WideDivRemExt};
#[cfg(target_pointer_width = "32")]
use zk_ee::utils::u256_arithmetic_advice::U256DivRemAdviceParams;
#[cfg(target_pointer_width = "64")]
use zk_ee::utils::u256_arithmetic_advice::U256DivRemAdviceParams64;

#[must_use]
pub fn verify_div_rem_hint(dividend: &U256, divisor: &U256, q_limbs: [u64; 4]) -> Option<U256> {
    let mut prod_lo = U256::from_limbs(q_limbs);
    let mut prod_hi = U256::from_limbs(q_limbs);
    prod_lo.widening_mul_assign_into(&mut prod_hi, divisor);

    if !prod_hi.is_zero() {
        return None;
    }

    // r = dividend - q * divisor
    let mut remainder = dividend.clone();
    let borrow = remainder.overflowing_sub_assign(&prod_lo);
    if borrow {
        return None;
    }

    if remainder >= *divisor {
        return None;
    }

    Some(remainder)
}

#[must_use]
pub fn verify_wide_div_rem_hint(
    dividend_lo: &U256,
    dividend_hi: &U256,
    divisor: &U256,
    q_lo_limbs: [u64; 4],
    q_hi_limbs: [u64; 4],
) -> Option<U256> {
    // Compute q * divisor as 512-bit
    let mut qd_lo = U256::from_limbs(q_lo_limbs);
    let mut qd_mid = U256::from_limbs(q_lo_limbs);
    qd_lo.widening_mul_assign_into(&mut qd_mid, divisor);

    let mut qd_hi_lo = U256::from_limbs(q_hi_limbs);
    let mut qd_hi_hi = U256::from_limbs(q_hi_limbs);
    qd_hi_lo.widening_mul_assign_into(&mut qd_hi_hi, divisor);

    // Accumulate into (qd_lo, qd_mid) — the 512-bit product q*d
    let c1 = qd_mid.overflowing_add_assign(&qd_hi_lo);
    if c1 || !qd_hi_hi.is_zero() {
        return None;
    }

    // Compute r = dividend - q*d (512-bit subtraction)
    let mut r_lo = dividend_lo.clone();
    let borrow_lo = r_lo.overflowing_sub_assign(&qd_lo);
    let mut r_hi = dividend_hi.clone();
    let borrow_mid = r_hi.overflowing_sub_assign(&qd_mid);
    let borrow_final = if borrow_lo {
        r_hi.overflowing_sub_assign(&U256::one())
    } else {
        false
    };

    // r must be non-negative (no borrow) and fit in 256 bits (r_hi == 0)
    if (borrow_mid | borrow_final) || !r_hi.is_zero() {
        return None;
    }

    if r_lo >= *divisor {
        return None;
    }

    Some(r_lo)
}

pub struct DivRemImpl<const USE_ADVICE: bool>;
pub struct WideDivRemImpl<const USE_ADVICE: bool>;

impl<const USE_ADVICE: bool> DivRemExt for DivRemImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(
        dividend_or_quotient: &mut U256,
        divisor_or_remainder: &mut U256,
        oracle: &mut O,
    ) {
        if USE_ADVICE {
            u256_div_rem_with_advice(dividend_or_quotient, divisor_or_remainder, oracle)
        } else {
            U256::div_rem(dividend_or_quotient, divisor_or_remainder)
        }
    }
}

impl<const USE_ADVICE: bool> WideDivRemExt for WideDivRemImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(
        dividend_lo: &mut U256,
        dividend_hi: &mut U256,
        divisor: &mut U256,
        oracle: &mut O,
    ) {
        if USE_ADVICE {
            u256_wide_div_rem_with_advice(dividend_lo, dividend_hi, divisor, oracle)
        } else {
            u256_wide_div_rem_software(dividend_lo, dividend_hi, divisor)
        }
    }
}

#[inline(always)]
fn read_limbs_from_oracle_response(it: &mut impl ExactSizeIterator<Item = usize>) -> [u64; 4] {
    [
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 0"),
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 1"),
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 2"),
        <u64 as UsizeDeserializable>::from_iter(it).expect("u256 limb 3"),
    ]
}

#[inline]
pub fn u256_div_rem_with_advice<O: IOOracle>(
    dividend_or_quotient: &mut U256,
    divisor_or_remainder: &mut U256,
    oracle: &mut O,
) {
    assert!(!divisor_or_remainder.is_zero());

    if *dividend_or_quotient < *divisor_or_remainder {
        let remainder = core::mem::replace(dividend_or_quotient, U256::zero());
        *divisor_or_remainder = remainder;
        return;
    }

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

    let remainder = verify_div_rem_hint(dividend_or_quotient, divisor_or_remainder, q_limbs)
        .expect("div_rem hint verification failed");

    *dividend_or_quotient = U256::from_limbs(q_limbs);
    *divisor_or_remainder = remainder;
}

fn u256_wide_div_rem_software(dividend_lo: &mut U256, dividend_hi: &mut U256, divisor: &mut U256) {
    assert!(!divisor.is_zero());
    let mut product = [0u64; 8];
    product[..4].copy_from_slice(dividend_lo.as_limbs());
    product[4..].copy_from_slice(dividend_hi.as_limbs());
    let mut d = *divisor.as_limbs();
    ruint::algorithms::div(&mut product, &mut d);
    // Remainder is in d, store it in divisor
    *divisor = U256::from_limbs(d);
}

#[inline]
fn u256_wide_div_rem_with_advice<O: IOOracle>(
    dividend_lo: &mut U256,
    dividend_hi: &mut U256,
    divisor: &mut U256,
    oracle: &mut O,
) {
    assert!(!divisor.is_zero());

    #[cfg(target_pointer_width = "32")]
    let mut it = {
        use zk_ee::utils::u256_arithmetic_advice::U256WideDivRemAdviceParams;
        let params = U256WideDivRemAdviceParams {
            dividend_lo_ptr: (dividend_lo as *const U256).addr() as u32,
            dividend_hi_ptr: (dividend_hi as *const U256).addr() as u32,
            divisor_ptr: (divisor as *const U256).addr() as u32,
        };
        oracle
            .raw_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256WideDivRemAdviceParams).addr() as u32),
            )
            .expect("wide_div_rem oracle query failed")
    };

    #[cfg(target_pointer_width = "64")]
    let mut it = {
        use zk_ee::utils::u256_arithmetic_advice::U256WideDivRemAdviceParams64;
        let params = U256WideDivRemAdviceParams64 {
            dividend_lo_ptr: (dividend_lo as *const U256).addr() as u64,
            dividend_hi_ptr: (dividend_hi as *const U256).addr() as u64,
            divisor_ptr: (divisor as *const U256).addr() as u64,
        };
        oracle
            .raw_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256WideDivRemAdviceParams64).addr() as u64),
            )
            .expect("wide_div_rem oracle query failed")
    };

    let q_lo_limbs = read_limbs_from_oracle_response(&mut it);
    let q_hi_limbs = read_limbs_from_oracle_response(&mut it);

    let remainder =
        verify_wide_div_rem_hint(dividend_lo, dividend_hi, divisor, q_lo_limbs, q_hi_limbs)
            .expect("wide_div_rem hint verification failed");

    *divisor = remainder;
}
