//! modexp for a modulus of one 256-bit digit (the common shape): residues are `U256`s and a
//! modular multiplication is one 512-bit product, one advised quotient and its check
//! (`u256_advice::reduce_product_with_quotient`), with no buffers to allocate or rotate.

use super::bigint::ModexpAdvisor;
use super::exponent::{exponentiate, ModMul};
use crate::system_functions::modexp::strip_leading_zeroes;
use crate::system_functions::u256_advice::{
    reduce_product_with_quotient, verify_wide_div_rem_hint,
};
use alloc::vec::Vec;
use core::alloc::Allocator;
use u256::U256;

struct U256ModMul<'a, Adv: ModexpAdvisor> {
    modulus: &'a U256,
    advisor: &'a mut Adv,
}

impl<Adv: ModexpAdvisor> ModMul for U256ModMul<'_, Adv> {
    type Elem = U256;

    fn placeholder(&mut self) -> U256 {
        U256::zero()
    }

    fn clone_elem(&mut self, x: &U256) -> U256 {
        x.clone()
    }

    fn is_zero(&self, x: &U256) -> bool {
        x.is_zero()
    }

    fn square_assign(&mut self, x: &mut U256) {
        // the product needs the multiplier as a separate operand
        let y = x.clone();
        self.mul_assign(x, &y);
    }

    fn mul_assign(&mut self, x: &mut U256, y: &U256) {
        // the product lands in (x, hi)
        let mut hi = x.clone();
        x.widening_mul_assign_into(&mut hi, y);
        let (q_lo, q_hi) = self.advisor.wide_quotient(x, &hi, self.modulus);
        reduce_product_with_quotient(x, &mut hi, self.modulus, q_lo, q_hi);
    }
}

/// `base^exp mod modulus` as big-endian bytes, for `modulus >= 2`
pub(crate) fn modexp<Adv: ModexpAdvisor, A: Allocator>(
    base: &[u8],
    exp: &[u8],
    modulus: &U256,
    advisor: &mut Adv,
    allocator: A,
) -> Vec<u8, A> {
    debug_assert!(!modulus.is_zero() && !modulus.is_one());

    let base = reduce_base(strip_leading_zeroes(base), modulus, advisor);
    let exp = strip_leading_zeroes(exp);

    let result = if exp.is_empty() {
        // base^0 = 1, including 0^0
        U256::one()
    } else if base.is_zero() || base.is_one() {
        base
    } else {
        let mut arithmetic = U256ModMul { modulus, advisor };
        exponentiate(&mut arithmetic, &base, exp)
    };

    // the multi-digit path's convention: no bytes for zero, the whole digit otherwise (the
    // caller pads or truncates the output to the modulus length)
    let mut out = Vec::with_capacity_in(32, allocator);
    if !result.is_zero() {
        out.extend_from_slice(&result.to_be_bytes());
    }
    out
}

/// `base mod modulus` for a big-endian `base` of any length, digit by digit from the top
/// (Horner): `acc = (acc * 2^256 + digit) mod modulus`, each step a 512-bit division with
/// an advised quotient. A digit already below the modulus with a zero accumulator (a base
/// of at most 32 bytes below the modulus, the common case) needs no advice.
fn reduce_base<Adv: ModexpAdvisor>(base: &[u8], modulus: &U256, advisor: &mut Adv) -> U256 {
    let mut acc = U256::zero();
    let (head, digits) = base.as_rchunks::<32>();
    if !head.is_empty() {
        let mut padded = [0u8; 32];
        padded[32 - head.len()..].copy_from_slice(head);
        reduce_digit(&mut acc, &padded, modulus, advisor);
    }
    for digit in digits {
        reduce_digit(&mut acc, digit, modulus, advisor);
    }
    acc
}

fn reduce_digit<Adv: ModexpAdvisor>(
    acc: &mut U256,
    digit: &[u8; 32],
    modulus: &U256,
    advisor: &mut Adv,
) {
    let mut lo = U256::from_be_bytes(digit);
    if acc.is_zero() {
        // lo < modulus: the subtraction on a scratch borrows
        let mut scratch = lo.clone();
        if scratch.overflowing_sub_assign(modulus) {
            *acc = lo;
            return;
        }
    }
    let (q_lo, q_hi) = advisor.wide_quotient(&lo, acc, modulus);
    assert!(
        verify_wide_div_rem_hint(&mut lo, acc, modulus, &q_lo, &q_hi),
        "modexp base reduction hint: wrong quotient"
    );
    *acc = lo;
}
