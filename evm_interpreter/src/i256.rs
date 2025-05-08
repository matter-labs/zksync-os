// Based on https://github.com/bluealloy/revm/blob/main/crates/interpreter/src/instructions/i256.rs

use core::cmp::Ordering;
use ruint::aliases::U256;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Sign {
    Plus,
    Minus,
    Zero,
}

pub const MIN_NEGATIVE_VALUE: U256 = U256::from_limbs([
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x8000000000000000,
]);

const FLIPH_BITMASK_U64: u64 = 0x7FFFFFFFFFFFFFFF;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct I256(pub Sign, pub U256);

#[inline(always)]
pub fn i256_sign<const DO_TWO_COMPL: bool>(val: &mut U256) -> Sign {
    if !val.bit(U256::BITS - 1) {
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
    if !val.bit(U256::BITS - 1) {
        if val.is_zero() {
            Sign::Zero
        } else {
            Sign::Plus
        }
    } else {
        Sign::Minus
    }
}

#[inline(always)]
fn u256_remove_sign(val: &mut U256) {
    unsafe {
        val.as_limbs_mut()[3] &= FLIPH_BITMASK_U64;
    }
}

#[inline(always)]
pub fn two_compl_mut(op: &mut U256) {
    *op = two_compl(*op);
}

pub fn two_compl(op: U256) -> U256 {
    op.wrapping_neg()
}

#[inline(always)]
pub fn i256_cmp(first: &U256, second: &U256) -> Ordering {
    let first_sign = i256_sign_by_ref(first);
    let second_sign = i256_sign_by_ref(second);
    match (first_sign, second_sign) {
        (Sign::Zero, Sign::Zero) => Ordering::Equal,
        (Sign::Zero, Sign::Plus) => Ordering::Less,
        (Sign::Zero, Sign::Minus) => Ordering::Greater,
        (Sign::Minus, Sign::Zero) => Ordering::Less,
        (Sign::Minus, Sign::Plus) => Ordering::Less,
        (Sign::Minus, Sign::Minus) => first.cmp(&second),
        (Sign::Plus, Sign::Minus) => Ordering::Greater,
        (Sign::Plus, Sign::Zero) => Ordering::Greater,
        (Sign::Plus, Sign::Plus) => first.cmp(&second),
    }
}

#[inline(always)]
pub fn i256_div(mut first: U256, mut second: U256) -> U256 {
    let second_sign = i256_sign::<true>(&mut second);
    if second_sign == Sign::Zero {
        return U256::ZERO;
    }
    let first_sign = i256_sign::<true>(&mut first);
    if first_sign == Sign::Minus && first == MIN_NEGATIVE_VALUE && second == U256::ONE {
        return two_compl(MIN_NEGATIVE_VALUE);
    }

    //let mut d = first / second;
    let mut d = first.div_rem(second).0;

    u256_remove_sign(&mut d);
    //set sign bit to zero

    if d.is_zero() {
        return d;
    }

    match (first_sign, second_sign) {
        (Sign::Zero, Sign::Plus)
        | (Sign::Plus, Sign::Zero)
        | (Sign::Zero, Sign::Zero)
        | (Sign::Plus, Sign::Plus)
        | (Sign::Minus, Sign::Minus) => d,
        (Sign::Zero, Sign::Minus)
        | (Sign::Plus, Sign::Minus)
        | (Sign::Minus, Sign::Zero)
        | (Sign::Minus, Sign::Plus) => two_compl(d),
    }
}

#[inline(always)]
pub fn i256_mod(mut first: U256, mut second: U256) -> U256 {
    let first_sign = i256_sign::<true>(&mut first);
    if first_sign == Sign::Zero {
        return U256::ZERO;
    }

    let _ = i256_sign::<true>(&mut second);
    let mut r = first % second;
    u256_remove_sign(&mut r);
    if r.is_zero() {
        return r;
    }
    if first_sign == Sign::Minus {
        two_compl(r)
    } else {
        r
    }
}
