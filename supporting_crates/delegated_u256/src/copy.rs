use super::{delegation::*, DelegatedU256};
use core::mem::MaybeUninit;

static mut SCRATCH_FOR_MUT: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();

#[inline(always)]
pub(super) unsafe fn copy_from_operand(source: *const DelegatedU256) -> DelegatedU256 {
    let mut result = MaybeUninit::<DelegatedU256>::uninit();
    unsafe {
        let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(result.as_mut_ptr(), source);

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
            let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(
                self.0.as_mut_ptr().cast(),
                source.0.as_ptr().cast(),
            );
        }
    }
}

/// # Safety
/// `dst` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_into_ptr(dst: *mut DelegatedU256, source: &DelegatedU256) {
    unsafe {
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(dst, source);
    }
}

/// # Safety
/// `dst` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub unsafe fn write_into_ptr_unchecked(dst: *mut DelegatedU256, source: &DelegatedU256) {
    unsafe {
        bigint_op_delegation::<MEMCOPY_BIT_IDX>(dst, source);
    }
}

#[inline(always)]
/// Copies the operand into the static scratch slot, for the destructive
/// delegations (e.g. a subtraction used as a comparison) on a shared reference.
///
/// # Safety
/// `operand` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub(super) unsafe fn copy_to_scratch(operand: *const DelegatedU256) -> *mut DelegatedU256 {
    #[allow(static_mut_refs)]
    unsafe {
        let _ = bigint_op_delegation::<MEMCOPY_BIT_IDX>(SCRATCH_FOR_MUT.as_mut_ptr(), operand);
        SCRATCH_FOR_MUT.as_mut_ptr()
    }
}
