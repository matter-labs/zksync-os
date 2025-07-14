use super::copy::*;
use super::{delegation::*, DelegatedU256};
use core::cmp::Ordering;
use core::ops::{BitAndAssign, BitOrAssign, ShlAssign, ShrAssign};
use core::{mem::MaybeUninit, ops::BitXorAssign};

pub static mut ZERO: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();
pub static mut ONE: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();

pub(super) fn init() {
    #[allow(static_mut_refs)]
    unsafe {
        ZERO.write(DelegatedU256::ZERO);
        ONE.write(DelegatedU256::ONE);
    }
}

impl PartialEq for DelegatedU256 {
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            // maybe copy values into scratch if they live in ROM
            with_ram_operand(self as *const Self, |scratch| {
                with_ram_operand(other as *const Self, |scratch_2| {
                    // equality is non-destructive, so we can cast
                    let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(scratch.cast_mut(), scratch_2);

                    eq != 0
                })
            })
        }
    }
}

impl Eq for DelegatedU256 {}

impl Ord for DelegatedU256 {
    fn cmp(&self, other: &Self) -> Ordering {
        unsafe {
            let scratch = copy_to_scratch(self as *const Self);
            with_ram_operand(other as *const Self, |other| {
                let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(scratch, other);
                if eq != 0 {
                    Ordering::Equal
                } else {
                    let borrow = bigint_op_delegation::<SUB_OP_BIT_IDX>(scratch, other);
                    if borrow != 0 {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
            })
        }
    }
}

impl PartialOrd for DelegatedU256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl DelegatedU256 {
    pub const ZERO: Self = Self([0; 4]);
    pub const ONE: Self = Self([1, 0, 0, 0]);

    pub fn zero() -> Self {
        #[allow(invalid_value)]
        #[allow(clippy::uninit_assumed_init)]
        // `result.assume_init()` may trigger stack-to-stack copy, so we can't do it later
        // This is safe because there are no references to result and it's initialized immediately
        // (and on RISC-V all memory is init by default)
        let mut result: Self = unsafe { MaybeUninit::uninit().assume_init() };
        result.write_zero();
        result
    }

    pub fn one() -> Self {
        #[allow(invalid_value)]
        #[allow(clippy::uninit_assumed_init)]
        // `result.assume_init()` may trigger stack-to-stack copy, so we can't do it later
        // This is safe because there are no references to result and it's initialized immediately
        // (and on RISC-V all memory is init by default)
        let mut result: Self = unsafe { MaybeUninit::uninit().assume_init() };
        result.write_one();
        result
    }

    pub fn write_zero(&mut self) {
        #[allow(static_mut_refs)]
        unsafe {
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(self as *mut Self, ZERO.as_ptr());
        }
    }

    pub fn write_one(&mut self) {
        #[allow(static_mut_refs)]
        unsafe {
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(self as *mut Self, ONE.as_ptr());
        }
    }

    pub fn is_zero_mut(&mut self) -> bool {
        #[allow(static_mut_refs)]
        let eq = unsafe { bigint_op_delegation::<EQ_OP_BIT_IDX>(self as *mut Self, ZERO.as_ptr()) };

        eq != 0
    }

    pub fn is_zero(&self) -> bool {
        let eq = unsafe {
            let src = copy_if_needed(self as *const Self);
            // we can cast constness since equality is non-destructive
            #[allow(static_mut_refs)]
            bigint_op_delegation::<EQ_OP_BIT_IDX>(src.cast_mut(), ZERO.as_ptr())
        };

        eq != 0
    }

    pub fn is_one(&self) -> bool {
        let eq = unsafe {
            let src = copy_if_needed(self as *const Self);
            // we can cast constness since equality is non-destructive
            #[allow(static_mut_refs)]
            bigint_op_delegation::<EQ_OP_BIT_IDX>(src.cast_mut(), ONE.as_ptr())
        };

        eq != 0
    }

    pub fn is_odd(&self) -> bool {
        self.0[0] & 1 == 1
    }

    pub fn is_even(&self) -> bool {
        !self.is_odd()
    }

    pub fn overflowing_add_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(self as *mut Self, rhs_ptr);
                carry != 0
            })
        }
    }

    pub fn overflowing_add_assign_with_carry(&mut self, rhs: &Self, carry: bool) -> bool {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                let carry = bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(
                    self as *mut Self,
                    rhs_ptr,
                    carry,
                );

                carry != 0
            })
        }
    }

    pub fn overflowing_sub_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                let borrow = bigint_op_delegation::<SUB_OP_BIT_IDX>(self as *mut Self, rhs_ptr);

                borrow != 0
            })
        }
    }

    pub fn overflowing_sub_assign_with_borrow(&mut self, rhs: &Self, borrow: bool) -> bool {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                let borrow = bigint_op_delegation_with_carry_bit::<SUB_OP_BIT_IDX>(
                    self as *mut Self,
                    rhs_ptr,
                    borrow,
                );

                borrow != 0
            })
        }
    }

    pub fn overflowing_sub_and_negate_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                let borrow =
                    bigint_op_delegation::<SUB_AND_NEGATE_OP_BIT_IDX>(self as *mut Self, rhs_ptr);

                borrow != 0
            })
        }
    }

    pub fn mul_low_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                let of = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self as *mut Self, rhs_ptr);

                of != 0
            })
        }
    }

    pub fn mul_high_assign(&mut self, rhs: &Self) {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(self as *mut Self, rhs_ptr);
            })
        }
    }

    pub fn widening_mul_assign(&mut self, rhs: &Self) -> Self {
        unsafe {
            #[allow(invalid_value)]
            #[allow(clippy::uninit_assumed_init)]
            // `result.assume_init()` may trigger stack-to-stack copy, so we can't do it later
            // This is safe because there are no references to result and it's initialized immediately
            // (and on RISC-V all memory is init by default)
            let mut result = MaybeUninit::uninit().assume_init();
            // no need to copy to scratch since self cannot be in ROM
            bigint_op_delegation::<MEMCOPY_BIT_IDX>(&mut result as *mut Self, self as *const Self);

            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self as *mut Self, rhs_ptr);
                bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(&mut result as *mut Self, rhs_ptr);
            });

            result
        }
    }

    pub fn widening_mul_assign_into(&mut self, high: &mut Self, rhs: &Self) {
        unsafe {
            with_ram_operand(rhs as *const Self, |rhs_ptr| {
                bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self as *mut Self, rhs_ptr);
                bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(high as *mut Self, rhs_ptr);
            })
        }
    }

    pub fn not_assign(&mut self) {
        self.0[0] = !self.0[0];
        self.0[1] = !self.0[1];
        self.0[2] = !self.0[2];
        self.0[3] = !self.0[3];
    }
}

