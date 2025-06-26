// This type is just some marker for API to say that "we expect not just [u32; 8], but something with alignment".
// Of course caller can supply unaligned pointer, but at least we ask to think about it

use core::mem::MaybeUninit;

#[allow(dead_code)]
#[repr(align(32))]
#[derive(Clone, Copy)]
pub struct AlignedPrecompileSpace(pub(crate) [u32; 8]);

pub const ZERO_REPR_CONST: AlignedPrecompileSpace = AlignedPrecompileSpace([0u32; 8]);
pub const ONE_REPR_CONST: AlignedPrecompileSpace = AlignedPrecompileSpace([1, 0, 0, 0, 0, 0, 0, 0]);

pub const ADD_OP_BIT_IDX: usize = 0;
pub const SUB_OP_BIT_IDX: usize = 1;
pub const SUB_AND_NEGATE_OP_BIT_IDX: usize = 2;
pub const MUL_LOW_OP_BIT_IDX: usize = 3;
pub const MUL_HIGH_OP_BIT_IDX: usize = 4;
pub const EQ_OP_BIT_IDX: usize = 5;

pub const CARRY_BIT_IDX: usize = 6;
pub const MEMCOPY_BIT_IDX: usize = 7;

#[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
const ROM_BOUND: usize = 1 << 21;

static mut SCRATCH_FOR_MUT: core::mem::MaybeUninit<AlignedPrecompileSpace> =
    core::mem::MaybeUninit::uninit();
#[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
static mut SCRATCH_FOR_REF: core::mem::MaybeUninit<AlignedPrecompileSpace> =
    core::mem::MaybeUninit::uninit();
#[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
static mut SCRATCH_FOR_REF_2: core::mem::MaybeUninit<AlignedPrecompileSpace> =
    core::mem::MaybeUninit::uninit();
static mut ZERO_REPR: core::mem::MaybeUninit<AlignedPrecompileSpace> =
    core::mem::MaybeUninit::uninit();
static mut ONE_REPR: core::mem::MaybeUninit<AlignedPrecompileSpace> =
    core::mem::MaybeUninit::uninit();

pub fn init() {
    unsafe {
        ZERO_REPR.write(ZERO_REPR_CONST);
        ONE_REPR.write(ONE_REPR_CONST);
    }
}

#[inline(always)]
/// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn is_zero(operand: *const AlignedPrecompileSpace) -> bool {
    // it'll copy into scratch if it's not in the mutable region
    let src = aligned_copy_if_needed(operand);
    // so we can just cast constness, as equality is non-overwriting
    let eq =
        bigint_op_delegation::<EQ_OP_BIT_IDX>(src.cast_mut().cast(), ZERO_REPR.as_ptr().cast());

    eq != 0
}

#[inline(always)]
/// Same as `is_zero`, but assumes the the `operand` is in modifyable memory segment.
/// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn is_zero_mut(operand: *mut AlignedPrecompileSpace) -> bool {
    let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(operand.cast(), ZERO_REPR.as_ptr().cast());

    eq != 0
}

#[inline(always)]
/// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn is_one(operand: *const AlignedPrecompileSpace) -> bool {
    // it'll copy into scratch if it's not in the mutable region
    let src = aligned_copy_if_needed(operand);
    // so we can just cast constness, as equality is non-overwriting
    let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(src.cast_mut().cast(), ONE_REPR.as_ptr().cast());

    eq != 0
}

#[inline(always)]
/// Same as `is_one`, but assumes the the `operand` is in modifyable memory segment.
/// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn is_one_mut(operand: *mut AlignedPrecompileSpace) -> bool {
    let eq = bigint_op_delegation::<EQ_OP_BIT_IDX>(operand.cast(), ONE_REPR.as_ptr().cast());

    eq != 0
}

// #[inline(always)]
// /// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
// pub unsafe fn copy_to_scratch(
//     operand: *const AlignedPrecompileSpace,
// ) -> *mut AlignedPrecompileSpace {
//     #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
//     {
//         if operand.addr() < ROM_BOUND {
//             SCRATCH_FOR_MUT.as_mut_ptr().write(operand.read());
//             SCRATCH_FOR_MUT.as_mut_ptr()
//         } else {
//             // otherwise we can just use precompile
//             let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
//                 SCRATCH_FOR_MUT.as_mut_ptr().cast(),
//                 operand.cast(),
//             );
//             SCRATCH_FOR_MUT.as_mut_ptr()
//         }
//     }
//
//     #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
//     {
//         SCRATCH_FOR_MUT.as_mut_ptr().write(operand.read());
//         SCRATCH_FOR_MUT.as_mut_ptr()
//     }
// }

#[inline(always)]
pub unsafe fn with_ram_operand<T, F: FnMut(*const AlignedPrecompileSpace) -> T>(
    operand: *const AlignedPrecompileSpace,
    mut f: F
) -> T {
    #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
    {
        let mut scratch_mu = MaybeUninit::<AlignedPrecompileSpace>::uninit();

        let scratch_ptr = if operand.addr() < ROM_BOUND {
            scratch_mu.as_mut_ptr().write(operand.read());
            scratch_mu.as_ptr()
        } else {
            operand
        };

        f(scratch_ptr)
    }

    #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
    {
        f(operand)
    }
}

