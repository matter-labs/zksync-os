use super::{delegation::*, DelegatedU256};
use core::mem::MaybeUninit;

static mut SCRATCH_FOR_MUT: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();
#[cfg(target_arch = "riscv32")]
static mut SCRATCH_FOR_REF: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();

#[cfg(target_arch = "riscv32")]
const ROM_BOUND: usize = 1 << 21;

#[inline(always)]
pub(super) unsafe fn copy_from_operand(source: *const DelegatedU256) -> DelegatedU256 {
    let mut result = MaybeUninit::<DelegatedU256>::uninit();
    unsafe {
        with_ram_operand(source, |src_ptr| {
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(result.as_mut_ptr(), src_ptr);
        });

        result.assume_init()
    }
}

impl Clone for DelegatedU256 {
    #[inline(always)]
    fn clone(&self) -> Self {
        unsafe { copy_from_operand(self as *const Self) }
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        unsafe {
            with_ram_operand(source.0.as_ptr().cast(), |src_ptr| {
                let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
                    self.0.as_mut_ptr().cast(),
                    src_ptr.cast(),
                );
            })
        }
    }
}

/// # Safety
/// `dst` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_into_ptr(dst: *mut DelegatedU256, source: &DelegatedU256) {
    unsafe {
        with_ram_operand(source as *const DelegatedU256, |src| {
            bigint_op_delegation::<MEMCOPY_BIT_IDX>(dst, src);
        })
    }
}

/// # Safety
/// `src` must be allocated in non ROM.
/// `dst` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_into_ptr_unchecked(dst: *mut DelegatedU256, source: &DelegatedU256) {
    unsafe {
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(dst, source);
    }
}

/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
#[inline(always)]
pub(super) unsafe fn with_ram_operand<T, F: FnMut(*const DelegatedU256) -> T>(
    operand: *const DelegatedU256,
    mut f: F,
) -> T {
    #[cfg(target_arch = "riscv32")]
    {
        let mut scratch_mu = MaybeUninit::<DelegatedU256>::uninit();

        let scratch_ptr = if operand.addr() < ROM_BOUND {
            scratch_mu.as_mut_ptr().write(operand.read());
            scratch_mu.as_ptr()
        } else {
            operand
        };

        f(scratch_ptr)
    }

    #[cfg(not(target_arch = "riscv32"))]
    {
        f(operand)
    }
}

#[inline(always)]
/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub(super) unsafe fn copy_to_scratch(operand: *const DelegatedU256) -> *mut DelegatedU256 {
    #[cfg(target_arch = "riscv32")]
    {
        if operand.addr() < ROM_BOUND {
            SCRATCH_FOR_MUT.as_mut_ptr().write(operand.read());
            SCRATCH_FOR_MUT.as_mut_ptr()
        } else {
            // otherwise we can just use precompile
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
                SCRATCH_FOR_MUT.as_mut_ptr().cast(),
                operand.cast(),
            );
            SCRATCH_FOR_MUT.as_mut_ptr()
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    #[allow(static_mut_refs)]
    {
        SCRATCH_FOR_MUT.as_mut_ptr().write(operand.read());
        SCRATCH_FOR_MUT.as_mut_ptr()
    }
}

#[inline(always)]
/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn copy_if_needed(operand: *const DelegatedU256) -> *const DelegatedU256 {
    #[cfg(target_arch = "riscv32")]
    unsafe {
        if operand.addr() < ROM_BOUND {
            SCRATCH_FOR_REF.write(operand.read());
            SCRATCH_FOR_REF.as_ptr()
        } else {
            operand
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    {
        operand
    }
}
