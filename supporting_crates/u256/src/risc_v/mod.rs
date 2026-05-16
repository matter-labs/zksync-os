use core::ops::{
    AddAssign, BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign, SubAssign,
};
use delegated_u256::*;

// Even though we derive, internally we use delegation circuit for equality, ordering and cloning
// See DelegatedU256 implementations for details
#[derive(
    Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Debug, serde::Serialize, serde::Deserialize,
)]
pub struct U256(DelegatedU256);

impl core::fmt::Display for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <DelegatedU256 as core::fmt::LowerHex>::fmt(&self.0, f)
    }
}

impl core::default::Default for U256 {
    #[inline(always)]
    fn default() -> Self {
        Self::zero()
    }
}

impl U256 {
    pub const ZERO: Self = Self(DelegatedU256::ZERO);
    pub const ONE: Self = Self(DelegatedU256::ONE);

    pub const BYTES: usize = 32;

    pub const fn from_limbs(limbs: [u64; 4]) -> Self {
        Self(DelegatedU256::from_limbs(limbs))
    }

    /// # Safety
    /// `dst` must be 32 byte aligned and point to 32 bytes of accessible memory.
    pub unsafe fn write_into_ptr(dst: *mut Self, source: &Self) {
        delegated_u256::write_into_ptr(dst.cast(), &source.0);
    }

    /// # Safety
    /// `dst` must be 32 byte aligned and point to 32 bytes of accessible memory.
    pub unsafe fn write_into_ptr_unchecked(dst: *mut Self, source: &Self) {
        delegated_u256::write_into_ptr_unchecked(dst.cast(), &source.0);
    }

