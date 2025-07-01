use crate::ark_ff_delegation::BigInt;
use core::{borrow, fmt::Debug, marker::PhantomData, mem::MaybeUninit};

use bigint_riscv::*;

#[inline(always)]
pub fn from_ark_ref(a: &BigInt<4>) -> &DelegatedU256 {
    debug_assert_eq!(
        core::mem::align_of_val(a),
        core::mem::align_of::<DelegatedU256>()
    );
    debug_assert_eq!(
        core::mem::size_of_val(a),
        core::mem::size_of::<DelegatedU256>()
    );

    unsafe { core::mem::transmute(a) }
}

#[inline(always)]

pub fn from_ark_mut(a: &mut BigInt<4>) -> &mut DelegatedU256 {
    debug_assert_eq!(
        core::mem::align_of_val(a),
        core::mem::align_of::<DelegatedU256>()
    );
    debug_assert_eq!(
        core::mem::size_of_val(a),
        core::mem::size_of::<DelegatedU256>()
    );

    unsafe { core::mem::transmute(a) }
}

pub trait DelegatedModParams: Default {
    /// Provides a reference to the modululs for delegation purposes
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn modulus() -> &'static DelegatedU256;
}

pub trait DelegatedMontParams: DelegatedModParams {
    /// Provides a reference to the reduction const (`-1/Self::modulus mod 2^256`) for Montgomerry reduction
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn reduction_const() -> &'static DelegatedU256;
}

pub trait DelegatedBarretParams: DelegatedModParams {
    /// Provides a reference to `-Self::modulus mod 2^256` for Barret reduction
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn neg_modulus() -> &'static DelegatedU256;
}

#[inline(always)]
/// Tries to get `self` in the range `[0..modulus)`.
/// Note: we assume `self < 2*modulus`, otherwise the result might not be in the range
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
unsafe fn sub_mod_with_carry<T: DelegatedModParams>(a: &mut DelegatedU256, carry: bool) {
    let borrow = a.overflowing_sub_assign(T::modulus());

    if borrow && !carry {
        a.overflowing_add_assign(T::modulus());
    }
}

#[inline(always)]
/// computes `self = self + rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn add_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU256, b: &DelegatedU256) {
    let carry = a.overflowing_add_assign(b);
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// computes `self = self - rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn sub_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU256, b: &DelegatedU256) {
    let borrow = a.overflowing_sub_assign(b);
    if borrow {
        a.overflowing_add_assign(T::modulus());
    }
}

#[inline(always)]
/// Computes `self = self + self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn double_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU256) {
    let temp = a.clone();
    let carry = a.overflowing_add_assign(&temp);
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// Computes `self = -self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn neg_mod_assign<T: DelegatedModParams>(a: &mut DelegatedU256) {
    if !a.is_zero() {
        a.overflowing_sub_and_negate_assign(T::modulus());
    }
}

#[inline(always)]
/// modular multiplication with barret reduction
/// # Safety
/// `DelegationBarretParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn mul_assign_barret<T: DelegatedBarretParams>(
    a: &mut DelegatedU256,
    b: &DelegatedU256,
) {
    let b = copy_if_needed(b);

    let mut temp0 = a.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(a, b);
    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut temp0, b);

    let mut temp1 = temp0.clone();

    // multiply copy_place0 by 2^256 - modulus
    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut temp1, T::neg_modulus());
    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut temp0, T::neg_modulus());

    // add and propagate the carry
    let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(a, &temp1) != 0;
    if carry {
        bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut temp0, ONE.as_ptr());
    }

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut temp0, T::neg_modulus());

    let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(a, &temp0) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// # Safety
/// `DelegationBarretParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_barret<T: DelegatedBarretParams>(a: &mut DelegatedU256) {
    let b = a.clone();
    mul_assign_barret::<T>(a, &b);
}

#[inline(always)]
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_montgomery<T: DelegatedMontParams>(a: &mut DelegatedU256) {
    let b = a.clone();
    mul_assign_montgomery::<T>(a, &b);
}

#[inline(always)]
/// Modular multiplication with montgomerry reduction.
/// It's the responsibility of the caller to make sure the parameters are in montgomerry form.
/// # Safety
///
pub unsafe fn mul_assign_montgomery<T: DelegatedMontParams>(
    a: &mut DelegatedU256,
    b: &DelegatedU256,
) {
    let b = copy_if_needed(b);

    let mut temp0 = a.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut temp0, b);
    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(a, b);

    let mut temp1 = temp0.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut temp1, T::reduction_const());

    let mut temp2 = temp1.clone();

    bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(&mut temp2, T::modulus());
    bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut temp1, T::modulus());

    let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut temp2, &temp0) != 0;

    debug_assert!(temp2.is_zero());

    if carry {
        bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut temp1, ONE.as_ptr());
    }

    let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(a, &temp1) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[cfg(test)]
#[derive(Debug)]
pub struct U256Wrapper<T: DelegatedModParams>(pub DelegatedU256, PhantomData<T>);

#[cfg(test)]
impl<T: DelegatedModParams + Debug> proptest::arbitrary::Arbitrary for U256Wrapper<T> {
    type Parameters = T;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Just, Strategy};

        any::<[u64; 4]>().prop_map(|limbs| {
            let mut res = DelegatedU256::from_limbs(limbs);
            unsafe {
                sub_mod_with_carry::<Self::Parameters>(&mut res, false);
                sub_mod_with_carry::<Self::Parameters>(&mut res, false);
            }
            Self(res, PhantomData::default())
        })
    }

    type Strategy = proptest::arbitrary::Mapped<[u64; 4], Self>;
}
