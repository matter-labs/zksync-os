use super::copy::*;
use super::{delegation::*, DelegatedU256};
use core::cmp::Ordering;
use core::ops::{BitAndAssign, BitOrAssign, ShlAssign, ShrAssign};
use core::{mem::MaybeUninit, ops::BitXorAssign};

// Immutable statics land in `.rodata` and are valid delegation operands without a runtime
// initialization
pub static ZERO: DelegatedU256 = DelegatedU256::ZERO;
pub static ONE: DelegatedU256 = DelegatedU256::ONE;
pub static MAX: DelegatedU256 = DelegatedU256::MAX;

impl PartialEq for DelegatedU256 {
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            // equality is non-destructive, so we can cast
            let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(
                (self as *const Self).cast_mut(),
                other as *const Self,
            );

            eq != 0
        }
    }
}

impl Eq for DelegatedU256 {}

impl Ord for DelegatedU256 {
    fn cmp(&self, other: &Self) -> Ordering {
        unsafe {
            let scratch = copy_to_scratch(self as *const Self);
            let other = other as *const Self;
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
    pub const MAX: Self = Self([u64::MAX; 4]);

    pub fn zero() -> Self {
        #[allow(static_mut_refs)]
        unsafe {
            copy_from_operand(core::ptr::addr_of!(ZERO))
        }
    }

    pub fn one() -> Self {
        #[allow(static_mut_refs)]
        unsafe {
            copy_from_operand(core::ptr::addr_of!(ONE))
        }
    }

    pub fn write_zero(&mut self) {
        #[allow(static_mut_refs)]
        unsafe {
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
                self as *mut Self,
                core::ptr::addr_of!(ZERO),
            );
        }
    }

    pub fn write_one(&mut self) {
        #[allow(static_mut_refs)]
        unsafe {
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
                self as *mut Self,
                core::ptr::addr_of!(ONE),
            );
        }
    }

    pub fn is_zero_mut(&mut self) -> bool {
        #[allow(static_mut_refs)]
        let eq = unsafe {
            bigint_op_delegation::<EQ_OP_BIT_IDX>(self as *mut Self, core::ptr::addr_of!(ZERO))
        };

        eq != 0
    }

    pub fn is_zero(&self) -> bool {
        self.eq_static(core::ptr::addr_of!(ZERO))
    }

    pub fn is_one(&self) -> bool {
        self.eq_static(core::ptr::addr_of!(ONE))
    }

    /// `self == 2^256 - 1`
    pub fn is_max(&self) -> bool {
        self.eq_static(core::ptr::addr_of!(MAX))
    }

    #[inline(always)]
    fn eq_static(&self, constant: *const Self) -> bool {
        // we can cast constness since equality is non-destructive
        let eq = unsafe {
            bigint_op_delegation::<EQ_OP_BIT_IDX>((self as *const Self).cast_mut(), constant)
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
            let carry =
                bigint_op_delegation::<ADD_OP_BIT_IDX>(self as *mut Self, rhs as *const Self);
            carry != 0
        }
    }

    pub fn overflowing_add_assign_with_carry(&mut self, rhs: &Self, carry: bool) -> bool {
        unsafe {
            let carry = bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(
                self as *mut Self,
                rhs as *const Self,
                carry,
            );

            carry != 0
        }
    }

    pub fn overflowing_sub_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            let borrow =
                bigint_op_delegation::<SUB_OP_BIT_IDX>(self as *mut Self, rhs as *const Self);

            borrow != 0
        }
    }

    pub fn overflowing_sub_assign_with_borrow(&mut self, rhs: &Self, borrow: bool) -> bool {
        unsafe {
            let borrow = bigint_op_delegation_with_carry_bit::<SUB_OP_BIT_IDX>(
                self as *mut Self,
                rhs as *const Self,
                borrow,
            );

            borrow != 0
        }
    }

    pub fn overflowing_sub_and_negate_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            let borrow = bigint_op_delegation::<SUB_AND_NEGATE_OP_BIT_IDX>(
                self as *mut Self,
                rhs as *const Self,
            );

            borrow != 0
        }
    }

    pub fn mul_low_assign(&mut self, rhs: &Self) -> bool {
        unsafe {
            let of =
                bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self as *mut Self, rhs as *const Self);

            of != 0
        }
    }

    pub fn mul_high_assign(&mut self, rhs: &Self) {
        unsafe {
            bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(self as *mut Self, rhs as *const Self);
        }
    }

    pub fn widening_mul_assign(&mut self, rhs: &Self) -> Self {
        unsafe {
            let mut result = MaybeUninit::<Self>::uninit();
            bigint_op_delegation::<MEMCOPY_BIT_IDX>(result.as_mut_ptr(), self as *const Self);

            bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self as *mut Self, rhs as *const Self);
            bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(result.as_mut_ptr(), rhs as *const Self);

            result.assume_init()
        }
    }

    pub fn widening_mul_assign_into(&mut self, high: &mut Self, rhs: &Self) {
        unsafe {
            bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(self as *mut Self, rhs as *const Self);
            bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(high as *mut Self, rhs as *const Self);
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
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(operand, core::ptr::addr_of!(ZERO));
    }
}

/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_one_into_ptr(operand: *mut DelegatedU256) {
    #[allow(static_mut_refs)]
    unsafe {
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(operand, core::ptr::addr_of!(ONE));
    }
}

/// Write a u64 value as a U256 directly into the target pointer.
/// Zeros the slot first via delegation, then sets limb[0].
///
/// # Safety
/// `operand` must be 32-byte aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_u64_into_ptr(operand: *mut DelegatedU256, value: u64) {
    #[allow(static_mut_refs)]
    unsafe {
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(operand, core::ptr::addr_of!(ZERO));
    }
    unsafe {
        (*operand).as_limbs_mut()[0] = value;
    }
}
