#[derive(Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(align(32))]
pub struct U256([u64; 4]);

use core::mem::MaybeUninit;
use core::{ops::*, u64};

use crypto::bigint_riscv::*;

impl Clone for U256 {
    #[inline(always)]
    fn clone(&self) -> Self {
        // custom clone by using precompile
        // NOTE on all uses of such initialization - we do not want to check if compiler will elide stack-to-stack copy
        // upon the call of `assume_init` in general, but we know that all underlying data will be overwritten and initialized
        unsafe {
            #[allow(invalid_value)]
            let mut result = MaybeUninit::<Self>::uninit().assume_init();
            let src_ptr = aligned_copy_if_needed(self.0.as_ptr().cast());
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
                result.0.as_mut_ptr().cast(),
                src_ptr.cast(),
            );

            result
        }
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        unsafe {
            let src_ptr = aligned_copy_if_needed(source.0.as_ptr().cast());
            let _ =
                bigint_op_delegation::<MEMCOPY_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
        }
    }
}

impl core::cmp::PartialEq for U256 {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            // aligned copy will make copy into scratch, and comparison is non-destructive, so we copy and recast
            let scratch = crypto::bigint_riscv::aligned_copy_if_needed(self.0.as_ptr().cast());
            let scratch_2 = crypto::bigint_riscv::aligned_copy_if_needed_2(other.0.as_ptr().cast());
            // equality is non-destructing
            let eq =
                bigint_op_delegation::<EQ_OP_BIT_IDX>(scratch.cast_mut().cast(), scratch_2.cast());
            eq != 0
        }
    }
}

impl core::cmp::Eq for U256 {}

impl core::cmp::Ord for U256 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // we use scratch space to get mutable memory for our comparisons
        unsafe {
            let scratch = crypto::bigint_riscv::copy_to_scratch(self.0.as_ptr().cast());
            let other = crypto::bigint_riscv::aligned_copy_if_needed(other.0.as_ptr().cast());
            // equality is non-destructing
            let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(scratch.cast(), other.cast());
            if eq != 0 {
                return core::cmp::Ordering::Equal;
            }
            let borrow = bigint_op_delegation::<SUB_OP_BIT_IDX>(scratch.cast(), other.cast());
            if borrow != 0 {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            }
        }
    }
}

impl core::cmp::PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(core::cmp::Ord::cmp(self, other))
    }
}

impl core::fmt::Display for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::Debug for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for word in self.as_limbs().iter().rev() {
            write!(f, "{:016x}", word)?;
        }

        core::fmt::Result::Ok(())
    }
}

impl core::default::Default for U256 {
    #[inline(always)]
    fn default() -> Self {
        Self::zero()
    }
}

impl U256 {
    pub const ZERO: Self = Self([0u64; 4]);
    // const ONE: Self = Self([1u64, 0u64, 0u64, 0u64]);

    pub const BYTES: usize = 32;

    pub const fn from_limbs(limbs: [u64; 4]) -> Self {
        Self(limbs)
    }

    pub unsafe fn write_into_ptr(dst: *mut Self, source: &Self) {
        unsafe {
            let src_ptr = aligned_copy_if_needed(source.0.as_ptr().cast());
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(dst.cast(), src_ptr.cast());
        }
    }

    #[inline(always)]
    pub fn zero() -> Self {
        unsafe {
            #[allow(invalid_value)]
            let mut result = MaybeUninit::<Self>::uninit().assume_init();
            crypto::bigint_riscv::write_zero_into(result.0.as_mut_ptr().cast());

            result
        }
    }

    #[inline(always)]
    pub fn one() -> Self {
        unsafe {
            #[allow(invalid_value)]
            let mut result = MaybeUninit::<Self>::uninit().assume_init();
            crypto::bigint_riscv::write_one_into(result.0.as_mut_ptr().cast());

            result
        }
    }

    pub fn bytereverse(&mut self) {
        unsafe {
            let limbs = self.as_limbs_mut();
            core::ptr::swap(&mut limbs[0] as *mut u64, &mut limbs[3] as *mut u64);
            core::ptr::swap(&mut limbs[1] as *mut u64, &mut limbs[2] as *mut u64);
            for limb in limbs.iter_mut() {
                *limb = limb.swap_bytes();
            }
        }
    }

    #[inline(always)]
    pub fn write_zero(into: &mut Self) {
        unsafe {
            crypto::bigint_riscv::write_zero_into(into.0.as_mut_ptr().cast());
        }
    }

    #[inline(always)]
    pub fn write_one(into: &mut Self) {
        unsafe {
            crypto::bigint_riscv::write_one_into(into.0.as_mut_ptr().cast());
        }
    }

