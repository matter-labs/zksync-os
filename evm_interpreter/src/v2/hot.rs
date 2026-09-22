//! The register-resident part of the frame state, and the resource charging on it.

use core::marker::PhantomData;

use u256::U256;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::system::evm::EvmError;
use zk_ee::system::{Computational, ErgsResource, EthereumLikeTypes, Resource, Resources};

use super::{fatal_exit, HotFrameParts};
use crate::gas_constants::MEMORY;
use crate::native_resource_constants::{
    HEAP_EXPANSION_BASE_NATIVE_COST, HEAP_EXPANSION_PER_BYTE_NATIVE_COST, STEP_NATIVE_COST,
};
use crate::{ExitCode, InstructionResult, STACK_SIZE};

const SLOT: usize = core::mem::size_of::<U256>();

/// The top of the stack: a pointer to the next free slot, which addresses the operands with
/// immediate offsets, and the depth, which the bounds checks compare against constants.
///
/// The methods update the pointer first and derive the operand addresses from the new value,
/// so that only one stack register is live across an instruction.
pub(crate) struct StackTop {
    pub sp: *mut U256,
    pub depth: usize,
}

impl StackTop {
    /// Fails unless the stack holds at least `n` values
    #[inline(always)]
    fn require(&self, n: usize) -> InstructionResult {
        if self.depth < n {
            Err(EvmError::StackUnderflow.into())
        } else {
            Ok(())
        }
    }

    /// Reserves the next slot for a push. The slot is uninitialized.
    #[inline(always)]
    fn push_slot(&mut self) -> Result<*mut U256, ExitCode> {
        if self.depth >= STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        self.sp = self.sp.wrapping_add(1);
        self.depth += 1;
        Ok(self.sp.wrapping_sub(1))
    }

    /// Reserves the next slot for a push and zeroes it
    #[inline(always)]
    pub fn push_slot_zeroed(&mut self) -> Result<*mut U256, ExitCode> {
        let slot = self.push_slot()?;
        // SAFETY: a free slot of the stack
        unsafe { U256::write_zero_into_ptr(slot) };
        Ok(slot)
    }

    #[inline(always)]
    pub fn pop_and_ignore(&mut self) -> InstructionResult {
        self.require(1)?;
        self.sp = self.sp.wrapping_sub(1);
        self.depth -= 1;
        Ok(())
    }

    /// Pops one value, returned as a pointer to its (still valid) slot
    #[inline(always)]
    pub fn pop_1_ptr(&mut self) -> Result<*const U256, ExitCode> {
        self.require(1)?;
        self.sp = self.sp.wrapping_sub(1);
        self.depth -= 1;
        Ok(self.sp)
    }

    /// Pops two values; the first one was on top
    #[inline(always)]
    pub fn pop_2_ptr(&mut self) -> Result<(*const U256, *const U256), ExitCode> {
        self.require(2)?;
        self.sp = self.sp.wrapping_sub(2);
        self.depth -= 2;
        Ok((self.sp.wrapping_add(1), self.sp))
    }

    #[inline(always)]
    fn pop_3_ptr(&mut self) -> Result<(*const U256, *const U256, *const U256), ExitCode> {
        self.require(3)?;
        self.sp = self.sp.wrapping_sub(3);
        self.depth -= 3;
        Ok((self.sp.wrapping_add(2), self.sp.wrapping_add(1), self.sp))
    }

    #[inline(always)]
    fn pop_4_ptr(
        &mut self,
    ) -> Result<(*const U256, *const U256, *const U256, *const U256), ExitCode> {
        self.require(4)?;
        self.sp = self.sp.wrapping_sub(4);
        self.depth -= 4;
        Ok((
            self.sp.wrapping_add(3),
            self.sp.wrapping_add(2),
            self.sp.wrapping_add(1),
            self.sp,
        ))
    }

    /// The top of the stack, which stays where it is
    #[inline(always)]
    pub fn top_ptr(&mut self) -> Result<*mut U256, ExitCode> {
        self.require(1)?;
        Ok(self.sp.wrapping_sub(1))
    }

    /// Pops one value and exposes the new top: the shape of a binary operation
    #[inline(always)]
    fn pop_1_and_peek_ptr(&mut self) -> Result<(*mut U256, *mut U256), ExitCode> {
        self.require(2)?;
        self.sp = self.sp.wrapping_sub(1);
        self.depth -= 1;
        Ok((self.sp, self.sp.wrapping_sub(1)))
    }

    /// Pops two values and exposes the new top
    #[inline(always)]
    fn pop_2_and_peek_ptr(&mut self) -> Result<(*mut U256, *mut U256, *mut U256), ExitCode> {
        self.require(3)?;
        self.sp = self.sp.wrapping_sub(2);
        self.depth -= 2;
        Ok((self.sp.wrapping_add(1), self.sp, self.sp.wrapping_sub(1)))
    }

