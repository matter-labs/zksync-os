use core::{borrow, mem::MaybeUninit};

use super::{
    delegation,
    u256::{self, U256},
    DelegatedModParams, DelegatedMontParams,
};
use crate::ark_ff_delegation::{BigInt, BigInteger};

pub(super) type U512 = BigInt<8>;

pub fn init() {
    unsafe {
        ZERO.as_mut_ptr().write(U256::zero());
        ONE.as_mut_ptr().write(U256::one());
        COPY_PLACE_0.as_mut_ptr().write(U512::zero());
        LOW_WORD_SCRATCH.as_mut_ptr().write(U256::zero());
        MUL_COPY_PLACE_0.as_mut_ptr().write(U256::zero());
        MUL_COPY_PLACE_1.as_mut_ptr().write(U256::zero());
        MUL_COPY_PLACE_2.as_mut_ptr().write(U256::zero());
        MUL_COPY_PLACE_3.as_mut_ptr().write(U256::zero());
        MUL_COPY_PLACE_4.as_mut_ptr().write(U256::zero());
        MUL_COPY_PLACE_5.as_mut_ptr().write(U256::zero());
    }
}

static mut ZERO: MaybeUninit<U256> = MaybeUninit::uninit();
static mut ONE: MaybeUninit<U256> = MaybeUninit::uninit();
static mut COPY_PLACE_0: MaybeUninit<U512> = MaybeUninit::uninit();
static mut LOW_WORD_SCRATCH: MaybeUninit<U256> = MaybeUninit::uninit();
static mut MUL_COPY_PLACE_0: MaybeUninit<U256> = MaybeUninit::uninit();
static mut MUL_COPY_PLACE_1: MaybeUninit<U256> = MaybeUninit::uninit();
static mut MUL_COPY_PLACE_2: MaybeUninit<U256> = MaybeUninit::uninit();
static mut MUL_COPY_PLACE_3: MaybeUninit<U256> = MaybeUninit::uninit();
static mut MUL_COPY_PLACE_4: MaybeUninit<U256> = MaybeUninit::uninit();
static mut MUL_COPY_PLACE_5: MaybeUninit<U256> = MaybeUninit::uninit();

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

    u256::copy(low_dst, as_low(src));
    u256::copy(high_dst, as_high(src));
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
    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    copy(temp, a);
    add_mod_assign::<T>(a, temp);
}

/// Computes `self = -self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn neg_mod_assign<T: DelegatedModParams<8>>(a: &mut U512) {
    let (low, high) = as_low_high_mut(a);
    let zero = unsafe { ZERO.assume_init_ref() };

    let is_low_zero = delegation::eq(low, zero) != 0;
    let is_high_zero = delegation::eq(high, zero) != 0;

    if !is_low_zero && !is_high_zero {
        let borrow = u256::sub_and_negate_assign(low, as_low(T::modulus()));
        u256::sub_and_negate_with_carry(high, as_high(T::modulus()), borrow);
    }
}