    #[inline(always)]
    pub unsafe fn write_zero_into_ptr(into: *mut Self) {
        unsafe {
            crypto::bigint_riscv::write_zero_into(into.cast());
        }
    }

    #[inline(always)]
    pub unsafe fn write_one_into_ptr(into: *mut Self) {
        unsafe {
            crypto::bigint_riscv::write_one_into(into.cast());
        }
    }

    #[inline(always)]
    pub const fn as_limbs(&self) -> &[u64; 4] {
        &self.0
    }

    #[inline(always)]
    pub fn as_limbs_mut(&mut self) -> &mut [u64; 4] {
        &mut self.0
    }

    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        unsafe { crypto::bigint_riscv::is_zero(self.0.as_ptr().cast()) }
    }

    #[inline(always)]
    pub fn is_one(&self) -> bool {
        unsafe { crypto::bigint_riscv::is_one(self.0.as_ptr().cast()) }
    }

    #[inline(always)]
    pub fn overflowing_add_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            let carry =
                bigint_op_delegation::<ADD_OP_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
            carry != 0
        }
    }

    #[inline(always)]
    pub fn overflowing_add_assign_with_carry_propagation(
        &mut self,
        rhs: &Self,
        carry_in: bool,
    ) -> bool {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            let carry = bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(
                self.0.as_mut_ptr().cast(),
                src_ptr.cast(),
                carry_in,
            );

            carry != 0
        }
    }

    #[inline(always)]
    pub fn overflowing_sub_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            let borrow =
                bigint_op_delegation::<SUB_OP_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
            borrow != 0
        }
    }

    #[inline(always)]
    pub fn overflowing_sub_assign_reversed(&mut self, rhs: &Self) -> bool {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            let borrow = bigint_op_delegation::<SUB_AND_NEGATE_OP_BIT_IDX>(
                self.0.as_mut_ptr().cast(),
                src_ptr.cast(),
            );
            borrow != 0
        }
    }

    #[inline(always)]
    pub fn wrapping_mul_assign(&mut self, rhs: &Self) {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
        }
    }

    #[inline(always)]
    pub fn high_mul_assign(&mut self, rhs: &Self) {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
        }
    }

    #[inline(always)]
    pub fn widening_mul_assign(&mut self, rhs: &Self) -> Self {
        unsafe {
            #[allow(invalid_value)]
            let mut result = MaybeUninit::<Self>::uninit().assume_init();
            let src_ptr = aligned_copy_if_needed(self.0.as_ptr().cast());
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
                result.0.as_mut_ptr().cast(),
                src_ptr.cast(),
            );

            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
            bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(
                result.0.as_mut_ptr().cast(),
                src_ptr.cast(),
            );

            result
        }
    }

    #[inline(always)]
    pub fn widening_mul_assign_into(&mut self, high: &mut Self, rhs: &Self) {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
            bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(high.0.as_mut_ptr().cast(), src_ptr.cast());
        }
    }

    #[inline(always)]
    /// Panics if divisor is 0
    pub fn div_rem(dividend_or_quotient: &mut Self, divisor_or_remainder: &mut Self) {
        // Eventually it'll be solved via non-determinism and comparison that a = q * divisor + r,
        // but for now it's just a naive one

        unsafe {
            let is_zero =
                crypto::bigint_riscv::is_zero_mut(divisor_or_remainder.0.as_mut_ptr().cast());
            assert!(is_zero == false);
            ruint::algorithms::div(&mut dividend_or_quotient.0, &mut divisor_or_remainder.0);
        }
    }

    #[inline(always)]
    /// Panics if divisor is 0
    pub fn div_ceil(dividend_or_quotient: &mut Self, divisor: &Self) {
        let mut divisor_or_remainder = divisor.clone();
        Self::div_rem(dividend_or_quotient, &mut divisor_or_remainder);

        if !divisor_or_remainder.is_zero() {
            let overflowed = dividend_or_quotient.overflowing_add_assign(&Self::one());
            assert!(overflowed == false); // Should not ever overflow
        }
    }

    #[inline(always)]
    pub fn not_mut(&mut self) {
        self.0[0] = !self.0[0];
        self.0[1] = !self.0[1];
        self.0[2] = !self.0[2];
        self.0[3] = !self.0[3];
    }

    pub fn try_from_be_slice(input: &[u8]) -> Option<Self> {
        match input.try_into() {
            Ok(bytes) => Some(Self::from_be_bytes(bytes)),
            Err(_) => None,
        }
    }

    pub fn from_be_bytes(input: &[u8; 32]) -> Self {
        unsafe {
            #[allow(invalid_value)]
            let mut result = MaybeUninit::<Self>::uninit().assume_init();
            let src = input.as_ptr_range().end.cast::<[u8; 8]>();
            result
                .0
                .as_mut_ptr()
                .write(u64::from_be_bytes(src.sub(1).read()));
            result
                .0
                .as_mut_ptr()
                .add(1)
                .write(u64::from_be_bytes(src.sub(2).read()));
            result
                .0
                .as_mut_ptr()
                .add(2)
                .write(u64::from_be_bytes(src.sub(3).read()));
            result
                .0
                .as_mut_ptr()
                .add(3)
                .write(u64::from_be_bytes(src.sub(4).read()));

            result
        }
    }

    pub fn from_le_bytes(input: &[u8; 32]) -> Self {
        unsafe {
            #[allow(invalid_value)]
            let mut result = MaybeUninit::<Self>::uninit().assume_init();
            let src = input.as_ptr().cast::<[u8; 8]>();
            result.0.as_mut_ptr().write(u64::from_le_bytes(src.read()));
            result
                .0
                .as_mut_ptr()
                .add(1)
                .write(u64::from_le_bytes(src.add(1).read()));
            result
                .0
                .as_mut_ptr()
                .add(2)
                .write(u64::from_le_bytes(src.add(2).read()));
            result
                .0
                .as_mut_ptr()
                .add(3)
                .write(u64::from_le_bytes(src.add(3).read()));

            result
        }
    }

    pub fn to_le_bytes(&self) -> [u8; 32] {
        unsafe { core::mem::transmute_copy(&self.0) }
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        unsafe {
            let mut limbs = self.0;
            core::ptr::swap(&mut limbs[0] as *mut u64, &mut limbs[3] as *mut u64);
            core::ptr::swap(&mut limbs[1] as *mut u64, &mut limbs[2] as *mut u64);
            for limb in limbs.iter_mut() {
                *limb = limb.swap_bytes();
            }
            core::mem::transmute(limbs)
        }
    }

    pub fn bit_len(&self) -> usize {
        let mut len = 256usize;
        for el in self.0.iter().rev() {
            if *el == 0 {
                len -= 64;
            } else {
                len -= el.leading_zeros() as usize;
                return len;
            }
        }

        len
    }

    pub fn byte(&self, byte_idx: usize) -> u8 {
        if byte_idx >= 32 {
            0
        } else {
            self.as_le_bytes_ref()[byte_idx]
        }
    }

    pub fn bit(&self, bit_idx: usize) -> bool {
        if bit_idx >= 256 {
            false
        } else {
            let (word, bit_idx) = (bit_idx / 64, bit_idx % 64);
            self.0[word] & 1 << bit_idx != 0
        }
    }

    pub fn as_le_bytes_ref(&self) -> &[u8; 32] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub fn reduce_mod(&mut self, modulus: &Self) {
        if modulus.is_zero() {
            Self::write_zero(self);
            return;
        }
        if (&*self).le(modulus) {
            let mut modulus = modulus.clone();
            Self::div_rem(self, &mut modulus);
            Clone::clone_from(self, &modulus);
        }
    }

    pub fn add_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        a.reduce_mod(&*modulus_or_result);
        b.reduce_mod(&*modulus_or_result);
        let of =
            bigint_op_delegation::<ADD_OP_BIT_IDX>(a.0.as_mut_ptr().cast(), b.0.as_ptr().cast());
        if of != 0 || (&*a).gt(&*modulus_or_result) {
            let _ = Self::overflowing_sub_assign_reversed(modulus_or_result, &*a);
        }
    }

    pub fn mul_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        if modulus_or_result.is_zero() {
            return;
        }

        let mut product = [a.clone(), a.clone()];
        let (low, high) = product.split_at_mut(1);
        Self::widening_mul_assign_into(&mut low[0], &mut high[0], &*b);
        let product: &mut [u64; 8] = unsafe { core::mem::transmute(&mut product[0]) };
        ruint::algorithms::div(product, modulus_or_result.as_limbs_mut());
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
        let of = unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            let of = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(
                result.0.as_mut_ptr().cast(),
                src_ptr.cast(),
            );

            of != 0
        };

        if of {
            None
        } else {
            Some(result)
        }
    }
}

