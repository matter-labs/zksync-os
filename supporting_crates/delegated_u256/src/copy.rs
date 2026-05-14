use super::{delegation::*, DelegatedU256};
use core::mem::MaybeUninit;
use core::ptr::addr_of_mut;

static mut SCRATCH_FOR_MUT: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();
#[cfg(target_arch = "riscv32")]
static mut SCRATCH_FOR_REF: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();

#[cfg(target_arch = "riscv32")]
const ROM_BOUND: usize = 1 << 21;

impl Clone for DelegatedU256 {
    #[inline(always)]
    fn clone(&self) -> Self {
        // custom clone by using precompile
        // NOTE on all uses of such initialization - we do not want to check if compiler will elide stack-to-stack copy
        // upon the call of `assume_init` in general, but we know that all underlying data will be overwritten and initialized
        unsafe {
            // We have to do `uninit().assume_init()` because calling `assume_init()` later may trigger a stack-to-stack copy
            // And this is safe because there are no references to result, and on risc-v all memory is init by default
            #[allow(invalid_value)]
            #[allow(clippy::uninit_assumed_init)]
            let mut result = MaybeUninit::<Self>::uninit().assume_init();
            with_ram_operand(self.0.as_ptr().cast(), |src_ptr| {
                let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(&mut result as *mut Self, src_ptr);
            });
            result
        }
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
        let scratch = addr_of_mut!(SCRATCH_FOR_MUT).cast::<DelegatedU256>();
        if operand.addr() < ROM_BOUND {
            scratch.write(operand.read());
            scratch
        } else {
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(scratch, operand.cast());
            scratch
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    {
        let scratch = addr_of_mut!(SCRATCH_FOR_MUT).cast::<DelegatedU256>();
        scratch.write(operand.read());
        scratch
    }
}

#[inline(always)]
/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn copy_if_needed(operand: *const DelegatedU256) -> *const DelegatedU256 {
    #[cfg(target_arch = "riscv32")]
    unsafe {
        if operand.addr() < ROM_BOUND {
            let scratch = addr_of_mut!(SCRATCH_FOR_REF).cast::<DelegatedU256>();
            scratch.write(operand.read());
            scratch as *const DelegatedU256
        } else {
            operand
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    {
        operand
    }
}
