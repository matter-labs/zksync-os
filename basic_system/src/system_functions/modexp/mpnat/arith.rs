// Adopted from https://github.com/aurora-is-near/aurora-engine

use zk_ee::system::logger::Logger;

use super::{double::U512, mpnat::{MPNatU256, U256_ZERO}, U256};
use core::{alloc::Allocator, mem::MaybeUninit};

static mut U512_SCRATCH: U512 = U512::zero_const();
static mut ZERO: Option<U256> = None;
static mut ONE: Option<U256> = None;

/// Computes `(x * y) mod 2^(WORD_BITS*out.len())`.
/// x and y can't reference RO memory.
pub unsafe  fn big_wrapping_mul<L: Logger>(
    l: &mut L,
    x: &[U256],
    y: &[U256],
    out: &mut [U256]
) {
    let zero = unsafe { ZERO.get_or_insert_with(|| U256::ZERO) };
    let one = unsafe { ONE.get_or_insert_with(|| U256::ONE) };
    let s = out.len();
    let mut double = unsafe { &mut U512_SCRATCH };
    let mut c = MaybeUninit::uninit();
    for i in 0..s {
        unsafe { zero.clone_into_unchecked(c.as_mut_ptr() as *mut _) };
        let c = unsafe { c.assume_init_mut() };
        for j in 0..(s - i) {
            let x = match x.get(j) {
                Some(x) => x,
                None => &zero,
            };
            unsafe { shifted_carrying_mul(
                l, 
                &out[i + j],
                &x,
                y.get(i).unwrap_or(&zero),
                &c,
                double,
                one,
            ) };
            c.clone_from(double.high());
            out[i + j].clone_from(double.low());
        }
    }
}

// Performs a += b, returning if there was overflow
pub fn in_place_add(a: &mut [U256], b: &[U256]) -> bool {
    debug_assert!(a.len() == b.len());

    let mut c = false;
    for (a_digit, b_digit) in a.iter_mut().zip(b) {
        let carry = a_digit.overflowing_add_assign_with_carry_propagation(&b_digit, c);
        c = carry;
    }

    c
}

/// Computes `a + xy + c` where any overflow is captured as the "carry",
/// the second part of the output. The arithmetic in this function is
/// guaranteed to never overflow because even when all 4 variables are
/// equal to `Word::MAX` the output is smaller than `DoubleWord::MAX`.
/// Safety: `x`,`y` can't be placed in RO memory.
unsafe fn shifted_carrying_mul<L: Logger>(logger: &mut L, a: &U256, x: &U256, y: &U256, c: &U256, out: &mut U512, one: &U256) {

    {
        let out = &mut out.0;
        let out = unsafe { core::mem::transmute(out) };

        unsafe { U512::from_narrow_mul_into(logger, x, y, out) };
    }

    out.add_assign_narrow(a, one);
    out.add_assign_narrow(c, one);
}

fn widening_mul<L: Logger>(logger: &mut L, lhs: &U256, rhs: &U256, out: &mut [&mut U256; 2]) {
    *out[0] = lhs.clone();
    *out[1] = lhs.clone();

    let r = out;

    let (r1, r2) = r.split_at_mut(1);

    r1[0].widening_mul_assign_into(&mut r2[0], rhs);
}

pub(crate) fn widening_add(lhs: &mut [&mut U256; 2], rhs: &U256) -> bool {
    let carry = lhs[0].overflowing_add_assign(rhs);

    match carry {
        true => lhs[1].overflowing_add_assign(&U256::ONE),
        false => false
    }
}