impl From<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn from(value: ruint::aliases::U256) -> Self {
        // NOTE: we can not use precompile call due to alignment requirements
        Self(*value.as_limbs())
    }
}

impl From<u64> for U256 {
    #[inline(always)]
    fn from(value: u64) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value;

        result
    }
}

impl From<u32> for U256 {
    #[inline(always)]
    fn from(value: u32) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;

        result
    }
}

impl From<u128> for U256 {
    #[inline(always)]
    fn from(value: u128) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;
        result.as_limbs_mut()[1] = (value >> 64) as u64;

        result
    }
}

impl Into<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn into(self) -> ruint::aliases::U256 {
        ruint::aliases::U256::from_limbs(self.0)
    }
}

impl TryInto<usize> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<usize, Self::Error> {
        if self.0[3] != 0 || self.0[2] != 0 || self.0[1] != 0 {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            if self.0[0] > usize::MAX as u64 {
                Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
            } else {
                Ok(self.0[0] as usize)
            }
        }
    }
}

impl TryInto<u64> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<u64, Self::Error> {
        if self.0[3] != 0 || self.0[2] != 0 || self.0[1] != 0 {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            Ok(self.0[0])
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
        self.0[0] ^= rhs.0[0];
        self.0[1] ^= rhs.0[1];
        self.0[2] ^= rhs.0[2];
        self.0[3] ^= rhs.0[3];
    }
}

