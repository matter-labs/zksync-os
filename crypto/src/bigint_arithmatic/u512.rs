use crate::ark_ff_delegation::BigInt;
use core::{borrow, mem::MaybeUninit};

use super::u256;
use bigint_riscv::*;

#[inline(always)]
pub fn from_ark_ref(a: &BigInt<8>) -> &DelegatedU512 {
    debug_assert_eq!(
        core::mem::align_of_val(a),
        core::mem::align_of::<DelegatedU512>()
    );
    debug_assert_eq!(
        core::mem::size_of_val(a),
        core::mem::size_of::<DelegatedU512>()
    );

    unsafe { core::mem::transmute(a) }
}

#[inline(always)]
pub fn from_ark_mut(a: &mut BigInt<8>) -> &mut DelegatedU512 {
    debug_assert_eq!(
        core::mem::align_of_val(a),
        core::mem::align_of::<DelegatedU512>()
    );
    debug_assert_eq!(
        core::mem::size_of_val(a),
        core::mem::size_of::<DelegatedU512>()
    );

    unsafe { core::mem::transmute(a) }
}

pub trait DelegatedModParams: Default {
    /// Provides a reference to the modululs for delegation purposes
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn modulus() -> &'static DelegatedU512;
}

pub trait DelegatedMontParams: DelegatedModParams {
    /// Provides a reference to the reduction const (`-1/Self::modulus mod 2^256`) for Montgomerry reduction
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn reduction_const() -> &'static DelegatedU256;
}

#[repr(C)]
pub struct DelegatedU512(DelegatedU256, DelegatedU256);

impl DelegatedU512 {
    pub fn one() -> Self {
        Self(DelegatedU256::one(), DelegatedU256::zero())
    }

    pub const fn from_limbs(limbs: [u64; 8]) -> Self {
        let (low_ref, high_ref) = limbs.split_at(4);
        let mut low = [0; 4];
        let mut high = [0; 4];

        low.copy_from_slice(low_ref);
        high.copy_from_slice(high_ref);

        Self(
            DelegatedU256::from_limbs(low),
            DelegatedU256::from_limbs(high),
        )
    }
}

/// Tries to get `self` in the range `[0..modulus)`.
/// Note: we assume `self < 2*modulus`, otherwise the result might not be in the range
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
unsafe fn sub_mod_with_carry<T: DelegatedModParams>(a: &mut DelegatedU512, carry: bool) {
    let borrow = bigint_op_delegation::<SUB_OP_BIT_IDX>(&mut a.0, &T::modulus().0) != 0;
    let borrow =
        bigint_op_delegation_with_carry_bit::<SUB_OP_BIT_IDX>(&mut a.1, &T::modulus().1, borrow)
            != 0;

    if borrow & !carry {
        let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.0, &T::modulus().0) != 0;
        bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(&mut a.1, &T::modulus().1, carry);
    }
}

/// Computes `self = self + rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn add_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU512, b: &DelegatedU512) {
    let carry = a.0.overflowing_add_assign(&b.0);
    let carry = a.1.overflowing_add_assign_with_carry(&b.1, carry);

    sub_mod_with_carry::<T>(a, carry);
}

/// Computes `self = self - rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn sub_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU512, b: &DelegatedU512) {
    let borrow = a.0.overflowing_sub_assign(&b.0);
    let borrow = a.1.overflowing_sub_assign_with_borrow(&b.1, borrow);

    if borrow {
        let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.0, &T::modulus().0) != 0;
        bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(&mut a.1, &T::modulus().1, carry);
    }
}

/// Computes `self = self + self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn double_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU512) {
    let low = a.0.clone();
    let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.0, &low) != 0;

    let high = a.1.clone();
    let carry = bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(&mut a.1, &high, carry) != 0;

    sub_mod_with_carry::<T>(a, carry);
}

/// Computes `self = -self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn neg_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU512) {
    if !a.0.is_zero_mut() && !a.1.is_zero_mut() {
        let borrow =
            bigint_op_delegation::<SUB_AND_NEGATE_OP_BIT_IDX>(&mut a.0, &T::modulus().0) != 0;
        bigint_op_delegation_with_carry_bit::<SUB_AND_NEGATE_OP_BIT_IDX>(
            &mut a.1,
            &T::modulus().1,
            borrow,
        );
    }
}