#[inline(always)]
pub unsafe fn aligned_copy_if_needed(
    operand: *const AlignedPrecompileSpace,
) -> *const AlignedPrecompileSpace {
    #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
    unsafe {
        if operand.addr() < ROM_BOUND {
            SCRATCH_FOR_REF.as_mut_ptr().write(operand.read());
            SCRATCH_FOR_REF.as_ptr()
        } else {
            operand
        }
    }

    #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
    {
        operand
    }
}

/// # Safety
/// TODO: document safety
#[inline(always)]
pub unsafe fn aligned_copy_if_needed_2(
    operand: *const AlignedPrecompileSpace,
) -> *const AlignedPrecompileSpace {
    #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
    unsafe {
        if operand.addr() < ROM_BOUND {
            SCRATCH_FOR_REF_2.as_mut_ptr().write(operand.read());
            SCRATCH_FOR_REF_2.as_ptr()
        } else {
            operand
        }
    }

    #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
    {
        operand
    }
}

#[allow(dead_code)]
#[inline(always)]
/// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub(crate) fn copy_if_needed(operand: *const [u32; 8]) -> *const [u32; 8] {
    #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
    unsafe {
        if operand.addr() < ROM_BOUND {
            SCRATCH_FOR_REF
                .as_mut_ptr()
                .cast::<[u32; 8]>()
                .write(operand.read());
            SCRATCH_FOR_REF.as_ptr().cast()
        } else {
            operand
        }
    }

    #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
    {
        operand
    }
}

/// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
#[inline(always)]
pub unsafe fn write_zero_into(operand: *mut AlignedPrecompileSpace) {
    #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
    unsafe {
        let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(operand.cast(), ZERO_REPR.as_ptr().cast());
    }

    #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
    unsafe {
        operand.write(ZERO_REPR_CONST);
    }
}

/// Safety: `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
#[inline(always)]
pub unsafe fn write_one_into(operand: *mut AlignedPrecompileSpace) {
    #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
    unsafe {
        let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(operand.cast(), ONE_REPR.as_ptr().cast());
    }

    #[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
    unsafe {
        operand.write(ONE_REPR_CONST);
    }
}

#[inline(always)]
pub fn bigint_op_delegation<const OP_SHIFT: usize>(a: *mut u32, b: *const u32) -> u32 {
    bigint_op_delegation_with_carry_bit::<OP_SHIFT>(a, b, false)
}

#[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
#[inline(always)]
// TODO: This is a dup of crypto::bigint_delegation::delegation::bigint_op_delegation_with_carry_bit
pub fn bigint_op_delegation_with_carry_bit<const OP_SHIFT: usize>(
    a: *mut u32,
    b: *const u32,
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


#[cfg(not(all(target_arch = "riscv32", feature = "bigint_ops")))]
#[inline(always)]
pub fn bigint_op_delegation_with_carry_bit<const OP_SHIFT: usize>(
    a: *mut u32,
    b: *const u32,
    carry: bool,
) -> u32 {
    // #[cfg(test)]
    unsafe {
        use ruint::aliases::{U256, U512};
        fn make_u256(words: &[u32; 8]) -> U256 {
            unsafe {
                let mut result = U256::ZERO;
                for (dst, [l, h]) in result
                    .as_limbs_mut()
                    .iter_mut()
                    .zip(words.array_chunks::<2>())
                {
                    *dst = ((*h as u64) << 32) | (*l as u64);
                }

                result
            }
        }

        let a = a.cast::<[u32; 8]>().read();
        let b = b.cast::<[u32; 8]>().read();
        let a = make_u256(&a);
        let b = make_u256(&b);
        let carry_or_borrow = U256::from(carry as u64);

        let result;
        let of = if OP_SHIFT == ADD_OP_BIT_IDX {
            let (t, of0) = a.overflowing_add(b);
            let (t, of1) = t.overflowing_add(carry_or_borrow);
            result = t;

            of0 || of1
        } else if OP_SHIFT == SUB_OP_BIT_IDX {
            let (t, of0) = a.overflowing_sub(b);
            let (t, of1) = t.overflowing_sub(carry_or_borrow);
            result = t;

            of0 || of1
        } else if OP_SHIFT == SUB_AND_NEGATE_OP_BIT_IDX {
            let (t, of0) = b.overflowing_sub(a);
            let (t, of1) = t.overflowing_sub(carry_or_borrow);
            result = t;

            of0 || of1
        } else if OP_SHIFT == MUL_LOW_OP_BIT_IDX {
            let t: U512 = a.widening_mul(b);
            result = U256::from_limbs(t.as_limbs()[..4].try_into().unwrap());

            t.as_limbs()[4..].iter().any(|el| *el != 0)
        } else if OP_SHIFT == MUL_HIGH_OP_BIT_IDX {
            let t: U512 = a.widening_mul(b);
            result = U256::from_limbs(t.as_limbs()[4..8].try_into().unwrap());

            false
        } else if OP_SHIFT == EQ_OP_BIT_IDX {
            result = a;

            a == b
        } else if OP_SHIFT == MEMCOPY_BIT_IDX {
            let (t, of) = b.overflowing_add(carry_or_borrow);
            result = t;

            of
        } else {
            panic!("unknown op")
        };

        let mut low_result = [0u32; 8];
        for ([l, h], src) in low_result
            .array_chunks_mut::<2>()
            .zip(result.as_limbs().iter())
        {
            *l = *src as u32;
            *h = (*src >> 32) as u32;
        }

        a.cast::<[u32; 8]>().write(low_result);

        of as u32
    }
}