impl<'a> BitAndAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: &'a U256) {
        self.0[0] &= rhs.0[0];
        self.0[1] &= rhs.0[1];
        self.0[2] &= rhs.0[2];
        self.0[3] &= rhs.0[3];
    }
}

impl<'a> BitOrAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: &'a U256) {
        self.0[0] |= rhs.0[0];
        self.0[1] |= rhs.0[1];
        self.0[2] |= rhs.0[2];
        self.0[3] |= rhs.0[3];
    }
}

impl ShrAssign<u32> for U256 {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: u32) {
        if rhs == 0 {
            return;
        }
        let (limbs, bits) = (rhs / 64, rhs % 64);

        match limbs {
            0 => {
                if bits != 0 {
                    let mut carry = self.0[3] << (64 - bits);
                    self.0[3] >>= bits;
                    let t = self.0[2] << (64 - bits);
                    self.0[2] = self.0[2] >> bits | carry;
                    carry = t;
                    let t = self.0[1] << (64 - bits);
                    self.0[1] = self.0[1] >> bits | carry;
                    carry = t;
                    self.0[0] = self.0[0] >> bits | carry;
                }
            }
            1 => {
                // let compiler optimize
                self.0[0] = self.0[1];
                self.0[1] = self.0[2];
                self.0[2] = self.0[3];
                self.0[3] = 0;

                if bits != 0 {
                    let mut carry = self.0[2] << (64 - bits);
                    self.0[2] >>= bits;
                    let t = self.0[1] << (64 - bits);
                    self.0[1] = self.0[1] >> bits | carry;
                    carry = t;
                    self.0[0] = self.0[0] >> bits | carry;
                }
            }
            2 => {
                self.0[0] = self.0[2];
                self.0[1] = self.0[3];
                self.0[2] = 0;
                self.0[3] = 0;

                if bits != 0 {
                    let carry = self.0[1] << (64 - bits);
                    self.0[1] >>= bits;
                    self.0[0] = self.0[0] >> bits | carry;
                }
            }
            3 => {
                self.0[0] = self.0[3];
                self.0[1] = 0;
                self.0[2] = 0;
                self.0[3] = 0;

                self.0[0] >>= bits;
            }
            _ => {
                Self::write_zero(self);
            }
        }
    }
}

impl ShlAssign<u32> for U256 {
    fn shl_assign(&mut self, rhs: u32) {
        if rhs == 0 {
            return;
        }

        let (limbs, bits) = (rhs / 64, rhs % 64);

        match limbs {
            0 => {
                if bits != 0 {
                    let mut carry = self.0[0] >> (64 - bits);
                    self.0[0] <<= bits;
                    let t = self.0[1] >> (64 - bits);
                    self.0[1] = self.0[1] << bits | carry;
                    carry = t;
                    let t = self.0[2] >> (64 - bits);
                    self.0[2] = self.0[2] << bits | carry;
                    carry = t;
                    self.0[3] = self.0[3] << bits | carry;
                }
            }
            1 => {
                // let compiler optimize
                self.0[3] = self.0[2];
                self.0[2] = self.0[1];
                self.0[1] = self.0[0];
                self.0[0] = 0;

                if bits != 0 {
                    let mut carry = self.0[1] >> (64 - bits);
                    self.0[1] <<= bits;
                    let t = self.0[2] >> (64 - bits);
                    self.0[2] = self.0[2] << bits | carry;
                    carry = t;
                    self.0[3] = self.0[3] << bits | carry;
                }
            }
            2 => {
                self.0[3] = self.0[1];
                self.0[2] = self.0[0];
                self.0[1] = 0;
                self.0[0] = 0;

                if bits != 0 {
                    let carry = self.0[2] >> (64 - bits);
                    self.0[2] <<= bits;
                    self.0[3] = self.0[3] << bits | carry;
                }
            }
            3 => {
                self.0[3] = self.0[0];
                self.0[0] = 0;
                self.0[1] = 0;
                self.0[2] = 0;

                self.0[3] <<= bits;
            }
            _ => {
                Self::write_zero(self);
            }
        }
    }
}