    /// # Safety
    /// `a` and `b` must be valid, properly aligned pointers to initialized `Self` values.
    ///
    /// On the delegated backend this is cheaper than a generic `mem::swap`, because it stays on
    /// the bigint memcopy path instead of forcing a raw 32-byte move sequence in RISC-V code.
    pub unsafe fn swap_in_place(a: *mut Self, b: *mut Self) {
        if core::ptr::eq(a, b) {
            return;
        }

        let mut tmp = core::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            Self::write_into_ptr_unchecked(tmp.as_mut_ptr(), &*a);
            Self::write_into_ptr_unchecked(a, &*b);
            Self::write_into_ptr_unchecked(b, tmp.assume_init_ref());
        }
    }

    pub fn clone_into(&self, dst: &mut Self) {
        unsafe { Self::write_into_ptr(dst as *mut _, self) };
    }

    pub unsafe fn clone_into_unchecked(&self, dst: &mut Self) {
        Self::write_into_ptr_unchecked(dst as *mut _, self);
    }

    #[inline(always)]
    pub fn zero() -> Self {
        Self::from_limbs([0, 0, 0, 0])
    }

    #[inline(always)]
    pub fn one() -> Self {
        Self::from_limbs([1, 0, 0, 0])
    }

    pub fn bytereverse(&mut self) {
        self.0.bytereverse();
    }

    #[inline(always)]
    pub fn write_zero(into: &mut Self) {
        into.0.write_zero();
    }

    #[inline(always)]
    pub fn write_one(into: &mut Self) {
        into.0.write_one();
    }

    #[inline(always)]
    pub unsafe fn write_zero_into_ptr(into: *mut Self) {
        delegated_u256::write_zero_into_ptr(into.cast());
    }

    #[inline(always)]
    pub unsafe fn write_one_into_ptr(into: *mut Self) {
        delegated_u256::write_one_into_ptr(into.cast());
    }

    #[inline(always)]
    pub unsafe fn write_u64_into_ptr(into: *mut Self, value: u64) {
        delegated_u256::write_u64_into_ptr(into.cast(), value);
    }

    #[inline(always)]
    pub const fn as_limbs(&self) -> &[u64; 4] {
        self.0.as_limbs()
    }

    #[inline(always)]
    pub fn as_limbs_mut(&mut self) -> &mut [u64; 4] {
        self.0.as_limbs_mut()
    }

    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[inline(always)]
    pub fn is_one(&self) -> bool {
        self.0.is_one()
    }

    #[inline(always)]
    pub fn overflowing_add_assign(&mut self, rhs: &Self) -> bool {
        self.0.overflowing_add_assign(&rhs.0)
    }

    #[inline(always)]
    pub fn overflowing_add(mut self, rhs: Self) -> (Self, bool) {
        let carry = self.0.overflowing_add_assign(&rhs.0);
        (self, carry)
    }

    #[inline(always)]
    pub fn overflowing_add_assign_with_carry_propagation(
        &mut self,
        rhs: &Self,
        carry: bool,
    ) -> bool {
        self.0.overflowing_add_assign_with_carry(&rhs.0, carry)
    }

    #[inline(always)]
    pub fn overflowing_sub_assign(&mut self, rhs: &Self) -> bool {
        self.0.overflowing_sub_assign(&rhs.0)
    }

    #[inline(always)]
    pub fn overflowing_sub(mut self, rhs: Self) -> (Self, bool) {
        let borrow = self.0.overflowing_sub_assign(&rhs.0);
        (self, borrow)
    }

    #[inline(always)]
    pub fn overflowing_sub_assign_reversed(&mut self, rhs: &Self) -> bool {
        self.0.overflowing_sub_and_negate_assign(&rhs.0)
    }

    #[inline(always)]
    pub fn wrapping_mul_assign(&mut self, rhs: &Self) {
        self.0.mul_low_assign(&rhs.0);
    }

    #[inline(always)]
    pub fn high_mul_assign(&mut self, rhs: &Self) {
        self.0.mul_high_assign(&rhs.0);
    }

    #[inline(always)]
    pub fn widening_mul_assign(&mut self, rhs: &Self) -> Self {
        let result = self.0.widening_mul_assign(&rhs.0);
        Self(result)
    }

    #[inline(always)]
    pub fn widening_mul_assign_into(&mut self, high: &mut Self, rhs: &Self) {
        self.0.widening_mul_assign_into(&mut high.0, &rhs.0);
    }

    #[inline(always)]
    /// Panics if divisor is 0.
    /// Note: EVM opcodes use the IOOracle-based advice path in zk_ee instead.
    /// This software fallback is used by div_ceil, add_mod, and tests.
    pub fn div_rem(dividend_or_quotient: &mut Self, divisor_or_remainder: &mut Self) {
        let is_zero = divisor_or_remainder.0.is_zero_mut();
        assert!(is_zero == false);

        ruint::algorithms::div(
            dividend_or_quotient.as_limbs_mut(),
            divisor_or_remainder.as_limbs_mut(),
        );
    }

    #[inline(always)]
    /// Panics if divisor is 0
    pub fn div_ceil(dividend_or_quotient: &mut Self, divisor: &Self) {
        let mut divisor_or_remainder = divisor.clone();
        Self::div_rem(dividend_or_quotient, &mut divisor_or_remainder);

        if !divisor_or_remainder.0.is_zero_mut() {
            let overflowed = dividend_or_quotient.overflowing_add_assign(&Self::one());
            assert!(overflowed == false); // Should not ever overflow
        }
    }

    #[inline(always)]
    pub fn not_mut(&mut self) {
        self.0.not_assign()
    }

    pub fn try_from_be_slice(input: &[u8]) -> Option<Self> {
        match input.try_into() {
            Ok(bytes) => Some(Self::from_be_bytes(bytes)),
            Err(_) => None,
        }
    }

    pub fn from_be_bytes(input: &[u8; 32]) -> Self {
        Self(DelegatedU256::from_be_bytes(input))
    }

    pub fn from_le_bytes(input: &[u8; 32]) -> Self {
        Self(DelegatedU256::from_le_bytes(input))
    }

    pub fn to_le_bytes(&self) -> [u8; 32] {
        self.0.to_le_bytes()
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    pub fn write_be_bytes_into(&self, dst: &mut [u8; 32]) {
        self.0.write_be_bytes_into(dst);
    }

    pub fn bit_len(&self) -> usize {
        self.0.bit_len()
    }

    pub fn leading_zeros(&self) -> usize {
        self.0.leading_zeros()
    }

    pub fn byte(&self, byte_idx: usize) -> u8 {
        assert!(byte_idx < 32);
        self.0.byte(byte_idx)
    }

    pub fn bit(&self, bit_idx: usize) -> bool {
        self.0.bit(bit_idx)
    }

    pub fn as_le_bytes_ref(&self) -> &[u8; 32] {
        self.0.as_le_bytes()
    }

    pub fn reduce_mod(&mut self, modulus: &Self) {
        if modulus.is_zero() {
            Self::write_zero(self);
            return;
        }
        if (&*self) >= modulus {
            let mut modulus = modulus.clone();
            Self::div_rem(self, &mut modulus);
            self.clone_from(&modulus);
        }
    }

    pub fn add_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        a.reduce_mod(&modulus_or_result);
        b.reduce_mod(&modulus_or_result);

        let of = unsafe { bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.0, &b.0) != 0 };

        if of || a >= modulus_or_result {
            unsafe { bigint_op_delegation::<SUB_OP_BIT_IDX>(&mut a.0, &modulus_or_result.0) };
        }

        modulus_or_result.clone_from(a);
    }

    /// Note: EVM MULMOD opcode uses the IOOracle-based advice path in zk_ee instead.
    /// This software fallback is used by tests.
    pub fn mul_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        if modulus_or_result.0.is_zero_mut() {
            return;
        }

        let mut product = [0u64; 8];
        let _ = ruint::algorithms::addmul(&mut product, a.as_limbs(), b.as_limbs());
        ruint::algorithms::div(&mut product, modulus_or_result.as_limbs_mut());
    }

    pub fn pow(base: &Self, exp: &Self, dst: &mut Self) {
        // Exponentiation by squaring
        Self::write_one(dst);
        let bits = crate::BitIteratorBE::new_without_leading_zeros(exp.as_limbs());
        for i in bits {
            let tmp = dst.clone();
            Self::wrapping_mul_assign(dst, &tmp);

            if i {
                Self::wrapping_mul_assign(dst, &base);
            }
        }
    }

    pub fn byte_len(&self) -> usize {
        (self.bit_len() + 7) / 8
    }

    pub fn checked_add(&self, rhs: &Self) -> Option<Self> {
        let mut result = self.clone();
        let of = result.overflowing_add_assign(rhs);
        if of {
            None
        } else {
            Some(result)
        }
    }

    pub fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        let mut result = self.clone();
        let of = result.overflowing_sub_assign(rhs);
        if of {
            None
        } else {
            Some(result)
        }
    }

    pub fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        let mut result = self.clone();
        let of = result.0.mul_low_assign(&rhs.0);

        if of {
            None
        } else {
            Some(result)
        }
    }

    #[inline(always)]
    pub fn arithmetic_shr_assign(&mut self, shift: usize) {
        let is_negative = self.bit(255);

        if shift >= 256 {
            if is_negative {
                *self = Self::from_limbs([u64::MAX; 4]);
            } else {
                Self::write_zero(self);
            }
            return;
        }

        if shift == 0 {
            return;
        }

        *self >>= shift as u32;
        if is_negative {
            let mut mask = Self::from_limbs([u64::MAX; 4]);
            mask <<= (256 - shift) as u32;
            core::ops::BitOrAssign::bitor_assign(self, &mask);
        }
    }
}