    #[inline(always)]
    pub fn pop_1(&mut self) -> Result<&U256, ExitCode> {
        // SAFETY: an initialized slot of the stack
        self.pop_1_ptr().map(|p| unsafe { &*p })
    }

    #[inline(always)]
    pub fn pop_2(&mut self) -> Result<(&U256, &U256), ExitCode> {
        // SAFETY: initialized slots of the stack
        self.pop_2_ptr().map(|(a, b)| unsafe { (&*a, &*b) })
    }

    #[inline(always)]
    pub fn pop_3(&mut self) -> Result<(&U256, &U256, &U256), ExitCode> {
        // SAFETY: initialized slots of the stack
        self.pop_3_ptr().map(|(a, b, c)| unsafe { (&*a, &*b, &*c) })
    }

    #[inline(always)]
    pub fn pop_4(&mut self) -> Result<(&U256, &U256, &U256, &U256), ExitCode> {
        // SAFETY: initialized slots of the stack
        self.pop_4_ptr()
            .map(|(a, b, c, d)| unsafe { (&*a, &*b, &*c, &*d) })
    }

    #[inline(always)]
    pub fn top_mut(&mut self) -> Result<&mut U256, ExitCode> {
        // SAFETY: an initialized slot of the stack
        self.top_ptr().map(|p| unsafe { &mut *p })
    }

    #[inline(always)]
    pub fn pop_1_and_peek_mut(&mut self) -> Result<(&U256, &mut U256), ExitCode> {
        // SAFETY: two distinct initialized slots of the stack
        self.pop_1_and_peek_ptr()
            .map(|(a, b)| unsafe { (&*a, &mut *b) })
    }

    #[inline(always)]
    pub fn pop_1_mut_and_peek(&mut self) -> Result<(&mut U256, &mut U256), ExitCode> {
        // SAFETY: two distinct initialized slots of the stack
        self.pop_1_and_peek_ptr()
            .map(|(a, b)| unsafe { (&mut *a, &mut *b) })
    }

    #[inline(always)]
    pub fn pop_2_mut_and_peek(&mut self) -> Result<((&mut U256, &mut U256), &mut U256), ExitCode> {
        // SAFETY: three distinct initialized slots of the stack
        self.pop_2_and_peek_ptr()
            .map(|(a, b, c)| unsafe { ((&mut *a, &mut *b), &mut *c) })
    }

    #[inline(always)]
    pub fn dup<const N: usize>(&mut self) -> InstructionResult {
        // one compare for both bounds: `N <= depth < STACK_SIZE`
        if self.depth.wrapping_sub(N) >= STACK_SIZE - N {
            return Err(if self.depth < N {
                EvmError::StackUnderflow.into()
            } else {
                EvmError::StackOverflow.into()
            });
        }
        self.sp = self.sp.wrapping_add(1);
        self.depth += 1;
        // SAFETY: the source is an initialized slot, the destination is the slot just
        // reserved; both are in the stack buffer, which is in RAM
        unsafe {
            U256::write_into_ptr_unchecked(self.sp.wrapping_sub(1), &*self.sp.wrapping_sub(1 + N));
        }
        Ok(())
    }

    #[inline(always)]
    pub fn swap<const N: usize>(&mut self) -> InstructionResult {
        self.require(N + 1)?;
        let a = self.sp.wrapping_sub(1);
        let b = self.sp.wrapping_sub(N + 1);
        // SAFETY: two distinct initialized slots of the stack, in RAM
        unsafe {
            let mut tmp = core::mem::MaybeUninit::<U256>::uninit();
            U256::write_into_ptr_unchecked(tmp.as_mut_ptr(), &*a);
            U256::write_into_ptr_unchecked(a, &*b);
            U256::write_into_ptr_unchecked(b, tmp.assume_init_ref());
        }
        Ok(())
    }

    #[inline(always)]
    pub fn push_zero(&mut self) -> InstructionResult {
        self.push_slot_zeroed().map(|_| ())
    }

    #[inline(always)]
    pub fn push_u64(&mut self, value: u64) -> InstructionResult {
        let slot = self.push_slot()?;
        // SAFETY: a free slot of the stack
        unsafe { U256::write_u64_into_ptr(slot, value) };
        Ok(())
    }

    #[inline(always)]
    pub fn push_b160(&mut self, address: ruint::aliases::B160) -> InstructionResult {
        let slot = self.push_slot_zeroed()?;
        // SAFETY: an initialized slot of the stack
        unsafe {
            let limbs = (*slot).as_limbs_mut();
            limbs[0] = address.as_limbs()[0];
            limbs[1] = address.as_limbs()[1];
            limbs[2] = address.as_limbs()[2];
        }
        Ok(())
    }

