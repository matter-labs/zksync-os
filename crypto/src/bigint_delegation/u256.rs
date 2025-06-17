use super::{delegation, DelegatedBarretParams, DelegatedModParams, DelegatedMontParams};
use crate::ark_ff_delegation::{BigInt, BigInteger};
use core::{fmt::Debug, marker::PhantomData, mem::MaybeUninit};

pub(super) type U256 = BigInt<4>;

static mut COPY_PLACE_0: MaybeUninit<U256> = MaybeUninit::uninit();
static mut COPY_PLACE_1: MaybeUninit<U256> = MaybeUninit::uninit();
static mut COPY_PLACE_2: MaybeUninit<U256> = MaybeUninit::uninit();
static mut COPY_PLACE_3: MaybeUninit<U256> = MaybeUninit::uninit();
static mut ONE: MaybeUninit<U256> = MaybeUninit::uninit();
static mut ZERO: MaybeUninit<U256> = MaybeUninit::uninit();
static mut SCRATCH: MaybeUninit<U256> = MaybeUninit::uninit();

pub fn init() {
    unsafe {
        COPY_PLACE_0.as_mut_ptr().write(U256::zero());
        COPY_PLACE_1.as_mut_ptr().write(U256::zero());
        COPY_PLACE_2.as_mut_ptr().write(U256::zero());
        COPY_PLACE_3.as_mut_ptr().write(U256::zero());
        ONE.as_mut_ptr().write(U256::one());
        ZERO.as_mut_ptr().write(U256::zero());
        SCRATCH.as_mut_ptr().write(U256::zero());
    }
}

pub const fn from_bytes_unchecked(bytes: &[u8; 32]) -> U256 {
    BigInt::<4>([
        u64::from_le_bytes([
            bytes[31], bytes[30], bytes[29], bytes[28], bytes[27], bytes[26], bytes[25], bytes[24],
        ]),
        u64::from_le_bytes([
            bytes[23], bytes[22], bytes[21], bytes[20], bytes[19], bytes[18], bytes[17], bytes[16],
        ]),
        u64::from_le_bytes([
            bytes[15], bytes[14], bytes[13], bytes[12], bytes[11], bytes[10], bytes[9], bytes[8],
        ]),
        u64::from_le_bytes([
            bytes[7], bytes[6], bytes[5], bytes[4], bytes[3], bytes[2], bytes[1], bytes[0],
        ]),
    ])
}

pub fn to_be_bytes(a: U256) -> [u8; 32] {
    let mut r = [0u8; 32];
    r[0..8].copy_from_slice(&a.0[3].to_be_bytes());
    r[8..16].copy_from_slice(&a.0[2].to_be_bytes());
    r[16..24].copy_from_slice(&a.0[1].to_be_bytes());
    r[24..32].copy_from_slice(&a.0[0].to_be_bytes());

    r
}

#[inline(always)]
pub fn copy(dst: &mut U256, src: &U256) {
    #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
    if (src as *const U256).addr() < delegation::ROM_BOUND {
        *dst = *src;
    } else {
        delegation::memcpy(dst, src);
    };

    #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
    {
        *dst = *src;
    }
}

#[inline(always)]
/// adds `rhs` to `self` and returns the carry
pub fn add_assign(a: &mut U256, b: &U256) -> bool {
    let b = delegation::copy_if_needed(b);
    delegation::add(a, b) != 0
}

#[inline(always)]
/// subtracts `rhs` from `self` and returns the borrow
pub fn sub_assign(a: &mut U256, b: &U256) -> bool {
    let b = delegation::copy_if_needed(b);
    delegation::sub(a, b) != 0
}

#[inline(always)]
/// subtracts `self` from `rhs` and reutrns the borrow
pub fn sub_and_negate_assign(a: &mut U256, b: &U256) -> bool {
    let b = delegation::copy_if_needed(b);
    delegation::sub_and_negate(a, b) != 0
}

#[inline(always)]
/// multiplies `self` with `rhs` and storest the lowest 256 bits in self
pub fn mul_low_assign(a: &mut U256, b: &U256) {
    let b = delegation::copy_if_needed(b);
    delegation::mul_low(a, b);
}

#[inline(always)]
/// multiplies `self` with `rhs` and storest the highest 256 bits in self
pub fn mul_high_assign(a: &mut U256, b: &U256) {
    let b = delegation::copy_if_needed(b);
    delegation::mul_high(a, b);
}

#[inline(always)]
pub fn mul_wide(a: &U256, b: &U256) -> (U256, U256) {
    let b = delegation::copy_if_needed(b);

    let mut low = U256::zero();
    let mut high = U256::zero();

    delegation::memcpy(&mut low, a);
    delegation::memcpy(&mut high, a);

    delegation::mul_low(&mut low, b);
    delegation::mul_high(&mut high, b);

    (low, high)
}

