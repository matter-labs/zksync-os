#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[repr(align(32))]
pub struct U256([u64; 4]);

use core::mem::MaybeUninit;

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

impl U256 {
    pub const ZERO: Self = Self([0u64; 4]);
    pub const ONE: Self = Self([1u64, 0u64, 0u64, 0u64]);

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
    pub fn overflowing_add_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            let src_ptr = aligned_copy_if_needed(rhs.0.as_ptr().cast());
            let carry =
                bigint_op_delegation::<ADD_OP_BIT_IDX>(self.0.as_mut_ptr().cast(), src_ptr.cast());
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
}

impl From<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn from(value: ruint::aliases::U256) -> Self {
        // NOTE: we can not use precompile call due to alignment requirements
        Self(*value.as_limbs())
    }
}

impl Into<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn into(self) -> ruint::aliases::U256 {
        ruint::aliases::U256::from_limbs(self.0)
    }
}
