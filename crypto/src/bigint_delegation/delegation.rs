use super::u256::U256;
use crate::BigIntOps;

#[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
const CARRY_BIT_IDX: usize = 6;

#[inline(always)]
pub(super) fn add(a: &mut U256, b: &U256) -> u32 {
    bigint_op_delegation(a, b, BigIntOps::Add)
}

#[inline(always)]
pub(super) fn sub(a: &mut U256, b: &U256) -> u32 {
    bigint_op_delegation(a, b, BigIntOps::Sub)
}

#[inline(always)]
pub(super) fn sub_and_negate(a: &mut U256, b: &U256) -> u32 {
    bigint_op_delegation(a, b, BigIntOps::SubAndNegate)
}

#[inline(always)]
pub(super) fn mul_low(a: &mut U256, b: &U256) {
    bigint_op_delegation(a, b, BigIntOps::MulLow);
}

#[inline(always)]
pub(super) fn mul_high(a: &mut U256, b: &U256) {
    bigint_op_delegation(a, b, BigIntOps::MulHigh);
}

#[inline(always)]
pub(super) fn eq(a: &U256, b: &U256) -> u32 {
    let a = a as *const _ as *mut _;
    bigint_op_delegation(a, b, BigIntOps::Eq)
}

#[inline(always)]
pub(super) fn memcpy(a: &mut U256, b: &U256) {
    bigint_op_delegation(a, b, BigIntOps::MemCpy);
}

#[inline(always)]
pub(super) fn sub_with_carry_bit(a: &mut U256, b: &U256, carry: bool) -> u32 {
    bigint_op_delegation_with_carry_bit(a, b, carry, BigIntOps::Sub)
}

#[inline(always)]
pub(super) fn add_with_carry_bit(a: &mut U256, b: &U256, carry: bool) -> u32 {
    bigint_op_delegation_with_carry_bit(a, b, carry, BigIntOps::Add)
}

#[inline(always)]
pub(super) fn sub_and_negate_with_carry_bit(a: &mut U256, b: &U256, carry: bool) -> u32 {
    bigint_op_delegation_with_carry_bit(a, b, carry, BigIntOps::SubAndNegate)
}

#[inline(always)]
fn bigint_op_delegation(a: *mut U256, b: *const U256, op: BigIntOps) -> u32 {
    bigint_op_delegation_with_carry_bit(a, b, false, op)
}

#[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
#[inline(always)]
pub(crate) fn bigint_op_delegation_with_carry_bit(
    a: *mut U256,
    b: *const U256,
    carry: bool,
    op: BigIntOps,
) -> u32 {
    debug_assert!(a.cast_const() != b);

    let a_adrr = a.addr();
    let b_adrr = b.addr();

    debug_assert!(a_adrr % 32 == 0);
    debug_assert!(b_adrr % 32 == 0);

    let mut mask = (1u32 << (op as usize)) | ((carry as u32) << CARRY_BIT_IDX);

    unsafe {
        core::arch::asm!(
            "csrrw x0, 0x7ca, x0",
            in("x10") a_adrr,
            in("x11") b_adrr,
            inlateout("x12") mask,
            options(nostack, preserves_flags)
        )
    }

    mask
}

#[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
#[inline(always)]
pub(crate) fn bigint_op_delegation_with_carry_bit(
    _a_ptr: *mut U256,
    _b_ptr: *const U256,
    _carry: bool,
    _op: BigIntOps,
) -> u32 {
    debug_assert!(_a_ptr.cast_const() != _b_ptr);
    debug_assert!(_a_ptr.addr() % 32 == 0);
    debug_assert!(_b_ptr.addr() % 32 == 0);

    #[cfg(any(feature = "testing", test))]
    unsafe {
        use ruint::aliases::{U256 as rU256, U512 as rU512};

        use core::ptr::{addr_of, addr_of_mut};

        let read = |ptr: *const U256| rU256::from_limbs(addr_of!((*ptr).0).read());
        let write = |ptr: *mut U256, value: rU256| {
            addr_of_mut!((*ptr).0).write(*value.as_limbs());
        };

        let carry_or_borrow = rU256::from(_carry as u64);
        match _op {
            BigIntOps::Add => {
                let a = read(_a_ptr);
                let b = read(_b_ptr);
                let (t, of0) = a.overflowing_add(b);
                let (t, of1) = t.overflowing_add(carry_or_borrow);
                write(_a_ptr, t);

                (of0 || of1) as u32
            }
            BigIntOps::Sub => {
                let a = read(_a_ptr);
                let b = read(_b_ptr);
                let (t, of0) = a.overflowing_sub(b);
                let (t, of1) = t.overflowing_sub(carry_or_borrow);
                write(_a_ptr, t);

                (of0 || of1) as u32
            }
            BigIntOps::SubAndNegate => {
                let a = read(_a_ptr);
                let b = read(_b_ptr);
                let (t, of0) = b.overflowing_sub(a);
                let (t, of1) = t.overflowing_sub(carry_or_borrow);
                write(_a_ptr, t);

                (of0 || of1) as u32
            }
            BigIntOps::MulLow => {
                let a = read(_a_ptr);
                let b = read(_b_ptr);
                let t: rU512 = a.widening_mul(b);
                write(
                    _a_ptr,
                    rU256::from_limbs(t.as_limbs()[..4].try_into().unwrap()),
                );

                t.as_limbs()[4..].iter().any(|el| *el != 0) as u32
            }
            BigIntOps::MulHigh => {
                let a = read(_a_ptr);
                let b = read(_b_ptr);
                let t: rU512 = a.widening_mul(b);
                write(
                    _a_ptr,
                    rU256::from_limbs(t.as_limbs()[4..8].try_into().unwrap()),
                );

                0
            }
            BigIntOps::MemCpy => {
                let b = read(_b_ptr);
                let (t, of) = b.overflowing_add(carry_or_borrow);
                write(_a_ptr, t);

                of as u32
            }
            BigIntOps::Eq => {
                let a = read(_a_ptr);
                let b = read(_b_ptr);
                (a == b) as u32
            }
        }
    }

    #[cfg(not(any(feature = "testing", test)))]
    unimplemented!()
}