#[inline(always)]
/// computes `self = self - rhs - carry` and returns the borrow
pub fn sub_with_carry_bit(a: &mut U256, b: &U256, carry: bool) -> bool {
    let b = delegation::copy_if_needed(b);
    delegation::sub_with_carry_bit(a, b, carry) != 0
}

#[inline(always)]
/// computes `self = self + rhs + carry` and returns the carry
pub fn add_with_carry_bit(a: &mut U256, b: &U256, carry: bool) -> bool {
    let b = delegation::copy_if_needed(b);
    delegation::add_with_carry_bit(a, b, carry) != 0
}

#[inline(always)]
/// computes `self = rhs - self - carry` and returns the borrow
pub fn sub_and_negate_with_carry(a: &mut U256, b: &U256, carry: bool) -> bool {
    let b = delegation::copy_if_needed(b);
    delegation::sub_and_negate_with_carry_bit(a, b, carry) != 0
}

#[inline(always)]
/// Tries to get `self` in the range `[0..modulus)`.
/// Note: we assume `self < 2*modulus`, otherwise the result might not be in the range
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn sub_mod_with_carry<T: DelegatedModParams<4>>(a: &mut U256, carry: bool) {
    let borrow = delegation::sub(a, T::modulus()) != 0;

    if borrow && !carry {
        delegation::add(a, T::modulus());
    }
}

#[inline(always)]
/// computes `self = self + rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn add_mod_assign<T: DelegatedModParams<4>>(a: &mut U256, b: &U256) {
    let b = delegation::copy_if_needed(b);
    let carry = delegation::add(a, b) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// computes `self = self - rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn sub_mod_assign<T: DelegatedModParams<4>>(a: &mut U256, b: &U256) {
    let b = delegation::copy_if_needed(b);
    let borrow = delegation::sub(a, b);
    if borrow != 0 {
        delegation::add(a, T::modulus());
    }
}

#[inline(always)]
/// Computes `self = self + self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn double_mod_assign<T: DelegatedModParams<4>>(a: &mut U256) {
    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(temp, a);
    let carry = delegation::add(a, temp) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// Computes `self = -self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn neg_mod_assign<T: DelegatedModParams<4>>(a: &mut U256) {
    let zero = unsafe { ZERO.assume_init_ref() };
    // delegation::eq returns 1 if they are equal and zero if not
    if delegation::eq(a, zero) == 0 {
        delegation::sub_and_negate(a, T::modulus());
    }
}

#[inline(always)]
pub fn eq(a: &U256, b: &U256) -> bool {
    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(temp, a);
    let b = delegation::copy_if_needed(b);

    delegation::eq(temp, b) != 0
}

#[inline(always)]
pub fn is_zero(a: &U256) -> bool {
    let zero = unsafe { ZERO.assume_init_ref() };
    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(temp, a);
    delegation::eq(temp, zero) != 0
}

#[inline(always)]
/// it takes `a` as mutable for the purposes of delegation calls, but doesn't mutate it
pub fn is_one(a: &mut U256) -> bool {
    let one = unsafe { ONE.assume_init_ref() };

    delegation::eq(a, one) != 0
}

#[inline(always)]
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn is_zero_mod<T: DelegatedModParams<4>>(a: &U256) -> bool {
    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    let zero = unsafe { ZERO.assume_init_ref() };

    delegation::memcpy(temp, a);

    (delegation::eq(temp, zero) != 0) || (delegation::eq(temp, T::modulus()) != 0)
}

#[inline(always)]
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn eq_mod<T: DelegatedModParams<4>>(a: &U256, b: &U256) -> bool {
    let temp0 = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(temp0, a);

    let b = delegation::copy_if_needed(b);

    if delegation::eq(temp0, b) == 0 {
        let temp1 = unsafe { COPY_PLACE_1.assume_init_mut() };
        delegation::memcpy(temp1, b);

        sub_mod_with_carry::<T>(temp0, false);
        sub_mod_with_carry::<T>(temp1, false);

        delegation::eq(temp0, temp1) != 0
    } else {
        true
    }
}

pub fn lt(a: &U256, b: &U256) -> bool {
    let b = delegation::copy_if_needed(b);

    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(temp, a);

    // if we get a borrow, then self < other
    delegation::sub(temp, b) != 0
}

pub fn leq(a: &U256, b: &U256) -> bool {
    let b = delegation::copy_if_needed(b);

    let temp = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(temp, a);

    // if we get a borrow, then self < other
    delegation::eq(temp, b) != 0 || delegation::sub(temp, b) != 0
}

