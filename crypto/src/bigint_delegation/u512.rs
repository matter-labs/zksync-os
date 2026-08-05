use super::{
    delegation,
    u256::{self, U256},
    DelegatedModParams, DelegatedMontParams,
};
use crate::ark_ff_delegation::{BigInt, BigInteger};

pub(super) type U512 = BigInt<8>;

static ZERO: U256 = U256::zero();
static ONE: U256 = U256::one();

struct ScratchSpace {
    copy_place_0: U512,
    low_word_scratch: U256,
    mul_copy_place_0: U256,
    mul_copy_place_1: U256,
    mul_copy_place_2: U256,
    mul_copy_place_3: U256,
    mul_copy_place_4: U256,
    mul_copy_place_5: U256,
}

#[cfg(not(test))]
static mut SCRATCH_SPACE: ScratchSpace = ScratchSpace {
    copy_place_0: U512::zero(),
    low_word_scratch: U256::zero(),
    mul_copy_place_0: U256::zero(),
    mul_copy_place_1: U256::zero(),
    mul_copy_place_2: U256::zero(),
    mul_copy_place_3: U256::zero(),
    mul_copy_place_4: U256::zero(),
    mul_copy_place_5: U256::zero(),
};

#[cfg(test)]
use std::cell::UnsafeCell;

#[cfg(test)]
thread_local! {
    static SCRATCH_SPACE: UnsafeCell<Box<ScratchSpace>> = UnsafeCell::new(Box::new(ScratchSpace {
        copy_place_0: U512::zero(),
        low_word_scratch: U256::zero(),
        mul_copy_place_0: U256::zero(),
        mul_copy_place_1: U256::zero(),
        mul_copy_place_2: U256::zero(),
        mul_copy_place_3: U256::zero(),
        mul_copy_place_4: U256::zero(),
        mul_copy_place_5: U256::zero(),
    }))
}

#[cfg(test)]
macro_rules! with_scratch {
    ($scratch:ident => $($body:tt)*) => {
        SCRATCH_SPACE.with(|cell| unsafe {
            let $scratch = &mut **cell.get();
            $($body)*
        })
    };
}

#[cfg(not(test))]
macro_rules! with_scratch {
    ($scratch:ident => $($body:tt)*) => {
        unsafe {
            let $scratch = &mut SCRATCH_SPACE;
            $($body)*
        }
    };
}

pub(super) fn as_low(a: &U512) -> &U256 {
    unsafe {
        let ptr = a as *const U512 as *const U256;

        debug_assert_eq!(ptr.addr() % 32, 0);

        ptr.as_ref().unwrap()
    }
}

pub(super) fn as_high(a: &U512) -> &U256 {
    unsafe {
        let ptr = (a as *const U512 as *const U256).add(1);

        debug_assert_eq!(ptr.addr() % 32, 0);

        ptr.as_ref().unwrap()
    }
}

pub(super) fn as_low_high_mut(a: &mut U512) -> (&mut U256, &mut U256) {
    unsafe {
        let low = a as *mut U512 as *mut U256;
        let high = (a as *mut U512 as *mut U256).add(1);

        // check alignment for U256
        debug_assert_eq!(low.addr() % 32, 0);
        debug_assert_eq!(high.addr() % 32, 0);

        (low.as_mut().unwrap(), high.as_mut().unwrap())
    }
}

fn copy(dst: &mut U512, src: &U512) {
    let (low_dst, high_dst) = as_low_high_mut(dst);

    delegation::memcpy(low_dst, as_low(src));
    delegation::memcpy(high_dst, as_high(src));
}

/// Tries to get `self` in the range `[0..modulus)`.
/// Note: we assume `self < 2*modulus`, otherwise the result might not be in the range
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
unsafe fn sub_mod_with_carry<T: DelegatedModParams<8>>(a: &mut U512, carry: bool) {
    let (low, high) = as_low_high_mut(a);

    let borrow = u256::sub_assign(low, as_low(T::modulus()));
    let borrow = u256::sub_with_carry_bit(high, as_high(T::modulus()), borrow);

    if borrow & !carry {
        let carry = u256::add_assign(low, as_low(T::modulus()));
        u256::add_with_carry_bit(high, as_high(T::modulus()), carry);
    }
}