/// Compute `self = self * rhs mod modulus` using montgomerry reduction.
/// Both `self` and `rhs` are assumed to be in montgomerry form.
/// The reduction constant is expected to be `-1/modulus mod 2^256`
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn mul_assign_montgomery<T: DelegatedMontParams>(
    a: &mut DelegatedU512,
    b: &DelegatedU512,
) {
    let (r0, r1) = {
        let b0 = copy_if_needed(&b.0);
        let mut r0 = a.0.clone();

        let mut carry_1 = r0.clone();

        bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut r0, b0);
        bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut carry_1, b0);

        let mut reduction_k = r0.clone();
        bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut reduction_k, T::reduction_const());

        let mut carry_2_low = T::modulus().0.clone();
        bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut carry_2_low, &reduction_k);

        let of = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut carry_2_low, &r0) != 0;

        let mut carry_2 = T::modulus().0.clone();
        bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut carry_2, &reduction_k);

        if of {
            bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut carry_2, ONE.as_ptr());
        }

        debug_assert!(carry_2_low.is_zero());

        let mut r1 = a.1.clone();

        let mut new_carry_1 = r1.clone();

        bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut r1, b0);
        let of = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut r1, &carry_1) != 0;
        bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut new_carry_1, b0);

        if of {
            bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_carry_1, ONE.as_ptr());
        }

        let mut new_carry_2_low = T::modulus().1.clone();

        bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut new_carry_2_low, &reduction_k);
        let of0 = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_carry_2_low, &r1) != 0;
        let of1 = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_carry_2_low, &carry_2) != 0;

        let mut new_carry_2 = T::modulus().1.clone();
        bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut new_carry_2, &reduction_k);

        if of0 || of1 {
            let temp = DelegatedU256::from(of0 as u64 + of1 as u64);
            bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_carry_2, &temp);
        }

        let r0 = new_carry_2_low;

        let mut r1 = new_carry_1;
        bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut r1, &new_carry_2);

        debug_assert!(r1.as_limbs()[2..4].iter().all(|&x| x == 0));

        (r0, r1)
    };

    let b1 = copy_if_needed(&b.1);

    let mut new_r0 = a.0.clone();

    let mut carry_1 = new_r0.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut new_r0, b1);
    let of = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_r0, &r0) != 0;
    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut carry_1, b1);
    if of {
        bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut carry_1, ONE.as_ptr());
    }

    let r0 = new_r0;

    let mut reduction_k = r0.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut reduction_k, T::reduction_const());

    let mut carry_2_low = T::modulus().0.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut carry_2_low, &reduction_k);
    let of = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut carry_2_low, &r0) != 0;

    let mut carry_2 = T::modulus().0.clone();

    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut carry_2, &reduction_k);

    if of {
        bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut carry_2, ONE.as_ptr());
    }

    debug_assert!(carry_2_low.is_zero());

    let mut new_r1 = a.1.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut new_r1, b1);
    let of0 = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_r1, &carry_1) != 0;
    let of1 = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_r1, &r1) != 0;

    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut a.1, b1);

    if of0 || of1 {
        let temp = DelegatedU256::from(of0 as u64 + of1 as u64);
        bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.1, &temp);
    }

    let r1 = new_r1;

    a.0 = T::modulus().1.clone();
    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut a.0, &reduction_k);

    let of0 = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.0, &r1) != 0;
    let of1 = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.0, &carry_2) != 0;

    let mut new_carry_2 = T::modulus().1.clone();
    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut new_carry_2, &reduction_k);

    if of0 || of1 {
        let temp = DelegatedU256::from(of0 as u64 + of1 as u64);
        bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut new_carry_2, &temp);
    }

    let carry2 = new_carry_2;

    bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.1, &carry2);

    debug_assert!(a.0.as_limbs()[6..8].iter().all(|&x| x == 0));
}

/// Compute `self = self^2 mod modulus` using montgomerry reduction.
/// `self` should be in montgomerry form.
/// The reduction constant is expected to be `-1/modulus mod 2^256`
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_montgomery<T: DelegatedMontParams>(a: &mut DelegatedU512) {
    let temp = DelegatedU512(a.0.clone(), a.1.clone());
    mul_assign_montgomery::<T>(a, &temp);
}
