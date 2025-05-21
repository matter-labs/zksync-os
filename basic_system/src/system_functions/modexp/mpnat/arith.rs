// Taken from https://github.com/aurora-is-near/aurora-engine, with changes
// to explicitly pass around allocator.

use zk_ee::system::logger::Logger;

use super::{double::U512, mpnat::MPNatU256, U256};
use core::alloc::Allocator;


/// Computes `(x * y) mod 2^(WORD_BITS*out.len())`.
pub fn big_wrapping_mul<L: Logger, A: Allocator + Clone>(l: &mut L, x: &MPNatU256<A>, y: &MPNatU256<A>, out: &mut [U256]) {
    let s = out.len();
    for i in 0..s {
        let mut c: U256 = U256::ZERO;
        for j in 0..(s - i) {
            let (prod, carry) = shifted_carrying_mul(
                l, 
                &out[i + j],
                x.digits.get(j).unwrap_or(&U256::ZERO),
                &y.digits.get(i).unwrap_or(&U256::ZERO),
                &c,
            );
            c = carry;
            out[i + j] = prod;
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
pub fn shifted_carrying_mul<L: Logger>(logger: &mut L, a: &U256, x: &U256, y: &U256, c: &U256) -> (U256, U256) {

    let mut r = U512::from_narrow_mul(logger, x, y);
    r.add_assign_narrow(a);
    r.add_assign_narrow(c);

    r.into_words()
}