/// Computes `self = self + rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn add_mod_assign<T: DelegatedModParams<8>>(a: &mut U512, b: &U512) {
    let (low, high) = as_low_high_mut(a);

    let carry = u256::add_assign(low, as_low(b));
    let carry = u256::add_with_carry_bit(high, as_high(b), carry);

    sub_mod_with_carry::<T>(a, carry);
}

/// Computes `self = self - rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn sub_mod_assign<T: DelegatedModParams<8>>(a: &mut U512, b: &U512) {
    let (low, high) = as_low_high_mut(a);

    let borrow = u256::sub_assign(low, as_low(b));
    let borrow = u256::sub_with_carry_bit(high, as_high(b), borrow);

    if borrow {
        let carry = u256::add_assign(low, as_low(T::modulus()));
        u256::add_with_carry_bit(high, as_high(T::modulus()), carry);
    }
}

/// Computes `self = self + self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn double_mod_assign<T: DelegatedModParams<8>>(a: &mut U512) {
    with_scratch!(s => {
        copy(&mut s.copy_place_0, a);
        add_mod_assign::<T>(a, &s.copy_place_0);
    })
}

/// Computes `self = -self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn neg_mod_assign<T: DelegatedModParams<8>>(a: &mut U512) {
    let (low, high) = as_low_high_mut(a);

    let is_low_zero = delegation::eq(low, &ZERO) != 0;
    let is_high_zero = delegation::eq(high, &ZERO) != 0;

    if !is_low_zero || !is_high_zero {
        let borrow = u256::sub_and_negate_assign(low, as_low(T::modulus()));
        u256::sub_and_negate_with_carry(high, as_high(T::modulus()), borrow);
    }
}

