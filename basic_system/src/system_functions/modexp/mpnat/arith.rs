// Taken from https://github.com/aurora-is-near/aurora-engine, with changes
// to explicitly pass around allocator.

use zk_ee::system::logger::Logger;

use super::{double::U512, mpnat::MPNatU256, U256};
use core::alloc::Allocator;

static mut U512_SCRATCH: U512 = U512::zero_const();

/// Computes `(x * y) mod 2^(WORD_BITS*out.len())`.
pub fn big_wrapping_mul<L: Logger>(
    l: &mut L,
    x: &[U256],
    y: &[U256],
    out: &mut [U256]
) {
    let s = out.len();
    let mut double = unsafe { &mut U512_SCRATCH };
    let mut zero_x;
    for i in 0..s {
        let mut c: U256 = U256::ZERO;
        for j in 0..(s - i) {
            let x = match x.get(j) {
                Some(x) => x,
                None => {
                    zero_x = U256::zero();
                    &zero_x
                },
            };
            shifted_carrying_mul(
                l, 
                &out[i + j],
                &x,
                y.get(i).unwrap_or(&U256::ZERO),
                &c,
                double,
            );
            c = double.high().clone();
            out[i + j] = double.low().clone();
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
fn shifted_carrying_mul<L: Logger>(logger: &mut L, a: &U256, x: &U256, y: &U256, c: &U256, out: &mut U512) {

    {
        let out = &mut out.0;
        let out = unsafe { core::mem::transmute(out) };

        U512::from_narrow_mul_into(logger, x, y, out);
    }

    out.add_assign_narrow(a);
    out.add_assign_narrow(c);
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
