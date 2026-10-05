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

    /// Reserves the next slot for a push without initializing it; the caller must write all
    /// 32 bytes before the slot is read
    #[inline(always)]
    pub(crate) fn push_slot_uninit(&mut self) -> Result<*mut U256, ExitCode> {
        self.push_slot()
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

    /// Pops two values whose slots the caller may use as scratch
    #[inline(always)]
    pub fn pop_2_mut(&mut self) -> Result<(&mut U256, &mut U256), ExitCode> {
        // SAFETY: two distinct initialized slots of the stack, still valid after the pop
        self.pop_2_ptr()
            .map(|(a, b)| unsafe { (&mut *a.cast_mut(), &mut *b.cast_mut()) })
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

    /// The top three values in pop order, without changing the stack
    #[inline(always)]
    pub fn peek_3(&self) -> Option<(&U256, &U256, &U256)> {
        if self.depth < 3 {
            return None;
        }
        // SAFETY: these are initialized slots of the stack
        unsafe {
            Some((
                &*self.sp.wrapping_sub(1),
                &*self.sp.wrapping_sub(2),
                &*self.sp.wrapping_sub(3),
            ))
        }
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

    // The small pushes write the whole slot with plain word stores: on the proving target
    // eight stores are cheaper than the zeroing delegation plus the stores of the low words.

    #[inline(always)]
    pub fn push_zero(&mut self) -> InstructionResult {
        let slot = self.push_slot()?;
        // SAFETY: a free slot of the stack
        unsafe { slot.write(U256::ZERO) };
        Ok(())
    }

    #[inline(always)]
    pub fn push_u64(&mut self, value: u64) -> InstructionResult {
        let slot = self.push_slot()?;
        // SAFETY: a free slot of the stack
        unsafe { slot.write(U256::from_limbs([value, 0, 0, 0])) };
        Ok(())
    }

    #[inline(always)]
    pub fn push_b160(&mut self, address: ruint::aliases::B160) -> InstructionResult {
        let slot = self.push_slot()?;
        let limbs = address.as_limbs();
        // SAFETY: a free slot of the stack
        unsafe { slot.write(U256::from_limbs([limbs[0], limbs[1], limbs[2], 0])) };
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
        if S::CODE_IS_PADDED {
            // SAFETY: the code is followed by `CODE_PADDING_BYTES` zero bytes (in the system's
            // buffer or in a padded copy, see `EthereumLikeTypes::CODE_IS_PADDED`), and `ip`
            // never gets further than 33 bytes past its end (a `STOP` is read there at the
            // latest, see `CODE_PADDING_BYTES`)
            unsafe { self.ip.sub(1).read() }
        } else if self.ip <= self.code_end {
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

    /// [`charge_step`] on the resources of the frame, for the inlined instructions (see
    /// [`fast_charge_step`])
    #[inline(always)]
    pub fn charge_step(&mut self, gas: u64, native: u64) -> InstructionResult {
        fast_charge_step(&mut self.resources, gas, native)
    }

    /// [`spend_gas_and_native`] on the resources of the frame, for the inlined instructions
    /// (see [`fast_spend_gas_and_native`])
    #[inline(always)]
    pub fn spend_gas_and_native(&mut self, gas: u64, native: u64) -> InstructionResult {
        fast_spend_gas_and_native(&mut self.resources, gas, native)
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

// --- the gas of gas-only resources in one register --------------------------------------

/// Largest charge taken from the low word of the ergs
const LOW_WORD_CHARGE_MAX: u64 = 1 << 31;

/// No instruction; hides `x` from the optimizer on the proving target. The identity elsewhere.
///
/// A charge tests the sign of the charged low word through it, so that LLVM keeps that test
/// instead of comparing the old value with the cost (which keeps both values live), and the
/// value given back on failure is not folded into the old one. On the high word, it keeps
/// LLVM from rebuilding the ergs with a 64-bit mask, a constant the loop would keep in a
/// register.
#[inline(always)]
fn opaque(x: u32) -> u32 {
    #[cfg(target_arch = "riscv32")]
    {
        let mut x = x;
        // SAFETY: the template is a comment, so no instruction: no memory access, no flags,
        // and the tied register keeps its value
        unsafe {
            core::arch::asm!("/* {0} */", inout(reg) x, options(pure, nomem, nostack, preserves_flags))
        };
        x
    }
    #[cfg(not(target_arch = "riscv32"))]
    x
}

/// Takes `cost` ergs from resources that are their ergs alone ([`ErgsOnly`]) on the low 32-bit
/// word of the ergs: `true` if it could, else `false`, with nothing charged.
///
/// For `cost <= 2^31` the low word minus `cost` has its top bit clear only if the low word is
/// at least `cost` (otherwise the difference is in `[-2^31, 0)`), so then nothing is borrowed
/// from the high word and the result is the charge (C2 of [`ErgsOnly`]). Every other case (a
/// low word below `cost`, one that stays at 2^31 or more after the subtraction, or a cost
/// above 2^31) is left to the caller, which runs the original function on the same resources:
/// a charge that fails, or one that borrows from the high word as `Ergs::charge` does.
///
/// [`ErgsOnly`]: zk_ee::system::ErgsOnly
#[inline(always)]
fn take_from_low_word<R: Resources>(resources: &mut R, cost: u64) -> bool {
    debug_assert!(R::ERGS_ONLY.holds());
    if cost > LOW_WORD_CHARGE_MAX {
        return false;
    }
    let ergs = resources.ergs().as_u64();
    let high = opaque((ergs >> 32) as u32);
    let with_low = |low: u32| {
        R::from_ergs(R::Ergs::from_computational(
            ((high as u64) << 32) | low as u64,
        ))
    };
    // in place, and given back if short: only the new low word is live after the subtraction
    let low = opaque((ergs as u32).wrapping_sub(cost as u32));
    *resources = with_low(low);
    if (low as i32) < 0 {
        core::hint::cold_path();
        *resources = with_low(low.wrapping_add(cost as u32));
        return false;
    }
    true
}

/// [`charge_step`], first on the low word of the ergs if the resources are their ergs alone
/// ([`take_from_low_word`]). The same result and resources in every case; for other resources
/// it is [`charge_step`].
#[inline(always)]
pub(crate) fn fast_charge_step<R: Resources>(
    resources: &mut R,
    gas: u64,
    native: u64,
) -> InstructionResult {
    if const { R::ERGS_ONLY.holds() } {
        // the checked operations only decide whether to try: in every other case `charge_step`
        // computes the costs itself, overflow included
        if let (Some(ergs), Some(_)) = (
            R::Ergs::from_legacy_gas(gas),
            native.checked_add(STEP_NATIVE_COST),
        ) {
            if take_from_low_word(resources, ergs.as_u64()) {
                return Ok(());
            }
        }
    }
    charge_step(resources, gas, native)
}

/// [`fast_charge_step`] for [`spend_gas_and_native`]
#[inline(always)]
pub(crate) fn fast_spend_gas_and_native<R: Resources>(
    resources: &mut R,
    gas: u64,
    native: u64,
) -> InstructionResult {
    if const { R::ERGS_ONLY.holds() } {
        if let Some(ergs) = R::Ergs::from_legacy_gas(gas) {
            if take_from_low_word(resources, ergs.as_u64()) {
                return Ok(());
            }
        }
    }
    spend_gas_and_native(resources, gas, native)
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
    // `3 * words + words^2 / 512`, in 32-bit arithmetic while `words^2` fits (heaps below
    // 2 MiB, i.e. nearly always): the 64-bit products are emulated on the proving target
    let end_cost = if new_heap_size_words < (1 << 16) {
        let words = new_heap_size_words as u32;
        (MEMORY as u32 * words + (words * words) / 512) as u64
    } else {
        MEMORY
            .saturating_mul(new_heap_size_words)
            .saturating_add(new_heap_size_words.saturating_mul(new_heap_size_words) / 512)
    };
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
/// `src` must be readable for 32 bytes, `dst` must be a valid slot; they must not overlap.
#[inline(always)]
pub(crate) unsafe fn read_be_word(src: *const u8, dst: *mut U256) {
    // a copy (by words when the source is aligned) and one byte-reversal delegation
    U256::write_be_bytes_into_slot(src, dst);
}

/// Writes the `U256` in the slot `src` as 32 big-endian bytes at `dst` (unaligned). The
/// slot is byte-reversed in place, so it must be one the caller no longer needs (a popped
/// operand).
///
/// # Safety
/// `dst` must be writable for 32 bytes, `src` must be an initialized slot; they must not
/// overlap.
#[inline(always)]
pub(crate) unsafe fn write_be_word(src: *mut U256, dst: *mut u8) {
    U256::write_slot_as_be_bytes(src, dst);
}

#[cfg(test)]
mod tests {
    use super::{
        charge_step, fast_charge_step, fast_spend_gas_and_native, spend_gas_and_native,
        take_from_low_word, StackTop, LOW_WORD_CHARGE_MAX,
    };
    use crate::InstructionResult;
    use u256::U256;
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative, GasOnlyResources};
    use zk_ee::system::{Computational, Ergs, ErgsResource, Resource, Resources};

    const GARBAGE: U256 = U256::from_limbs([0xa5a5_a5a5_a5a5_a5a5; 4]);

    /// A stack over a buffer of garbage slots, as a reused stack buffer is
    fn garbage_stack(buffer: &mut [U256]) -> StackTop {
        buffer.fill(GARBAGE);
        StackTop {
            sp: buffer.as_mut_ptr(),
            depth: 0,
        }
    }

    /// The small pushes write the same slot as the zeroing writes they replaced
    #[test]
    fn small_pushes_match_zeroing_writes() {
        let values = [0, 1, 0xff, u32::MAX as u64, 1 << 32, u64::MAX - 1, u64::MAX];
        let address =
            ruint::aliases::B160::from_limbs([u64::MAX, 0x0123_4567_89ab_cdef, u32::MAX as u64]);
        let mut buffer = [GARBAGE; 16];
        let mut stack = garbage_stack(&mut buffer);
        for value in values {
            stack.push_u64(value).unwrap();
        }
        stack.push_zero().unwrap();
        stack.push_b160(address).unwrap();
        assert_eq!(stack.depth, values.len() + 2);

        let mut expected = [GARBAGE; 16];
        for (i, value) in values.into_iter().enumerate() {
            // SAFETY: a slot of the buffer
            unsafe { U256::write_u64_into_ptr(&mut expected[i], value) };
        }
        // SAFETY: slots of the buffer
        unsafe {
            U256::write_zero_into_ptr(&mut expected[values.len()]);
            U256::write_zero_into_ptr(&mut expected[values.len() + 1]);
        }
        let limbs = expected[values.len() + 1].as_limbs_mut();
        limbs[..3].copy_from_slice(address.as_limbs());
        assert_eq!(buffer, expected);
    }

    // --- the low word of gas-only resources ----------------------------------------------

    fn amount<R: Resources>(ergs: u64, native: u64) -> R {
        R::from_ergs_and_native(
            R::Ergs::from_computational(ergs),
            R::Native::from_computational(native),
        )
    }

    fn next(x: &mut u64) -> u64 {
        *x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *x
    }

    /// Budgets at the edges of the low word: around 2^31 and 2^32 and their multiples, the
    /// largest ergs (`i64::MAX`) and above, up to `u64::MAX`
    fn budgets() -> Vec<u64> {
        let mut values = vec![
            0,
            1,
            2,
            3,
            20,
            2300,
            (1 << 31) - 2,
            (1 << 31) - 1,
            1 << 31,
            (1 << 31) + 1,
            (1 << 31) + 2,
            (1 << 32) - 2,
            (1 << 32) - 1,
            1 << 32,
            (1 << 32) + 1,
            (1 << 32) + 2,
            (3 << 31) - 1,
            3 << 31,
            (3 << 31) + 1,
            (1 << 35) + 7,
            i64::MAX as u64 - 1,
            i64::MAX as u64,
            1 << 63,
            (1 << 63) + 1,
            u64::MAX - (1 << 31),
            u64::MAX - 1,
            u64::MAX,
        ];
        let mut x = 0x2545_f491_4f6c_dd1d;
        for _ in 0..6 {
            let r = next(&mut x);
            values.extend([r, r >> 31, r >> 33]);
        }
        values
    }

    /// Gas costs: those of the instructions, the largest the low word takes (2^31) and around
    /// it, around 2^32, and costs that don't convert at 256 ergs per gas; plus the budget and
    /// around it (in gas), for exact exhaustion
    fn gas_costs<R: Resources>(budget: u64) -> Vec<u64> {
        let gas = budget / R::Ergs::from_legacy_gas(1).unwrap().as_u64();
        let mut costs = vec![
            0,
            1,
            2,
            3,
            8,
            10,
            (1 << 23) - 1,
            1 << 23,
            (1 << 23) + 1,
            (1 << 31) - 1,
            1 << 31,
            (1 << 31) + 1,
            (1 << 32) - 1,
            1 << 32,
            (1 << 32) + 1,
            i64::MAX as u64,
            u64::MAX / 256 + 1,
            u64::MAX,
        ];
        costs.extend([gas.wrapping_sub(1), gas, gas.wrapping_add(1)]);
        costs
    }

    /// Native costs, up to the largest the step's native can be added to (see
    /// `overflowing_step_native_runs_the_original` for the others)
    const NATIVE: [u64; 3] = [0, 20, u64::MAX - 20];

    /// Both charges of a step on the same resources, the fast one and the original one: the
    /// same result and resources, or the same panic (with overflow checks)
    fn check_step<R: Resources + std::panic::RefUnwindSafe>(initial: &R, gas: u64, native: u64) {
        let run = |f: fn(&mut R, u64, u64) -> InstructionResult| {
            std::panic::catch_unwind(|| {
                let mut r = initial.clone();
                let result = f(&mut r, gas, native);
                (result, r)
            })
        };
        for (fast, original) in [
            (
                fast_charge_step::<R> as fn(&mut R, u64, u64) -> InstructionResult,
                charge_step::<R> as fn(&mut R, u64, u64) -> InstructionResult,
            ),
            (fast_spend_gas_and_native::<R>, spend_gas_and_native::<R>),
        ] {
            match (run(fast), run(original)) {
                (Ok(fast), Ok(original)) => {
                    assert_eq!(fast, original, "{initial:?} {gas} {native}")
                }
                (Err(_), Err(_)) => {}
                _ => panic!("only one side panicked: {initial:?} {gas} {native}"),
            }
        }
    }

    /// Every charge of an inlined instruction, from every budget, against the original
    /// function, for gas-only resources (at 1 and 256 ergs per gas) and for the resources of
    /// the ZKsync OS STF (where the fast functions are the original ones)
    fn check_single_charges<R: Resources + std::panic::RefUnwindSafe>(natives: &[u64]) {
        for ergs in budgets() {
            for &budget_native in natives {
                let initial = amount::<R>(ergs, budget_native);
                for gas in gas_costs::<R>(ergs) {
                    for native in NATIVE {
                        check_step(&initial, gas, native);
                    }
                }
            }
        }
    }

    #[test]
    fn single_charges_match_the_original() {
        assert!(GasOnlyResources::ERGS_ONLY.holds());
        assert!(BaseResources::<(), 256>::ERGS_ONLY.holds());
        assert!(!BaseResources::<DecreasingNative>::ERGS_ONLY.holds());
        check_single_charges::<GasOnlyResources>(&[0]);
        check_single_charges::<BaseResources<(), 256>>(&[0]);
        check_single_charges::<BaseResources<DecreasingNative>>(&[0, 20, u64::MAX]);
    }

    /// The low word takes exactly the charges of at most 2^31 it can pay without a borrow: the
    /// ones whose low word stays below 2^31 after the subtraction. It takes them as the
    /// original charge does, and leaves the others alone.
    #[test]
    fn low_word_takes_exactly_the_borrow_free_charges() {
        type R = GasOnlyResources;
        let mut costs = vec![
            0,
            1,
            2,
            3,
            20,
            (1 << 31) - 1,
            1 << 31,
            (1 << 31) + 1,
            u64::MAX,
        ];
        let mut x = 7;
        for _ in 0..16 {
            costs.push(next(&mut x) >> 33);
        }
        let mut taken = 0;
        for ergs in budgets() {
            for &cost in &costs {
                let mut r = R::from_ergs(Ergs(ergs));
                let low = (ergs as u32).wrapping_sub(cost as u32);
                let expected = cost <= LOW_WORD_CHARGE_MAX && (low as i32) >= 0;
                assert_eq!(take_from_low_word(&mut r, cost), expected, "{ergs} {cost}");
                if expected {
                    taken += 1;
                    assert!(ergs >= cost && (ergs as u32) >= cost as u32);
                    let mut original = R::from_ergs(Ergs(ergs));
                    assert_eq!(original.charge(&R::from_ergs(Ergs(cost))), Ok(()));
                    assert_eq!(r, original, "{ergs} {cost}");
                } else {
                    assert_eq!(r, R::from_ergs(Ergs(ergs)), "{ergs} {cost}");
                }
            }
        }
        assert!(taken > 0);
    }

    /// The edges themselves: a budget below 2^31 is paid down in the low word until a charge
    /// it can't pay, which then fails as the original one does (out of gas, ergs empty); one of
    /// 2^31 or more leaves the low word to the original charge whenever it would borrow
    #[test]
    fn budgets_at_the_edges() {
        type R = GasOnlyResources;
        let ok: InstructionResult = Ok(());
        for (ergs, gas, left) in [
            ((1 << 31) - 1, (1 << 31) - 1, Some(0)),
            ((1 << 31) - 1, 1 << 31, None),
            (1 << 31, 1 << 31, Some(0)),
            (1 << 31, 1, Some((1 << 31) - 1)),
            ((1 << 31) + 1, 2, Some((1 << 31) - 1)),
            ((1 << 32) - 1, 1, Some((1 << 32) - 2)),
            (1 << 32, 1, Some((1 << 32) - 1)),
            ((1 << 32) + 1, 2, Some((1 << 32) - 1)),
            ((1 << 32) + 1, (1 << 31) + 1, Some(1 << 31)),
            (i64::MAX as u64, 3, Some(i64::MAX as u64 - 3)),
            (u64::MAX, 3, Some(u64::MAX - 3)),
            (2, 3, None),
            (0, 0, Some(0)),
            (0, 1, None),
        ] {
            let mut fast = R::from_ergs(Ergs(ergs));
            let mut original = fast.clone();
            let result = fast_charge_step(&mut fast, gas, 0);
            assert_eq!(result, charge_step(&mut original, gas, 0), "{ergs} {gas}");
            assert_eq!(fast, original);
            match left {
                Some(left) => {
                    assert_eq!(result, ok);
                    assert_eq!(fast.ergs().0, left);
                }
                None => {
                    assert!(result.is_err());
                    assert_eq!(fast.ergs().0, 0);
                }
            }
        }
    }

    /// A step native that overflows when the step's native is added runs the original
    /// function: the same panic with overflow checks, the same result without
    #[test]
    fn overflowing_step_native_runs_the_original() {
        for ergs in [0, 1000, (1 << 31) + 1, u64::MAX] {
            for gas in [0, 3, u64::MAX] {
                for native in [u64::MAX - 19, u64::MAX] {
                    check_step(&GasOnlyResources::from_ergs(Ergs(ergs)), gas, native);
                    check_step(
                        &amount::<BaseResources<DecreasingNative>>(ergs, 100),
                        gas,
                        native,
                    );
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Op {
        Step(u64, u64),
        GasAndNative(u64, u64),
        /// A charge of all that is left, or of one more
        Exhaust(u64),
        /// CALL or CREATE: everything goes to the callee, which gives back what it didn't spend
        CallAndReturn(u64),
        /// Out of gas in an outlined instruction
        ExhaustErgs,
    }

    /// Sequences of charges from the budgets at the edges, compared with the original
    /// functions after every step
    fn check_sequences<R: Resources>(steps: usize) {
        let ops = [
            Op::Step(3, 30),
            Op::Step(2, 0),
            Op::Step(1 << 23, 0),
            Op::Step((1 << 31) - 1, 0),
            Op::GasAndNative(1, 20),
            Op::GasAndNative((1 << 31) + 1, 0),
            Op::Exhaust(0),
            Op::Exhaust(1),
            Op::CallAndReturn(700),
            Op::CallAndReturn(1 << 31),
            Op::ExhaustErgs,
        ];
        let mut x = 0x0123_4567_89ab_cdef;
        for ergs in budgets() {
            let mut fast = amount::<R>(ergs, 1 << 40);
            let mut original = fast.clone();
            for _ in 0..steps {
                let op = ops[(next(&mut x) >> 33) as usize % ops.len()];
                let (got, expected) = match op {
                    Op::Step(gas, native) => (
                        fast_charge_step(&mut fast, gas, native),
                        charge_step(&mut original, gas, native),
                    ),
                    Op::GasAndNative(gas, native) => (
                        fast_spend_gas_and_native(&mut fast, gas, native),
                        spend_gas_and_native(&mut original, gas, native),
                    ),
                    Op::Exhaust(more) => {
                        let gas = original.legacy_gas() + more;
                        (
                            fast_spend_gas_and_native(&mut fast, gas, 0),
                            spend_gas_and_native(&mut original, gas, 0),
                        )
                    }
                    Op::CallAndReturn(spent) => {
                        let call = |r: &mut R| {
                            let mut taken = r.take();
                            let _ = charge_step(&mut taken, spent, spent);
                            r.reclaim(taken);
                        };
                        call(&mut fast);
                        call(&mut original);
                        (Ok(()), Ok(()))
                    }
                    Op::ExhaustErgs => {
                        fast.exhaust_ergs();
                        original.exhaust_ergs();
                        (Ok(()), Ok(()))
                    }
                };
                assert_eq!(got, expected, "{ergs} {op:?}");
                assert_eq!(fast, original, "{ergs} {op:?}");
            }
        }
    }

    #[test]
    fn sequences_match_the_original() {
        check_sequences::<GasOnlyResources>(300);
        check_sequences::<BaseResources<(), 256>>(300);
        check_sequences::<BaseResources<DecreasingNative>>(100);
    }
}