    /// Pushes a copy of `value`, which may live anywhere (also in ROM)
    #[inline(always)]
    pub fn push(&mut self, value: &U256) -> InstructionResult {
        let slot = self.push_slot()?;
        // SAFETY: a free slot of the stack
        unsafe { U256::write_into_ptr(slot, value) };
        Ok(())
    }
}

/// State touched by (almost) every instruction, in registers for a run of the loop.
///
/// Built from the [`HotFrameParts`] of the frame, which it borrows for its whole life, and
/// written back by [`Hot::finish`]. Only plain values, never borrowed by an outlined call
/// (see [`Hot::outlined`]), so that it can stay in registers.
pub(crate) struct Hot<'h, S: EthereumLikeTypes> {
    /// Next byte to execute (or read as an immediate)
    pub ip: *const u8,
    /// One past the last byte of the code
    pub code_end: *const u8,
    /// The stack
    pub stack: StackTop,
    /// Resources of the frame. For the Ethereum STF this is one `u64` of gas.
    pub resources: S::Resources,
    /// Where the values came from and go back to
    holder: *mut HotFrameParts<S>,
    _borrow: PhantomData<&'h mut HotFrameParts<S>>,
}

impl<'h, S: EthereumLikeTypes> Hot<'h, S> {
    /// `outlined` copies the state bitwise; nothing in it may own a resource
    const NO_DROP_GLUE: () = assert!(!core::mem::needs_drop::<Self>());

    /// Takes the hot state out of the holder for a run over `code`
    #[inline(always)]
    pub fn new(holder: &'h mut HotFrameParts<S>, code: &[u8]) -> Self {
        let depth = holder.stack.depth();
        Self {
            ip: code.as_ptr().wrapping_add(holder.instruction_pointer),
            code_end: code.as_ptr().wrapping_add(code.len()),
            stack: StackTop {
                sp: holder.stack.base_ptr().wrapping_add(depth),
                depth,
            },
            resources: core::mem::replace(&mut holder.resources, S::Resources::empty()),
            holder: holder as *mut HotFrameParts<S>,
            _borrow: PhantomData,
        }
    }

    /// Puts the state back into the holder; `code_start` is that of `new`
    #[inline(always)]
    pub fn finish(self, code_start: *const u8) {
        let Self {
            ip,
            stack,
            resources,
            holder,
            ..
        } = self;
        // SAFETY: `holder` is the exclusive borrow of `new`, held for `'h`
        let holder = unsafe { &mut *holder };
        holder.instruction_pointer = ip.addr().wrapping_sub(code_start.addr());
        // SAFETY: `depth` counts the initialized prefix of the buffer
        unsafe { holder.stack.set_depth(stack.depth) };
        holder.resources = resources;
    }

    /// Runs an outlined instruction on a copy of the state and takes the copy back.
    ///
    /// The copy is what the callee's `&mut Self` points to, so the address of the state
    /// itself never escapes: passing it to a real call would keep it in memory for the whole
    /// loop.
    #[inline(always)]
    pub fn outlined<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let () = Self::NO_DROP_GLUE;
        // SAFETY: a bitwise move out and back in, with no drop in between (no drop glue), and
        // the original is not read while the copy is live
        unsafe {
            let mut copy = core::ptr::read(self);
            let result = f(&mut copy);
            core::ptr::write(self, copy);
            result
        }
    }

    /// The opcode at `ip`, advancing past it; `STOP` past the end of the code (the pointer
    /// still advances, as the frame's instruction pointer used to)
    #[cfg(target_arch = "riscv32")]
    #[inline(always)]
    pub fn fetch_and_advance(&mut self) -> u8 {
        self.ip = self.ip.wrapping_add(1);
        if self.ip <= self.code_end {
            // SAFETY: in bounds of the code
            unsafe { self.ip.sub(1).read() }
        } else {
            crate::opcodes::STOP
        }
    }

    /// The opcode at `ip` (for the host tracer, which sees the frame before the step)
    #[cfg(not(target_arch = "riscv32"))]
    #[inline(always)]
    pub fn peek(&self) -> u8 {
        if self.ip < self.code_end {
            // SAFETY: in bounds of the code
            unsafe { self.ip.read() }
        } else {
            crate::opcodes::STOP
        }
    }

    /// Remaining EVM gas
    #[inline(always)]
    pub fn gas_left(&self) -> u64 {
        self.resources.legacy_gas()
    }
}

// --- resources -------------------------------------------------------------------------