/// Compute `self = self * rhs mod modulus` using montgomery reduction.
/// Both `self` and `rhs` are assumed to be in montgomery form.
/// The reduction constant is expected to be `-1/modulus mod 2^256`
///
/// Note: we assume that modulus is strictly less than 512 bits
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn mul_assign_montgomery<T: DelegatedMontParams<8>>(a: &mut U512, b: &U512) {
    // otherwise we may get a carry in the final addition
    assert!(T::MODULUS_BITSIZE < 512);

    with_scratch!(s => {
        let (r0, r1) = {
            let b0 = as_low(b);
            let r0 = &mut s.mul_copy_place_0;
            delegation::memcpy(r0, as_low(a));

            let carry_1 = &mut s.mul_copy_place_1;
            delegation::memcpy(carry_1, r0);

            u256::mul_low_assign(r0, b0);
            u256::mul_high_assign(carry_1, b0);

            let reduction_k = &mut s.mul_copy_place_2;
            delegation::memcpy(reduction_k, r0);
            u256::mul_low_assign(reduction_k, T::reduction_const());

            let carry_2_low = &mut s.mul_copy_place_3;
            delegation::memcpy(carry_2_low, as_low(T::modulus()));

            u256::mul_low_assign(carry_2_low, reduction_k);
            let of = u256::add_assign(carry_2_low, r0);

            let carry_2 = &mut s.mul_copy_place_4;
            delegation::memcpy(carry_2, as_low(T::modulus()));

            u256::mul_high_assign(carry_2, reduction_k);

            if of {
                u256::add_assign(carry_2, &ONE);
            }

            // We can reuse mul_copy_place_3
            debug_assert!(carry_2_low.is_zero());

            let r1 = &mut s.mul_copy_place_3;
            delegation::memcpy(r1, as_high(a));

            let new_carry_1 = &mut s.mul_copy_place_5;
            delegation::memcpy(new_carry_1, r1);

            u256::mul_low_assign(r1, b0);
            let of = u256::add_assign(r1, carry_1);

            u256::mul_high_assign(new_carry_1, b0);

            if of {
                u256::add_assign(new_carry_1, &ONE);
            }

            // now mul_copy_place_1 is available
            let carry_1 = new_carry_1;

            let new_carry_2_low = &mut s.mul_copy_place_1;
            delegation::memcpy(new_carry_2_low, as_high(T::modulus()));

            u256::mul_low_assign(new_carry_2_low, reduction_k);
            let of0 = u256::add_assign(new_carry_2_low, r1);
            let of1 = u256::add_assign(new_carry_2_low, carry_2);

            // we can reuse mul_copy_place_4 now
            let new_carry_2 = &mut s.mul_copy_place_4;
            delegation::memcpy(new_carry_2, as_high(T::modulus()));

            u256::mul_high_assign(new_carry_2, reduction_k);

            if of0 || of1 {
                let temp = &mut s.low_word_scratch;
                temp.0[0] = of0 as u64 + of1 as u64;
                u256::add_assign(new_carry_2, temp);
            }

            let r0 = new_carry_2_low;
            let carry_2 = new_carry_2;

            let r1 = carry_1;
            u256::add_assign(r1, carry_2);

            debug_assert!(r1.0[2..4].iter().all(|&x| x == 0));

            // we use mul_copy_place_1 and mul_copy_place_5
            (r0, r1)
        };

        let b1 = as_high(b);

        let new_r0 = &mut s.mul_copy_place_0;
        delegation::memcpy(new_r0, as_low(a));

        let carry_1 = &mut s.mul_copy_place_2;
        delegation::memcpy(carry_1, new_r0);

        u256::mul_low_assign(new_r0, b1);
        let of = u256::add_assign(new_r0, r0);
        u256::mul_high_assign(carry_1, b1);
        if of {
            u256::add_assign(carry_1, &ONE);
        }
        // mul_copy_place_1 is free
        let r0 = new_r0;

        let reduction_k = &mut s.mul_copy_place_1;
        delegation::memcpy(reduction_k, r0);

        u256::mul_low_assign(reduction_k, T::reduction_const());

        let carry_2_low = &mut s.mul_copy_place_3;
        delegation::memcpy(carry_2_low, as_low(T::modulus()));

        u256::mul_low_assign(carry_2_low, reduction_k);
        let of = u256::add_assign(carry_2_low, r0);

        let carry_2 = &mut s.mul_copy_place_4;
        delegation::memcpy(carry_2, as_low(T::modulus()));

        u256::mul_high_assign(carry_2, reduction_k);

        if of {
            u256::add_assign(carry_2, &ONE);
        }

        // mul_copy_place_3 is free
        debug_assert!(carry_2_low.is_zero());

        let new_r1 = &mut s.mul_copy_place_3;
        delegation::memcpy(new_r1, as_high(a));

        u256::mul_low_assign(new_r1, b1);
        let of0 = u256::add_assign(new_r1, carry_1);
        let of1 = u256::add_assign(new_r1, r1);

        let (a0, a1) = as_low_high_mut(a);
        u256::mul_high_assign(a1, b1);

        if of0 || of1 {
            let temp = &mut s.low_word_scratch;
            temp.0[0] = of0 as u64 + of1 as u64;
            u256::add_assign(a1, temp);
        }

        // mul_copy_place_5 is free
        let r1 = new_r1;

        delegation::memcpy(a0, as_high(T::modulus()));
        u256::mul_low_assign(a0, reduction_k);

        let of0 = u256::add_assign(a0, r1);
        let of1 = u256::add_assign(a0, carry_2);

        let new_carry_2 = &mut s.mul_copy_place_4;
        delegation::memcpy(new_carry_2, as_high(T::modulus()));

        u256::mul_high_assign(new_carry_2, reduction_k);

        if of0 || of1 {
            let temp = &mut s.low_word_scratch;
            temp.0[0] = of0 as u64 + of1 as u64;
            u256::add_assign(new_carry_2, temp);
        }

        let carry2 = new_carry_2;

        let carry = u256::add_assign(a1, carry2);
        // we can't have a carry since MODULUS_BITSIZE < 512
        debug_assert!(!carry);

        let borrow = u256::sub_assign(a0, as_low(T::modulus()));
        let borrow = u256::sub_with_carry_bit(a1, as_high(T::modulus()), borrow);
        if borrow {
            let carry = u256::add_assign(a0, as_low(T::modulus()));
            let _ = u256::add_with_carry_bit(a1, as_high(T::modulus()), carry);
        }

        debug_assert!(a.0[6..8].iter().all(|&x| x == 0));
    })
}

