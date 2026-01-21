use super::DelegatedU256;

pub const ADD_OP_BIT_IDX: usize = 0;
pub const SUB_OP_BIT_IDX: usize = 1;
pub const SUB_AND_NEGATE_OP_BIT_IDX: usize = 2;
pub const MUL_LOW_OP_BIT_IDX: usize = 3;
pub const MUL_HIGH_OP_BIT_IDX: usize = 4;
pub const EQ_OP_BIT_IDX: usize = 5;

pub const CARRY_BIT_IDX: usize = 6;
pub const MEMCOPY_BIT_IDX: usize = 7;

#[inline(always)]
/// # Safety
/// `a` and `b` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn bigint_op_delegation<const OP_SHIFT: usize>(
    a: *mut DelegatedU256,
    b: *const DelegatedU256,
) -> u32 {
    bigint_op_delegation_with_carry_bit::<OP_SHIFT>(a, b, false)
}

#[cfg(target_arch = "riscv32")]
#[inline(always)]
/// # Safety
/// `a` and `b` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn bigint_op_delegation_with_carry_bit<const OP_SHIFT: usize>(
    a: *mut DelegatedU256,
    b: *const DelegatedU256,
    carry: bool,
) -> u32 {
    debug_assert!(a.cast_const() != b);
    let mut mask = (1u32 << OP_SHIFT) | ((carry as u32) << CARRY_BIT_IDX);

    unsafe {
        core::arch::asm!(
            "csrrw x0, 0x7ca, x0",
            in("x10") a.addr(),
            in("x11") b.addr(),
            inlateout("x12") mask,
            options(nostack, preserves_flags)
        )
    }

    mask
}

#[cfg(not(target_arch = "riscv32"))]
#[inline(always)]
/// # Safety
/// `a` and `b` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn bigint_op_delegation_with_carry_bit<const OP_SHIFT: usize>(
    _a: *mut DelegatedU256,
    _b: *const DelegatedU256,
    carry: bool,
) -> u32 {
    debug_assert!(_a.is_aligned());
    debug_assert!(_b.is_aligned());
    debug_assert!(_a.cast_const() != _b);

    use ruint::aliases::{U256, U512};

    let read = |ptr: *const DelegatedU256| U256::from_limbs((*ptr).0);
    let write = |ptr: *mut DelegatedU256, value: U256| {
        (*ptr).0 = *value.as_limbs();
    };

    let carry_or_borrow = U256::from(carry as u64);

    if OP_SHIFT == ADD_OP_BIT_IDX {
        let a = read(_a);
        let b = read(_b);
        let (t, of0) = a.overflowing_add(b);
        let (t, of1) = t.overflowing_add(carry_or_borrow);
        write(_a, t);

        (of0 || of1) as u32
    } else if OP_SHIFT == SUB_OP_BIT_IDX {
        let a = read(_a);
        let b = read(_b);
        let (t, of0) = a.overflowing_sub(b);
        let (t, of1) = t.overflowing_sub(carry_or_borrow);
        write(_a, t);

        (of0 || of1) as u32
    } else if OP_SHIFT == SUB_AND_NEGATE_OP_BIT_IDX {
        let a = read(_a);
        let b = read(_b);
        let (t, of0) = b.overflowing_sub(a);
        let (t, of1) = t.overflowing_sub(carry_or_borrow);
        write(_a, t);

        (of0 || of1) as u32
    } else if OP_SHIFT == MUL_LOW_OP_BIT_IDX {
        let a = read(_a);
        let b = read(_b);
        let t: U512 = a.widening_mul(b);
        write(_a, U256::from_limbs(t.as_limbs()[..4].try_into().unwrap()));

        t.as_limbs()[4..].iter().any(|el| *el != 0) as u32
    } else if OP_SHIFT == MUL_HIGH_OP_BIT_IDX {
        let a = read(_a);
        let b = read(_b);
        let t: U512 = a.widening_mul(b);
        write(_a, U256::from_limbs(t.as_limbs()[4..8].try_into().unwrap()));

        0
    } else if OP_SHIFT == EQ_OP_BIT_IDX {
        let a = read(_a);
        let b = read(_b);

        (a == b) as u32
    } else if OP_SHIFT == MEMCOPY_BIT_IDX {
        let b = read(_b);
        let (t, of) = b.overflowing_add(carry_or_borrow);
        write(_a, t);

        of as u32
    } else {
        panic!("unknown op")
    }
}