impl From<u8> for DelegatedU256 {
    fn from(value: u8) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;
        result
    }
}

impl From<u16> for DelegatedU256 {
    fn from(value: u16) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;
        result
    }
}

impl From<u32> for DelegatedU256 {
    fn from(value: u32) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;
        result
    }
}

impl From<u64> for DelegatedU256 {
    fn from(value: u64) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value;
        result
    }
}

impl From<u128> for DelegatedU256 {
    fn from(value: u128) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;
        result.as_limbs_mut()[1] = (value >> 64) as u64;
        result
    }
}

impl<'a> BitXorAssign<&'a Self> for DelegatedU256 {
    fn bitxor_assign(&mut self, rhs: &'a Self) {
        self.0[0] ^= rhs.0[0];
        self.0[1] ^= rhs.0[1];
        self.0[2] ^= rhs.0[2];
        self.0[3] ^= rhs.0[3];
    }
}

impl<'a> BitAndAssign<&'a Self> for DelegatedU256 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: &'a Self) {
        self.0[0] &= rhs.0[0];
        self.0[1] &= rhs.0[1];
        self.0[2] &= rhs.0[2];
        self.0[3] &= rhs.0[3];
    }
}

impl<'a> BitOrAssign<&'a Self> for DelegatedU256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: &'a Self) {
        self.0[0] |= rhs.0[0];
        self.0[1] |= rhs.0[1];
        self.0[2] |= rhs.0[2];
        self.0[3] |= rhs.0[3];
    }
}

impl ShrAssign<u32> for DelegatedU256 {
    fn shr_assign(&mut self, rhs: u32) {
        if rhs != 0 {
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
                    self.write_zero();
                }
            }
        }
    }
}

impl ShlAssign<u32> for DelegatedU256 {
    fn shl_assign(&mut self, rhs: u32) {
        if rhs != 0 {
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
                    self.write_zero();
                }
            }
        }
    }
}

/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_zero_into_ptr(operand: *mut DelegatedU256) {
    #[allow(static_mut_refs)]
    unsafe {
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(operand, ZERO.as_ptr());
    }
}

/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_one_into_ptr(operand: *mut DelegatedU256) {
    #[allow(static_mut_refs)]
    unsafe {
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(operand, ONE.as_ptr());
    }
}