crate::conversions::impl_conversions!(U256);

impl From<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn from(value: ruint::aliases::U256) -> Self {
        // NOTE: we can not use precompile call due to alignment requirements
        Self::from_limbs(*value.as_limbs())
    }
}

impl From<u64> for U256 {
    #[inline(always)]
    fn from(value: u64) -> Self {
        Self(DelegatedU256::from(value))
    }
}

impl From<u32> for U256 {
    #[inline(always)]
    fn from(value: u32) -> Self {
        Self(DelegatedU256::from(value))
    }
}

impl From<u128> for U256 {
    #[inline(always)]
    fn from(value: u128) -> Self {
        Self(DelegatedU256::from(value))
    }
}

impl Into<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn into(self) -> ruint::aliases::U256 {
        ruint::aliases::U256::from_limbs(self.0.to_limbs())
    }
}

impl TryInto<usize> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<usize, Self::Error> {
        let limbs = self.0.to_limbs();
        if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            if limbs[0] > usize::MAX as u64 {
                Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
            } else {
                Ok(limbs[0] as usize)
            }
        }
    }
}

impl TryInto<u64> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<u64, Self::Error> {
        let limbs = self.0.to_limbs();
        if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            Ok(limbs[0])
        }
    }
}

impl<'a> AddAssign<&'a U256> for U256 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: &'a U256) {
        let _ = self.overflowing_add_assign(rhs);
    }
}

impl<'a> SubAssign<&'a U256> for U256 {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: &'a U256) {
        let _ = self.overflowing_sub_assign(rhs);
    }
}

impl<'a> BitXorAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: &'a U256) {
        self.0 ^= &rhs.0;
    }
}

impl<'a> BitAndAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: &'a U256) {
        self.0 &= &rhs.0;
    }
}

impl<'a> BitOrAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: &'a U256) {
        self.0 |= &rhs.0;
    }
}

impl ShrAssign<u32> for U256 {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: u32) {
        self.0 >>= rhs;
    }
}

impl ShlAssign<u32> for U256 {
    fn shl_assign(&mut self, rhs: u32) {
        self.0 <<= rhs;
    }
}