#[inline(always)]
/// modular multiplication with barret reduction
/// # Safety
/// `DelegationBarretParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn mul_assign_barret<T: DelegatedBarretParams<4>>(a: &mut U256, b: &U256) {
    let b = delegation::copy_if_needed(b);

    let temp0 = unsafe { COPY_PLACE_1.assume_init_mut() };
    let temp1 = unsafe { COPY_PLACE_2.assume_init_mut() };

    // we will keep high part of product in temp0 until the very end
    delegation::memcpy(temp0, a);

    delegation::mul_low(a, b);
    delegation::mul_high(temp0, b);

    delegation::memcpy(temp1, temp0);

    // multiply copy_place0 by 2^256 - modulus
    delegation::mul_low(temp1, T::neg_modulus());
    delegation::mul_high(temp0, T::neg_modulus());

    // add and propagate the carry
    let carry = delegation::add(a, temp1) != 0;
    if carry {
        let one = unsafe { ONE.assume_init_ref() };
        delegation::add(temp0, one);
    }

    delegation::mul_low(temp0, T::neg_modulus());

    let carry = delegation::add(a, temp0) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// # Safety
/// `DelegationBarretParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_barret<T: DelegatedBarretParams<4>>(a: &mut U256) {
    let b = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(b, a);

    mul_assign_barret::<T>(a, b);
}

#[inline(always)]
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_montgomery<T: DelegatedMontParams<4>>(a: &mut U256) {
    let b = unsafe { COPY_PLACE_0.assume_init_mut() };
    delegation::memcpy(b, a);

    mul_assign_montgomery::<T>(a, b);
}

#[inline(always)]
/// Modular multiplication with montgomerry reduction.
/// It's the responsibility of the caller to make sure the parameters are in montgomerry form.
/// # Safety
///
pub unsafe fn mul_assign_montgomery<T: DelegatedMontParams<4>>(a: &mut U256, b: &U256) {
    let b = delegation::copy_if_needed(b);

    let temp0 = unsafe { COPY_PLACE_1.assume_init_mut() };
    let temp1 = unsafe { COPY_PLACE_2.assume_init_mut() };
    let temp2 = unsafe { COPY_PLACE_3.assume_init_mut() };

    delegation::memcpy(temp0, a);

    delegation::mul_low(temp0, b);
    delegation::mul_high(a, b);

    delegation::memcpy(temp1, temp0);

    delegation::mul_low(temp1, T::reduction_const());

    delegation::memcpy(temp2, temp1);

    delegation::mul_low(temp2, T::modulus());
    delegation::mul_high(temp1, T::modulus());

    let carry = delegation::add(temp2, temp0) != 0;

    debug_assert!(temp2.is_zero());

    if carry {
        let one = unsafe { ONE.assume_init_ref() };
        delegation::add(temp1, one);
    }

    let carry = delegation::add(a, temp1) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[cfg(test)]
#[derive(Debug)]
pub struct U256Wrapper<T: DelegatedModParams<4>>(pub U256, PhantomData<T>);

#[cfg(test)]
impl<T: DelegatedModParams<4> + Debug> proptest::arbitrary::Arbitrary for U256Wrapper<T> {
    type Parameters = T;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Just, Strategy};

        any::<[u64; 4]>().prop_map(|words| {
            let mut res = BigInt::<4>(words);
            unsafe {
                sub_mod_with_carry::<Self::Parameters>(&mut res, false);
                sub_mod_with_carry::<Self::Parameters>(&mut res, false);
            }
            Self(res, PhantomData::default())
        })
    }

    type Strategy = proptest::arbitrary::Mapped<[u64; 4], Self>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ark_ff_delegation::BigInt;

    use ark_ff::{BigInt as BigIntRef, BigInteger};
    use proptest::{prop_assert_eq, proptest};

    #[derive(Default, Debug)]
    struct ZeroMod;

    impl DelegatedModParams<4> for ZeroMod {
        unsafe fn modulus() -> &'static BigInt<4> {
            unsafe { ZERO.assume_init_ref() }
        }
    }

    #[ignore = "requires a single threaded runner"]
    #[test]
    fn test_mul_wide() {
        proptest!(|(x: U256Wrapper<ZeroMod>, y: U256Wrapper<ZeroMod>)| {
            let (x, y) = (x.0, y.0);
            let x_ref = BigIntRef::new(x.0);
            let y_ref = BigIntRef::new(y.0);

            let (ref_low, ref_high) = x_ref.mul(&y_ref);
            let (low, high) = mul_wide(&x, &y);

            prop_assert_eq!(low.0, ref_low.0);
            prop_assert_eq!(high.0, ref_high.0);

        })
    }
}