/// Compute `self = self * rhs mod modulus` using montgomerry reduction.
/// Both `self` and `rhs` are assumed to be in montgomerry form.
/// The reduction constant is expected to be `-1/modulus mod 2^256`
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn mul_assign_montgomery<T: DelegatedMontParams<8>>(a: &mut U512, b: &U512) {
    let one = unsafe { ONE.assume_init_ref() };

    let (r0, r1) = {
        let b0 = delegation::copy_if_needed(as_low(b));
        let r0 = unsafe { MUL_COPY_PLACE_0.assume_init_mut() };
        delegation::memcpy(r0, as_low(a));

        let carry_1 = unsafe { MUL_COPY_PLACE_1.assume_init_mut() };
        delegation::memcpy(carry_1, r0);

        u256::mul_low_assign(r0, b0);
        u256::mul_high_assign(carry_1, b0);

        let reduction_k = unsafe { MUL_COPY_PLACE_2.assume_init_mut() };
        delegation::memcpy(reduction_k, r0);
        u256::mul_low_assign(reduction_k, T::reduction_const());

        let carry_2_low = unsafe { MUL_COPY_PLACE_3.assume_init_mut() };
        delegation::memcpy(carry_2_low, as_low(T::modulus()));

        u256::mul_low_assign(carry_2_low, reduction_k);
        let of = u256::add_assign(carry_2_low, r0);

        let carry_2 = unsafe { MUL_COPY_PLACE_4.assume_init_mut() };
        delegation::memcpy(carry_2, as_low(T::modulus()));

        u256::mul_high_assign(carry_2, reduction_k);

        if of {
            u256::add_assign(carry_2, one);
        }

        // We can reuse MUL_COPY_PLACE_3
        debug_assert!(carry_2_low.is_zero());

        let r1 = unsafe { MUL_COPY_PLACE_3.assume_init_mut() };
        delegation::memcpy(r1, as_high(a));

        let new_carry_1 = unsafe { MUL_COPY_PLACE_5.assume_init_mut() };
        delegation::memcpy(new_carry_1, r1);

        u256::mul_low_assign(r1, b0);
        let of = u256::add_assign(r1, carry_1);

        u256::mul_high_assign(new_carry_1, b0);

        if of {
            u256::add_assign(new_carry_1, one);
        }

        // now MUL_COPY_PLACE_1 is available
        let carry_1 = new_carry_1;

        let new_carry_2_low = unsafe { MUL_COPY_PLACE_1.assume_init_mut() };
        delegation::memcpy(new_carry_2_low, as_high(T::modulus()));

        u256::mul_low_assign(new_carry_2_low, reduction_k);
        let of0 = u256::add_assign(new_carry_2_low, r1);
        let of1 = u256::add_assign(new_carry_2_low, carry_2);

        // we can reuse MUL_COPY_PLACE_4 now
        let new_carry_2 = unsafe { MUL_COPY_PLACE_4.assume_init_mut() };
        delegation::memcpy(new_carry_2, as_high(T::modulus()));

        u256::mul_high_assign(new_carry_2, reduction_k);

        if of0 || of1 {
            let temp = unsafe { LOW_WORD_SCRATCH.assume_init_mut() };
            temp.0[0] = of0 as u64 + of1 as u64;
            u256::add_assign(new_carry_2, temp);
        }

        let r0 = new_carry_2_low;
        let carry_2 = new_carry_2;

        let r1 = carry_1;
        u256::add_assign(r1, carry_2);

        debug_assert!(r1.0[2..4].iter().all(|&x| x == 0));

        // we use MUL_COPY_PLACE_1 and MUL_COPY_PLACE_5
        (r0, r1)
    };

    let b1 = delegation::copy_if_needed(as_high(b));

    let new_r0 = unsafe { MUL_COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(new_r0, as_low(a));

    let carry_1 = unsafe { MUL_COPY_PLACE_2.assume_init_mut() };
    delegation::memcpy(carry_1, new_r0);

    u256::mul_low_assign(new_r0, b1);
    let of = u256::add_assign(new_r0, r0);
    u256::mul_high_assign(carry_1, b1);
    if of {
        u256::add_assign(carry_1, one);
    }
    // MUL_COPY_PLACE_1 is free
    let r0 = new_r0;

    let reduction_k = unsafe { MUL_COPY_PLACE_1.assume_init_mut() };
    delegation::memcpy(reduction_k, r0);

    u256::mul_low_assign(reduction_k, T::reduction_const());

    let carry_2_low = unsafe { MUL_COPY_PLACE_3.assume_init_mut() };
    delegation::memcpy(carry_2_low, as_low(T::modulus()));

    u256::mul_low_assign(carry_2_low, reduction_k);
    let of = u256::add_assign(carry_2_low, r0);

    let carry_2 = unsafe { MUL_COPY_PLACE_4.assume_init_mut() };
    delegation::memcpy(carry_2, as_low(T::modulus()));

    u256::mul_high_assign(carry_2, reduction_k);

    if of {
        u256::add_assign(carry_2, one);
    }

    // MUL_COPY_PLACE_3 is free
    debug_assert!(carry_2_low.is_zero());

    let new_r1 = unsafe { MUL_COPY_PLACE_3.assume_init_mut() };
    delegation::memcpy(new_r1, as_high(a));

    u256::mul_low_assign(new_r1, b1);
    let of0 = u256::add_assign(new_r1, carry_1);
    let of1 = u256::add_assign(new_r1, r1);

    let (a0, a1) = as_low_high_mut(a);
    u256::mul_high_assign(a1, b1);

    if of0 || of1 {
        let temp = unsafe { LOW_WORD_SCRATCH.assume_init_mut() };
        temp.0[0] = of0 as u64 + of1 as u64;
        u256::add_assign(a1, temp);
    }

    // MUL_COPY_PLACE_5 is free
    let r1 = new_r1;

    delegation::memcpy(a0, as_high(T::modulus()));
    u256::mul_low_assign(a0, reduction_k);

    let of0 = u256::add_assign(a0, r1);
    let of1 = u256::add_assign(a0, carry_2);

    let new_carry_2 = unsafe { MUL_COPY_PLACE_4.assume_init_mut() };
    delegation::memcpy(new_carry_2, as_high(T::modulus()));

    u256::mul_high_assign(new_carry_2, reduction_k);

    if of0 || of1 {
        let temp = unsafe { LOW_WORD_SCRATCH.assume_init_mut() };
        temp.0[0] = of0 as u64 + of1 as u64;
        u256::add_assign(new_carry_2, temp);
    }

    let carry2 = new_carry_2;

    u256::add_assign(a1, carry2);

    debug_assert!(a.0[6..8].iter().all(|&x| x == 0));
}

/// Compute `self = self^2 mod modulus` using montgomerry reduction.
/// `self` should be in montgomerry form.
/// The reduction constant is expected to be `-1/modulus mod 2^256`
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_montgomery<T: DelegatedMontParams<8>>(a: &mut U512) {
    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    copy(temp, a);
    mul_assign_montgomery::<T>(a, temp);
}
