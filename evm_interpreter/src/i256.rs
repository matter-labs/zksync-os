// Based on https://github.com/bluealloy/revm/blob/main/crates/interpreter/src/instructions/i256.rs

use core::cmp::Ordering;
use u256::U256;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Sign {
    Plus,
    Minus,
    Zero,
}

const MIN_NEGATIVE_VALUE_REPR: [u64; 4] = [
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x8000000000000000,
];

#[inline(always)]
pub fn i256_sign<const DO_TWO_COMPL: bool>(val: &mut U256) -> Sign {
    if val.as_limbs()[3] >> 63 == 0 {
        if val.is_zero() {
            Sign::Zero
        } else {
            Sign::Plus
        }
    } else {
        if DO_TWO_COMPL {
            two_compl_mut(val);
        }
        Sign::Minus
    }
}

#[inline(always)]
pub fn i256_sign_by_ref(val: &U256) -> Sign {
    if val.as_limbs()[3] >> 63 == 0 {
        if val.is_zero() {
            Sign::Zero
        } else {
            Sign::Plus
        }
    } else {
        Sign::Minus
    }
}

// #[inline(always)]
// fn u256_remove_sign(val: &mut U256) {
//     const FLIPH_BITMASK_U64: u64 = 0x7FFFFFFFFFFFFFFF;

//     val.as_limbs_mut()[3] &= FLIPH_BITMASK_U64;
// }

#[inline(always)]
pub fn two_compl_mut(op: &mut U256) {
    // we can just subtract it from 0
    op.overflowing_sub_assign_reversed(&U256::zero());
}

#[inline(always)]
pub fn i256_cmp(first: &U256, second: &U256) -> Ordering {
    let first_sign = first.bit(255);
    let second_sign = second.bit(255);
    match (first_sign, second_sign) {
        (true, false) => Ordering::Less,    // negative < positive,
        (false, true) => Ordering::Greater, // positive > negative,
        _ => {
            // same sign, trivial for both positives
            // in two complements min negative value is < -1 if viewed as unsigned bit patterns, so we can perform same ops
            let mut tmp = first.clone();
            let uf = tmp.overflowing_sub_assign(&second);
            if uf {
                Ordering::Less
            } else {
                if tmp.is_zero() {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            }
        }
    }
}

#[inline(always)]
pub fn i256_div(dividend: &mut U256, divisor_or_quotient: &mut U256) {
    let divisor_sign = i256_sign::<true>(divisor_or_quotient);
    if divisor_sign == Sign::Zero {
        U256::write_zero(divisor_or_quotient);
        return;
    }

    let dividend_sign = i256_sign::<true>(dividend);
    if dividend_sign == Sign::Minus
        && *dividend.as_limbs() == MIN_NEGATIVE_VALUE_REPR
        && divisor_or_quotient.is_one()
    {
        // it's signed division overflow
        U256::write_zero(divisor_or_quotient);
        divisor_or_quotient.as_limbs_mut()[3] = 0x80000000_00000000;
        two_compl_mut(divisor_or_quotient);
        return;
    }

    // this is unsigned division of moduluses
    U256::div_rem(dividend, divisor_or_quotient);

    // u256_remove_sign(dividend);
    // set sign bit to zero

    if dividend.is_zero() {
        U256::write_zero(divisor_or_quotient);
        return;
    } else {
        match (dividend_sign, divisor_sign) {
            (Sign::Zero, Sign::Plus)
            | (Sign::Plus, Sign::Zero)
            | (Sign::Zero, Sign::Zero)
            | (Sign::Plus, Sign::Plus)
            | (Sign::Minus, Sign::Minus) => {
                // no extra manipulation required
                Clone::clone_from(divisor_or_quotient, &dividend);
            }
            (Sign::Zero, Sign::Minus)
            | (Sign::Plus, Sign::Minus)
            | (Sign::Minus, Sign::Zero)
            | (Sign::Minus, Sign::Plus) => {
                U256::write_zero(divisor_or_quotient);
                divisor_or_quotient.overflowing_sub_assign(&dividend);
            }
        }
    }
}

#[inline(always)]
pub fn i256_mod(dividend: &mut U256, divisor_or_remainder: &mut U256) {
    let dividend_sign = i256_sign::<true>(dividend);
    if dividend_sign == Sign::Zero {
        U256::write_zero(divisor_or_remainder);
        return;
    }

    let _ = i256_sign::<true>(divisor_or_remainder);

    // this is unsigned division of moduluses
    U256::div_rem(dividend, divisor_or_remainder);

    // u256_remove_sign(divisor_or_remainder);

    if divisor_or_remainder.is_zero() {
        return;
    }
    if dividend_sign == Sign::Minus {
        two_compl_mut(divisor_or_remainder);
    }
}