/// Compute `self = self^2 mod modulus` using montgomery reduction.
/// `self` should be in montgomery form.
/// The reduction constant is expected to be `-1/modulus mod 2^256`
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_montgomery<T: DelegatedMontParams<8>>(a: &mut U512) {
    with_scratch!(s => {
        copy(&mut s.copy_place_0, a);
        mul_assign_montgomery::<T>(a, &s.copy_place_0);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ark_ff_delegation::BigInt;

    // A simple modulus for testing: use a large prime in U512 form
    // We use BLS12-381 Fq modulus padded to 8 limbs
    #[derive(Default, Debug)]
    struct TestMod;

    static TEST_MODULUS: BigInt<8> = BigInt([
        13402431016077863595u64,
        2210141511517208575u64,
        7435674573564081700u64,
        7239337960414712511u64,
        5412103778470702295u64,
        1873798617647539866u64,
        0,
        0,
    ]);

    impl DelegatedModParams<8> for TestMod {
        const MODULUS_BITSIZE: usize = 381;

        fn modulus() -> &'static BigInt<8> {
            &TEST_MODULUS
        }
    }

    #[test]
    fn test_neg_mod_zero() {
        // Negating zero should give zero
        let mut a = U512::zero();
        unsafe {
            neg_mod_assign::<TestMod>(&mut a);
        }
        assert!(a.is_zero(), "neg(0) should be 0");
    }

    #[test]
    fn test_neg_mod_with_only_low_nonzero() {
        // Regression test: when only low part is non-zero, negation should still work
        let mut a = U512::zero();
        a.0[0] = 1; // low part is non-zero, high part is zero

        let original = a;
        unsafe {
            neg_mod_assign::<TestMod>(&mut a);
        }

        // Result should NOT be the original (unless original was 0, which it isn't)
        assert!(!a.is_zero(), "neg(1) should not be 0");
        assert_ne!(a.0, original.0, "neg(a) should differ from a when a != 0");

        // Verify: neg(a) + a should equal modulus (in non-reduced form) or 0 mod modulus
        // Actually let's verify by negating twice
        unsafe {
            neg_mod_assign::<TestMod>(&mut a);
        }
        assert_eq!(a.0, original.0, "neg(neg(a)) should equal a");
    }

    #[test]
    fn test_neg_mod_with_only_high_nonzero() {
        // Regression test: when only high part is non-zero, negation should still work
        let mut a = U512::zero();
        a.0[4] = 1; // high part is non-zero (limb index 4 is in the high U256), low part is zero

        let original = a;
        unsafe {
            neg_mod_assign::<TestMod>(&mut a);
        }

        assert!(!a.is_zero(), "neg(a) should not be 0 when a != 0");
        assert_ne!(a.0, original.0, "neg(a) should differ from a when a != 0");

        // Verify by negating twice
        unsafe {
            neg_mod_assign::<TestMod>(&mut a);
        }
        assert_eq!(a.0, original.0, "neg(neg(a)) should equal a");
    }

    #[test]
    fn test_neg_mod_both_parts_nonzero() {
        // When both parts are non-zero
        let mut a = U512::zero();
        a.0[0] = 42; // low part non-zero
        a.0[4] = 17; // high part non-zero

        let original = a;
        unsafe {
            neg_mod_assign::<TestMod>(&mut a);
        }

        assert!(!a.is_zero(), "neg(a) should not be 0 when a != 0");

        // Verify by negating twice
        unsafe {
            neg_mod_assign::<TestMod>(&mut a);
        }
        assert_eq!(a.0, original.0, "neg(neg(a)) should equal a");
    }
}
