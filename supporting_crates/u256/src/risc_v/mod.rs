#[derive(Hash, serde::Serialize, serde::Deserialize)]
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
            let other = crypto::bigint_riscv::aligned_copy_if_needed(other.0.as_ptr().cast());
            // equality is non-destructing
            let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(scratch.cast_mut().cast(), other.cast());
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
        // TODO
        core::fmt::Result::Ok(())
    }
}

impl core::fmt::Debug for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // TODO
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
    // pub const ZERO: Self = Self([0u64; 4]);
    // pub const ONE: Self = Self([1u64, 0u64, 0u64, 0u64]);

    pub const fn from_limbs(limbs: [u64; 4]) -> Self {
        Self(limbs)
    }

    pub unsafe fn write_into(dst: *mut Self, source: &Self) {
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

    pub fn bytereverse_u256(&mut self) {
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
        crypto::bigint_riscv::write_zero_into(into.0.as_mut_ptr().cast());
    }

    #[inline(always)]
    pub fn write_one(into: &mut Self) {
        crypto::bigint_riscv::write_one_into(into.0.as_mut_ptr().cast());
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
    pub fn div_assign_with_remainder(&mut self, rem: &mut Self, divisor: &Self) {
        // Eventually it'll be solved via non-determinism and comparison that a = q * divisor + r,
        // but for now it's just a naive one

        unsafe {
            let src_ptr = aligned_copy_if_needed(divisor.0.as_ptr().cast());
            bigint_op_delegation::<MEMCOPY_BIT_IDX>(rem.0.as_mut_ptr().cast(), src_ptr.cast());
            let is_zero = crypto::bigint_riscv::is_zero_mut(rem.0.as_mut_ptr().cast());
            assert!(is_zero == false);
            ruint::algorithms::div(&mut self.0, &mut rem.0);
        }
    }

    #[inline(always)]
    pub fn not_mut(&mut self) {
        self.0[0] = !self.0[0];
        self.0[1] = !self.0[1];
        self.0[2] = !self.0[2];
        self.0[3] = !self.0[3];
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

impl Into<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn into(self) -> ruint::aliases::U256 {
        ruint::aliases::U256::from_limbs(self.0)
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

impl Not for U256 {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self::Output {
        Self([!self.0[0], !self.0[1], !self.0[2], !self.0[3]])
    }
}

impl ShrAssign<u32> for U256 {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: u32) {
        let (limbs, bits) = (rhs / 64, rhs % 64);

        match limbs {
            0 => {
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
            1 => {
                // let compiler optimize
                self.0[2] = self.0[3];
                self.0[1] = self.0[2];
                self.0[0] = self.0[1];
                self.0[3] = 0;

                let mut carry = self.0[2] << (64 - bits);
                self.0[2] >>= bits;
                let t = self.0[1] << (64 - bits);
                self.0[1] = self.0[1] >> bits | carry;
                carry = t;
                self.0[0] = self.0[0] >> bits | carry;
            }
            2 => {
                self.0[1] = self.0[3];
                self.0[0] = self.0[2];
                self.0[2] = 0;
                self.0[3] = 0;

                let carry = self.0[1] << (64 - bits);
                self.0[1] >>= bits;
                self.0[0] = self.0[0] >> bits | carry;
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
        let (limbs, bits) = (rhs / 64, rhs % 64);

        match limbs {
            0 => {
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
            1 => {
                // let compiler optimize
                self.0[1] = self.0[0];
                self.0[2] = self.0[1];
                self.0[3] = self.0[2];
                self.0[0] = 0;

                let mut carry = self.0[1] >> (64 - bits);
                self.0[1] <<= bits;
                let t = self.0[2] >> (64 - bits);
                self.0[2] = self.0[2] << bits | carry;
                carry = t;
                self.0[3] = self.0[3] << bits | carry;
            }
            2 => {
                self.0[2] = self.0[0];
                self.0[3] = self.0[1];
                self.0[0] = 0;
                self.0[1] = 0;

                let carry = self.0[2] >> (64 - bits);
                self.0[2] <<= bits;
                self.0[3] = self.0[3] << bits | carry;
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