/// Exit code of a failed charge
#[inline(always)]
fn charge_failed(e: SystemError) -> ExitCode {
    match e {
        SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
            ExitCode::EvmError(EvmError::OutOfGas)
        }
        SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(f)) => ExitCode::FatalRuntime(f),
        SystemError::LeafDefect(e) => fatal_exit(e),
    }
}

#[inline(always)]
fn charge<R: Resources>(resources: &mut R, cost: &R) -> InstructionResult {
    match resources.charge(cost) {
        Ok(()) => Ok(()),
        Err(e) => Err(charge_failed(e)),
    }
}

/// Charges only the "native" (proving) resource
#[inline(always)]
pub(crate) fn spend_native<R: Resources>(resources: &mut R, native: u64) -> InstructionResult {
    let cost = R::from_native(Computational::from_computational(native));
    charge(resources, &cost)
}

/// Charges gas only (variable costs charged after the step charge)
#[inline(always)]
pub(crate) fn spend_gas<R: Resources>(resources: &mut R, gas: u64) -> InstructionResult {
    let Some(ergs) = R::Ergs::from_legacy_gas(gas) else {
        return Err(EvmError::OutOfGas.into());
    };
    charge(resources, &R::from_ergs(ergs))
}

/// Charges gas and native
#[inline(always)]
pub(crate) fn spend_gas_and_native<R: Resources>(
    resources: &mut R,
    gas: u64,
    native: u64,
) -> InstructionResult {
    let Some(ergs) = R::Ergs::from_legacy_gas(gas) else {
        return Err(EvmError::OutOfGas.into());
    };
    let cost = R::from_ergs_and_native(ergs, Computational::from_computational(native));
    charge(resources, &cost)
}

/// The first charge of an instruction: its gas and native cost together with the native cost
/// of the step. Same accounting as charging the step first, including out of gas, where the
/// step is still paid.
#[inline(always)]
pub(crate) fn charge_step<R: Resources>(
    resources: &mut R,
    gas: u64,
    native: u64,
) -> InstructionResult {
    let Some(ergs) = R::Ergs::from_legacy_gas(gas) else {
        spend_native(resources, STEP_NATIVE_COST)?;
        return Err(EvmError::OutOfGas.into());
    };
    let cost = R::from_ergs_and_native(
        ergs,
        Computational::from_computational(native + STEP_NATIVE_COST),
    );
    match resources.charge(&cost) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) = &e {
                // ergs are checked first and nothing is charged then
                spend_native(resources, STEP_NATIVE_COST)?;
            }
            Err(charge_failed(e))
        }
    }
}

/// Charges the growth of the heap from `current_msize` to `new_msize` (both multiples of 32)
#[inline(always)]
pub(crate) fn pay_for_memory_growth<R: Resources>(
    resources: &mut R,
    gas_paid_for_heap_growth: &mut u64,
    current_msize: usize,
    new_msize: usize,
) -> InstructionResult {
    let net_byte_increase = new_msize - current_msize;
    let new_heap_size_words = new_msize as u64 / 32;
    debug_assert_eq!(new_heap_size_words * 32, new_msize as u64);
    let end_cost = MEMORY
        .saturating_mul(new_heap_size_words)
        .saturating_add(new_heap_size_words.saturating_mul(new_heap_size_words) / 512);
    let net_cost_gas = end_cost - *gas_paid_for_heap_growth;
    let net_cost_native = HEAP_EXPANSION_BASE_NATIVE_COST.saturating_add(
        HEAP_EXPANSION_PER_BYTE_NATIVE_COST.saturating_mul(net_byte_increase as u64),
    );
    spend_gas_and_native(resources, net_cost_gas, net_cost_native)?;
    *gas_paid_for_heap_growth = end_cost;
    Ok(())
}

// --- byte order ------------------------------------------------------------------------

/// Copies 32 bytes at `src` (unaligned, big-endian) into the slot `dst` as a `U256`
///
/// # Safety
/// `src` must be readable for 32 bytes, `dst` must be a valid slot.
#[inline(always)]
pub(crate) unsafe fn read_be_word(src: *const u8, dst: *mut U256) {
    // Byte loads and stores: the source is unaligned and rv32im has no byte-swap, so
    // assembling words costs as much as it saves (measured: a 4-byte unroll was slower).
    let dst = dst.cast::<u8>();
    for i in 0..SLOT {
        dst.add(SLOT - 1 - i).write(src.add(i).read());
    }
}

/// Writes the `U256` in the slot `src` as 32 big-endian bytes at `dst` (unaligned)
///
/// # Safety
/// `dst` must be writable for 32 bytes, `src` must be an initialized slot.
#[inline(always)]
pub(crate) unsafe fn write_be_word(src: *const U256, dst: *mut u8) {
    let src = src.cast::<u8>();
    for i in 0..SLOT {
        dst.add(i).write(src.add(SLOT - 1 - i).read());
    }
}
