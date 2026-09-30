//! The register-resident part of the frame state, and the resource charging on it.

use core::marker::PhantomData;

use u256::U256;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::system::evm::EvmError;
use zk_ee::system::{Computational, ErgsResource, EthereumLikeTypes, Resources};

use super::{fatal_exit, HotFrameParts};
use crate::gas_constants::MEMORY;
use crate::native_resource_constants::{
    HEAP_EXPANSION_BASE_NATIVE_COST, HEAP_EXPANSION_PER_BYTE_NATIVE_COST, STEP_NATIVE_COST,
};
use crate::{CodeWithSentinel, ExitCode, InstructionResult, STACK_SIZE};

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
    /// Next byte to execute (or read as an immediate). At a proving fetch it is at most
    /// `code_end` for unpadded code, and at most 32 bytes past it for padded code.
    pub ip: *const u8,
    /// One past the last byte of the code
    pub code_end: *const u8,
    /// The stack
    pub stack: StackTop,
    /// Resources of the frame. They stay in the holder, where a run leaves only their high
    /// parts (see [`LowResources`]); outlined instructions see them whole
    /// ([`InHolder::whole`]). The inlined ones charge through [`Self::charge_step`] and the
    /// like, which take the low parts first.
    pub resources: InHolder<'h, HotFrameParts<S>>,
    /// The low parts of the resources (private: a charge that bypasses them breaks the split)
    low: LowResources,
}

impl<'h, S: EthereumLikeTypes> Hot<'h, S> {
    /// `outlined` copies the state bitwise; nothing in it may own a resource
    const NO_DROP_GLUE: () = assert!(!core::mem::needs_drop::<Self>());

    /// Takes the hot state out of the holder for a run over `code`. Both code pointers are
    /// derived from the pointer of `code`, whose span includes the zero byte after the code.
    #[inline(always)]
    pub fn new(
        holder: &'h mut HotFrameParts<S>,
        code: &CodeWithSentinel<'_, S::Allocator>,
    ) -> Self {
        let depth = holder.stack.depth();
        Self {
            ip: code.as_ptr().wrapping_add(holder.instruction_pointer),
            code_end: code.as_ptr().wrapping_add(code.len()),
            stack: StackTop {
                sp: holder.stack.base_ptr().wrapping_add(depth),
                depth,
            },
            low: LowResources::split(&mut holder.resources),
            resources: InHolder::new(holder),
        }
    }

    /// Puts the state back into the holder; `code_start` is that of `new`
    #[inline(always)]
    pub fn finish(self, code_start: *const u8) {
        let Self {
            ip,
            stack,
            resources,
            low,
            ..
        } = self;
        debug_assert!(low.is_split(), "finished while suspended");
        let holder = resources.into_holder();
        holder.instruction_pointer = ip.addr().wrapping_sub(code_start.addr());
        // SAFETY: `depth` counts the initialized prefix of the buffer
        unsafe { holder.stack.set_depth(stack.depth) };
        low.join(&mut holder.resources);
    }

    /// Runs an outlined instruction on a copy of the state and takes the copy back.
    ///
    /// The copy is what the callee's `&mut Self` points to, so the address of the state
    /// itself never escapes: passing it to a real call would keep it in memory for the whole
    /// loop. The callee sees the resources whole, in the holder ([`InHolder::whole`]).
    #[inline(always)]
    pub fn outlined<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let () = Self::NO_DROP_GLUE;
        self.low.suspend(self.resources.get_mut());
        self.resources.set_outlined(true);
        // SAFETY: a bitwise move out and back in, with no drop in between (no drop glue), and
        // the original (with its copy of the resources handle) is not used while the copy is
        // live
        let result = unsafe {
            let mut copy = core::ptr::read(self);
            let result = f(&mut copy);
            core::ptr::write(self, copy);
            result
        };
        self.resources.set_outlined(false);
        self.low.resume(self.resources.get_mut());
        result
    }

    /// The opcode at `ip`, advancing past it. At the end of the code this reads the zero byte
    /// that frame setup puts after it, i.e. a `STOP` (the pointer still advances)
    #[cfg(target_arch = "riscv32")]
    #[inline(always)]
    pub fn fetch_and_advance(&mut self) -> u8 {
        self.ip = self.ip.wrapping_add(1);
        // SAFETY: `ip` was derived from the pointer of a `CodeWithSentinel` (`Self::new`), from
        // which the code and the zero byte after it are readable.
        // Unpadded code: `ip - 1` is in the code or at its end, the zero byte. Sequential
        // fetches stop at the end, a truncated PUSH immediate stops `ip` there
        // (`stop_at_code_end`), a jump lands at most there (`ColdFrameParts::set_code`), and
        // `Interpreter::run` clamps a pointer set from outside to it.
        // Padded code is followed by `CODE_PADDING_BYTES` zero bytes
        // (`EthereumLikeTypes::CODE_IS_PADDED`), and `ip - 1` is at most 32 bytes past its end
        // (a `STOP` is read there at the latest).
        unsafe { self.ip.sub(1).read() }
    }

    /// After a PUSH immediate that runs past the end of the code: the proving fetch has no end
    /// check (see [`Self::fetch_and_advance`]), so `ip` stops at the zero byte after the code,
    /// i.e. a `STOP`. The host keeps the advanced pointer, which its tracers report.
    #[inline(always)]
    pub fn stop_at_code_end(&mut self) {
        if cfg!(target_arch = "riscv32") {
            self.ip = self.code_end;
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

    /// Remaining EVM gas (also in an outlined instruction)
    #[inline(always)]
    pub fn gas_left(&self) -> u64 {
        self.low.legacy_gas(self.resources.get())
    }

    /// The whole resources (for the host tracer, also in an outlined instruction)
    #[inline(always)]
    pub fn joined_resources(&self) -> S::Resources {
        self.low.joined(self.resources.get())
    }

    /// Runs `f` on the whole resources, for an inlined instruction (see
    /// [`LowResources::suspended`])
    #[inline(always)]
    pub fn suspended<T>(&mut self, f: impl FnOnce(&mut S::Resources) -> T) -> T {
        self.low.suspended(self.resources.get_mut(), f)
    }

    /// [`charge_step`] on the resources of the frame
    #[inline(always)]
    pub fn charge_step(&mut self, gas: u64, native: u64) -> InstructionResult {
        self.low.charge_step(self.resources.get_mut(), gas, native)
    }

    /// [`spend_native`] on the resources of the frame
    #[inline(always)]
    pub fn spend_native(&mut self, native: u64) -> InstructionResult {
        self.low.spend_native(self.resources.get_mut(), native)
    }

    /// [`spend_gas_and_native`] on the resources of the frame
    #[inline(always)]
    pub fn spend_gas_and_native(&mut self, gas: u64, native: u64) -> InstructionResult {
        self.low
            .spend_gas_and_native(self.resources.get_mut(), gas, native)
    }
}

/// The resources of the frame, in the holder that [`Hot`] borrows for `'h`.
///
/// A pointer to the holder, not to its resources, so that [`Hot::finish`] writes the rest
/// back through the same pointer (and a run keeps one register for both). [`Hot::outlined`]
/// copies it bitwise with the rest of the state; nothing else makes a second one.
///
/// While a run has the resources split, the holder keeps only their high parts, so outside
/// this module they are reached only through [`Self::whole`], in an outlined instruction.
pub(crate) struct InHolder<'h, H> {
    holder: *mut H,
    _borrow: PhantomData<&'h mut H>,
    /// In an outlined instruction (set by [`Hot::outlined`])
    #[cfg(debug_assertions)]
    outlined: bool,
}

/// What holds the resources of a frame between runs
pub(crate) trait ResourcesHolder {
    type Resources;
    fn resources(&self) -> &Self::Resources;
    fn resources_mut(&mut self) -> &mut Self::Resources;
}

impl<S: EthereumLikeTypes> ResourcesHolder for HotFrameParts<S> {
    type Resources = S::Resources;

    #[inline(always)]
    fn resources(&self) -> &S::Resources {
        &self.resources
    }

    #[inline(always)]
    fn resources_mut(&mut self) -> &mut S::Resources {
        &mut self.resources
    }
}

// SAFETY (of the three unsafe blocks below):
// - valid: `holder` comes from the `&'h mut H` given to `new`, and `_borrow` keeps the
//   handle from outliving `'h`, so it points to a live, aligned `H`;
// - exclusive: that borrow is moved into the handle, so for `'h` the holder is reached only
//   through `holder`. The only other copies of it are the bitwise copies of the whole state
//   in `Hot::outlined`, which uses the original only before the copy is made and after it
//   is written back over the original, so one handle is in use at a time;
// - no overlap: a reference from `get` / `get_mut` borrows the handle, so the borrow
//   checker keeps it from outliving the handle's next mutable use, and `into_holder`
//   consumes the handle. No such reference outlives a call that ends the borrow: the
//   charges, the heap growth and the outlined instructions take it for the call only, and
//   the tracer views copy the resources (`Hot::joined_resources`).
impl<'h, H: ResourcesHolder> InHolder<'h, H> {
    #[inline(always)]
    pub(crate) fn new(holder: &'h mut H) -> Self {
        Self {
            holder,
            _borrow: PhantomData,
            #[cfg(debug_assertions)]
            outlined: false,
        }
    }

    /// The borrow of `new` back, at the end of the run
    #[inline(always)]
    pub(crate) fn into_holder(self) -> &'h mut H {
        // SAFETY: see above
        unsafe { &mut *self.holder }
    }

    /// The resources in the holder: only their high parts while split
    #[inline(always)]
    fn get(&self) -> &H::Resources {
        // SAFETY: see above
        unsafe { (*self.holder).resources() }
    }

    /// See [`Self::get`]
    #[inline(always)]
    fn get_mut(&mut self) -> &mut H::Resources {
        // SAFETY: see above
        unsafe { (*self.holder).resources_mut() }
    }

    /// The whole resources, in an outlined instruction (see [`Hot::outlined`])
    #[inline(always)]
    pub(crate) fn whole(&mut self) -> &mut H::Resources {
        #[cfg(debug_assertions)]
        debug_assert!(self.outlined, "whole resources while split");
        self.get_mut()
    }

    /// Marks the run of an outlined instruction, for the check in [`Self::whole`]
    #[inline(always)]
    fn set_outlined(&mut self, outlined: bool) {
        #[cfg(debug_assertions)]
        {
            self.outlined = outlined;
        }
        #[cfg(not(debug_assertions))]
        let _ = outlined;
    }
}

// --- resources in registers ----------------------------------------------------------

/// Largest charge the low parts take (and give) themselves
const LOW_CHARGE_MAX: u64 = 1 << 31;

/// No instruction; hides `x` from the optimizer on the proving target. The identity elsewhere.
///
/// A charge tests the sign of the charged value through it, so that LLVM keeps that test
/// instead of comparing the old value with the cost (which keeps both values live), and a
/// restored value is not folded back into the old one.
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

/// `x mod 2^31`, by shifts: a mask would be a constant the loop keeps in a register
#[inline(always)]
fn clear_top_bit(x: u32) -> u32 {
    opaque(x << 1) >> 1
}

/// `(high, low)` with `high + low == amount`, `high` a multiple of 2^31 and `low < 2^31`
#[inline(always)]
fn split_amount(amount: u64) -> (u64, u32) {
    let low = clear_top_bit(amount as u32);
    (amount ^ low as u64, low)
}

/// The low 31 bits of the ergs and the native resource of a frame, in registers for a run,
/// while the resources in the holder keep the rest (the high parts).
///
/// Only the amounts that [`Resources::SPLIT_FORM`] allows are split; the others stay whole
/// in the holder, and for [`SplitForm::NONE`] this is today's code. A charge that both parts
/// can pay is taken from the low parts, borrowing 2^31 from the high part when a low part
/// runs out. Any other charge (a short or a large one) is undone and then run by the
/// original function on the joined resources, so it is today's charge on today's amounts.
///
/// While an outlined instruction, the heap growth or an original charge runs, the resources
/// are joined in the holder and the low parts are zero ("suspended"), so reading through
/// both still gives the whole value.
///
/// [`SplitForm::NONE`]: zk_ee::system::SplitForm::NONE
#[derive(Clone, Copy)]
pub struct LowResources {
    ergs: u32,
    native: u32,
    /// Split, not suspended (checked by the transitions)
    #[cfg(debug_assertions)]
    split: bool,
}

impl LowResources {
    /// Takes the low parts out of `resources`, which keep the high parts
    #[inline(always)]
    pub fn split<R: Resources>(resources: &mut R) -> Self {
        let form = R::SPLIT_FORM;
        if !form.ergs() {
            return Self {
                ergs: 0,
                native: 0,
                #[cfg(debug_assertions)]
                split: true,
            };
        }
        debug_assert_eq!(
            R::from_ergs_and_native(resources.ergs(), resources.native()),
            *resources
        );
        let (ergs_high, ergs) = split_amount(resources.ergs().as_u64());
        let (native_high, native) = if form.native() {
            let (high, low) = split_amount(resources.native().as_u64());
            (R::Native::from_computational(high), low)
        } else {
            (resources.native(), 0)
        };
        *resources = R::from_ergs_and_native(R::Ergs::from_computational(ergs_high), native_high);
        Self {
            ergs,
            native,
            #[cfg(debug_assertions)]
            split: true,
        }
    }

    /// Puts the low parts back: `high` holds the whole resources again
    #[inline(always)]
    pub fn join<R: Resources>(self, high: &mut R) {
        let form = R::SPLIT_FORM;
        if !form.ergs() {
            return;
        }
        self.check_high_parts(high);
        let ergs = R::Ergs::from_computational(high.ergs().as_u64() | self.ergs as u64);
        let native = if form.native() {
            R::Native::from_computational(high.native().as_u64() | self.native as u64)
        } else {
            high.native()
        };
        *high = R::from_ergs_and_native(ergs, native);
    }

    /// The whole resources (in both states)
    #[inline(always)]
    pub fn joined<R: Resources>(&self, high: &R) -> R {
        let mut value = high.clone();
        self.join(&mut value);
        value
    }

    /// Remaining EVM gas of the whole resources (in both states)
    #[inline(always)]
    pub fn legacy_gas<R: Resources>(&self, high: &R) -> u64 {
        if R::SPLIT_FORM.ergs() {
            self.check_high_parts(high);
            R::Ergs::from_computational(high.ergs().as_u64() | self.ergs as u64).as_legacy_gas()
        } else {
            high.legacy_gas()
        }
    }

    /// Joins the resources in `high`, for code that charges them there, until `resume`
    #[inline(always)]
    pub fn suspend<R: Resources>(&mut self, high: &mut R) {
        debug_assert!(self.is_split(), "suspended twice");
        self.join(high);
        self.ergs = 0;
        self.native = 0;
        #[cfg(debug_assertions)]
        {
            self.split = false;
        }
    }

    /// Splits whatever the code since `suspend` left in `high`
    #[inline(always)]
    pub fn resume<R: Resources>(&mut self, high: &mut R) {
        debug_assert!(!self.is_split(), "resumed without a suspend");
        *self = Self::split(high);
    }

    /// Runs `f` on the joined resources and splits them again, also when `f` fails
    #[inline(always)]
    pub fn suspended<R: Resources, T>(&mut self, high: &mut R, f: impl FnOnce(&mut R) -> T) -> T {
        self.suspend(high);
        let result = f(high);
        self.resume(high);
        result
    }

    #[inline(always)]
    fn is_split(&self) -> bool {
        #[cfg(debug_assertions)]
        {
            self.split
        }
        #[cfg(not(debug_assertions))]
        {
            true
        }
    }

    /// While split, the split amounts in `high` must be high parts, i.e. multiples of 2^31. A
    /// charge on `high` that bypasses the low parts breaks this, and a later borrow of 2^31
    /// from it would then underflow.
    #[inline(always)]
    fn check_high_parts<R: Resources>(&self, high: &R) {
        let form = R::SPLIT_FORM;
        let is_high_part = |amount: u64| amount.is_multiple_of(LOW_CHARGE_MAX);
        debug_assert!(
            !self.is_split()
                || !form.ergs()
                || (is_high_part(high.ergs().as_u64())
                    && (!form.native() || is_high_part(high.native().as_u64()))),
            "charged on the high parts"
        );
    }

    /// Takes `cost <= 2^31` ergs from the low part; `false`, with nothing charged, iff the
    /// ergs are short
    #[inline(always)]
    fn take_ergs<R: Resources>(&mut self, high: &mut R, cost: u32) -> bool {
        // exact in `i32`: the low part is below 2^31 and the cost at most 2^31
        self.ergs = opaque(self.ergs.wrapping_sub(cost));
        if (self.ergs as i32) < 0 {
            core::hint::cold_path();
            let amount = high.ergs().as_u64();
            if amount == 0 {
                self.ergs = self.ergs.wrapping_add(cost);
                return false;
            }
            // the high part is a nonzero multiple of 2^31, so at least 2^31
            *high = R::from_ergs_and_native(
                R::Ergs::from_computational(amount - LOW_CHARGE_MAX),
                high.native(),
            );
            // the negative low part plus 2^31
            self.ergs = clear_top_bit(self.ergs);
        }
        true
    }

    /// [`Self::take_ergs`] for the native resource; `true` at once if it is not split
    #[inline(always)]
    fn take_native<R: Resources>(&mut self, high: &mut R, cost: u32) -> bool {
        if !R::SPLIT_FORM.native() {
            return true;
        }
        self.native = opaque(self.native.wrapping_sub(cost));
        if (self.native as i32) < 0 {
            core::hint::cold_path();
            let amount = high.native().as_u64();
            if amount == 0 {
                self.native = self.native.wrapping_add(cost);
                return false;
            }
            *high = R::from_ergs_and_native(
                high.ergs(),
                R::Native::from_computational(amount - LOW_CHARGE_MAX),
            );
            self.native = clear_top_bit(self.native);
        }
        true
    }

    /// Charges `ergs` and `native` if both parts can pay; else `false`, with nothing charged
    #[inline(always)]
    fn try_fast<R: Resources>(&mut self, high: &mut R, ergs: u64, native: u64) -> bool {
        debug_assert!(self.is_split(), "charged while suspended");
        self.check_high_parts(high);
        if !R::SPLIT_FORM.ergs() || ergs > LOW_CHARGE_MAX || native > LOW_CHARGE_MAX {
            return false;
        }
        if !self.take_ergs(high, ergs as u32) {
            return false;
        }
        if self.take_native(high, native as u32) {
            return true;
        }
        core::hint::cold_path();
        // the native resource is short: give the ergs back (no overflow: they were there)
        self.join(high);
        *high = R::from_ergs_and_native(
            R::Ergs::from_computational(high.ergs().as_u64() + ergs),
            high.native(),
        );
        *self = Self::split(high);
        false
    }

    /// [`charge_step`] on the split resources
    #[inline(always)]
    pub fn charge_step<R: Resources>(
        &mut self,
        high: &mut R,
        gas: u64,
        native: u64,
    ) -> InstructionResult {
        // the checked conversions only decide whether the low parts may pay: in every other
        // case `charge_step` computes the costs itself, overflow included
        if let (Some(ergs), Some(native_cost)) = (
            R::Ergs::from_legacy_gas(gas),
            native.checked_add(STEP_NATIVE_COST),
        ) {
            if self.try_fast(high, ergs.as_u64(), native_cost) {
                return Ok(());
            }
        }
        self.suspend(high);
        let result = charge_step(high, gas, native);
        self.resume(high);
        result
    }

    /// [`spend_native`] on the split resources
    #[inline(always)]
    pub fn spend_native<R: Resources>(&mut self, high: &mut R, native: u64) -> InstructionResult {
        if self.try_fast(high, 0, native) {
            return Ok(());
        }
        self.suspend(high);
        let result = spend_native(high, native);
        self.resume(high);
        result
    }

    /// [`spend_gas_and_native`] on the split resources
    #[inline(always)]
    pub fn spend_gas_and_native<R: Resources>(
        &mut self,
        high: &mut R,
        gas: u64,
        native: u64,
    ) -> InstructionResult {
        if let Some(ergs) = R::Ergs::from_legacy_gas(gas) {
            if self.try_fast(high, ergs.as_u64(), native) {
                return Ok(());
            }
        }
        self.suspend(high);
        let result = spend_gas_and_native(high, gas, native);
        self.resume(high);
        result
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
        charge_step, spend_gas, spend_gas_and_native, spend_native, InHolder, LowResources,
        ResourcesHolder, StackTop,
    };
    use crate::InstructionResult;
    use u256::U256;
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative, GasOnlyResources};
    use zk_ee::system::evm::EvmError;
    use zk_ee::system::{errors::system::SystemError, Computational, Ergs, Resource, Resources};

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

    fn amount<R: Resources>(ergs: u64, native: u64) -> R {
        R::from_ergs_and_native(
            R::Ergs::from_computational(ergs),
            R::Native::from_computational(native),
        )
    }

    /// Budgets at the edges of the split: 2^31 and its multiples (a low part of 0 borrows on
    /// the first charge), ergs up to and above `MAX_ERGS`, native up to `FORMAL_INFINITE`
    fn budgets() -> Vec<u64> {
        let mut values = vec![
            0,
            1,
            2,
            20,
            768,
            (1 << 31) - 1,
            1 << 31,
            (1 << 31) + 1,
            (1 << 32) - 1,
            1 << 32,
            (1 << 32) + 20,
            3 << 31,
            (3 << 31) + 767,
            1 << 35,
            Ergs::<1>::MAX_ERGS,
            Ergs::<1>::MAX_ERGS + 1,
            u64::MAX - (1 << 31),
            u64::MAX - 20,
            u64::MAX - 1,
            u64::MAX,
        ];
        if cfg!(miri) {
            values.truncate(12);
        }
        values
    }

    /// Gas costs: the constants of the instructions, exactly 2^31 ergs (`1 << 23` gas at 256)
    /// and above, and gas that does not convert
    const GAS: [u64; 13] = [
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
        u64::MAX / 256 + 1,
        u64::MAX,
    ];

    /// Native costs, including the ones that make the step charge exactly 2^31 and the ones
    /// whose sum with the step native overflows
    const NATIVE: [u64; 14] = [
        0,
        1,
        19,
        20,
        40,
        288,
        (1 << 31) - 21,
        (1 << 31) - 20,
        (1 << 31) - 19,
        1 << 31,
        (1 << 31) + 1,
        u64::MAX - 20,
        u64::MAX - 19,
        u64::MAX,
    ];

    /// The resources in both forms: split into `low` and `high`, and `whole`, charged by
    /// the original functions
    struct Pair<R> {
        low: LowResources,
        high: R,
        whole: R,
    }

    #[derive(Clone, Copy, Debug)]
    enum Op {
        Step(u64, u64),
        Native(u64),
        GasAndNative(u64, u64),
        /// An outlined instruction: its step charge, then the reads of a suspended state
        /// (the SSTORE stipend check and a tracer view)
        Outlined(u64, u64),
        /// CREATE: a native charge with infinite ergs in an outlined instruction
        InfiniteErgs(u64),
        /// CALL: the resources are drained in an outlined instruction, the run ends, and the
        /// next run starts after the callee gave back its unspent part
        CallAndReturn(u64),
        /// Heap growth that pays and then fails (the capacity check after the charge)
        GrowthFailsAfterCharge(u64, u64),
        /// The run ends and the next one starts
        Rerun,
    }

    impl<R: Resources> Pair<R> {
        fn new(whole: R) -> Self {
            let mut high = whole.clone();
            let low = LowResources::split(&mut high);
            Self { low, high, whole }
        }

        fn check(&self, what: &dyn core::fmt::Debug) {
            assert!(self.low.is_split(), "{what:?}");
            assert_eq!(self.low.joined(&self.high), self.whole, "{what:?}");
            assert_eq!(
                self.low.legacy_gas(&self.high),
                self.whole.legacy_gas(),
                "{what:?}"
            );
        }

        fn rerun(&mut self) {
            self.low.join(&mut self.high);
            assert_eq!(self.high, self.whole);
            self.low = LowResources::split(&mut self.high);
        }

        fn apply(&mut self, op: Op) {
            let whole = &mut self.whole;
            let (expected, got) = match op {
                Op::Step(gas, native) => (
                    charge_step(whole, gas, native),
                    self.low.charge_step(&mut self.high, gas, native),
                ),
                Op::Native(native) => (
                    spend_native(whole, native),
                    self.low.spend_native(&mut self.high, native),
                ),
                Op::GasAndNative(gas, native) => (
                    spend_gas_and_native(whole, gas, native),
                    self.low.spend_gas_and_native(&mut self.high, gas, native),
                ),
                Op::Outlined(gas, native) => {
                    let expected = charge_step(whole, gas, native);
                    let (gas_left, view) = (whole.legacy_gas(), whole.clone());
                    self.low.suspend(&mut self.high);
                    let got = charge_step(&mut self.high, gas, native);
                    assert_eq!(self.low.legacy_gas(&self.high), gas_left);
                    assert_eq!(self.low.joined(&self.high), view);
                    self.low.resume(&mut self.high);
                    (expected, got)
                }
                Op::InfiniteErgs(native) => (
                    whole.with_infinite_ergs(|r| spend_native(r, native)),
                    self.low.suspended(&mut self.high, |r| {
                        r.with_infinite_ergs(|r| spend_native(r, native))
                    }),
                ),
                Op::CallAndReturn(spent) => {
                    let give_back = |taken: R| {
                        let mut taken = taken;
                        let _ = charge_step(&mut taken, spent, spent);
                        taken
                    };
                    let taken = whole.take();
                    whole.reclaim(give_back(taken));
                    let taken = self.low.suspended(&mut self.high, |r| r.take());
                    // the run ends with the call, the frame gets the rest back between runs
                    self.low.join(&mut self.high);
                    self.high.reclaim(give_back(taken));
                    self.low = LowResources::split(&mut self.high);
                    (Ok(()), Ok(()))
                }
                Op::GrowthFailsAfterCharge(gas, native) => {
                    let fail = |r: &mut R| -> InstructionResult {
                        spend_gas_and_native(r, gas, native)?;
                        Err(EvmError::MemoryLimitOOG.into())
                    };
                    (fail(whole), self.low.suspended(&mut self.high, fail))
                }
                Op::Rerun => {
                    self.rerun();
                    (Ok(()), Ok(()))
                }
            };
            assert_eq!(got, expected, "{op:?}");
            self.check(&op);
        }
    }

    /// Every charge of the register form against the original function, from every budget
    /// pair, for the ergs-and-native form, the ergs-only form and the unsplit form
    fn check_single_charges<R: Resources>() {
        let budgets = budgets();
        for &ergs in &budgets {
            for &native in &budgets {
                let initial = amount::<R>(ergs, native);
                for gas in GAS {
                    for cost in NATIVE {
                        for op in [
                            Op::Step(gas, cost),
                            Op::Native(cost),
                            Op::GasAndNative(gas, cost),
                        ] {
                            if cost > u64::MAX - 20 && matches!(op, Op::Step(..)) {
                                continue; // overflows, see `overflowing_costs_run_the_original`
                            }
                            let mut pair = Pair::new(initial.clone());
                            pair.check(&initial);
                            pair.apply(op);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn single_charges_match_the_original() {
        assert!(!NativeFirst::SPLIT_FORM.ergs() && !NativeFirst::SPLIT_FORM.native());
        check_single_charges::<BaseResources<DecreasingNative>>();
        check_single_charges::<GasOnlyResources>();
        if !cfg!(miri) {
            check_single_charges::<NativeFirst>();
        }
    }

    /// The costs the register form can't represent (gas that does not convert, a step
    /// native that overflows, more than 2^31) run the original function: the same result
    /// and resources, or the same panic with overflow checks
    fn check_unrepresentable<R: Resources + std::panic::RefUnwindSafe>(
        ergs: u64,
        native: u64,
        op: Op,
    ) {
        let original = std::panic::catch_unwind(|| {
            let mut pair = Pair::new(amount::<R>(ergs, native));
            let result = match op {
                Op::Step(gas, native) => charge_step(&mut pair.whole, gas, native),
                Op::GasAndNative(gas, native) => spend_gas_and_native(&mut pair.whole, gas, native),
                _ => unreachable!(),
            };
            (result, pair.whole)
        });
        let split = std::panic::catch_unwind(|| {
            let mut pair = Pair::new(amount::<R>(ergs, native));
            let result = match op {
                Op::Step(gas, native) => pair.low.charge_step(&mut pair.high, gas, native),
                Op::GasAndNative(gas, native) => {
                    pair.low.spend_gas_and_native(&mut pair.high, gas, native)
                }
                _ => unreachable!(),
            };
            (result, pair.low.joined(&pair.high))
        });
        match (original, split) {
            (Ok(original), Ok(split)) => assert_eq!(original, split, "{op:?}"),
            (Err(_), Err(_)) => {}
            _ => panic!("only one side panicked: {ergs} {native} {op:?}"),
        }
    }

    #[test]
    fn unrepresentable_costs_run_the_original() {
        for (ergs, native) in [(1000, 100), (1000, 10), (u64::MAX, u64::MAX), (0, 0)] {
            for op in [
                Op::Step(u64::MAX, u64::MAX),
                Op::Step(u64::MAX, 5),
                Op::Step(u64::MAX / 256 + 1, 5),
                Op::Step(1, u64::MAX),
                Op::Step(1, u64::MAX - 19),
                Op::Step(1, u64::MAX - 20),
                Op::Step(1 << 40, 1 << 40),
                Op::GasAndNative(u64::MAX, 5),
                Op::GasAndNative(1 << 40, u64::MAX),
            ] {
                check_unrepresentable::<BaseResources<DecreasingNative>>(ergs, native, op);
                check_unrepresentable::<GasOnlyResources>(ergs, native, op);
            }
        }
    }

    /// Sequences of charges and of the other things a run does with its resources, from the
    /// budgets at the edges, compared with the original functions after every step
    fn check_sequences<R: Resources>(steps: usize) {
        let ops = [
            Op::Step(3, 30),
            Op::Step(1 << 23, 0),
            Op::Step(0, (1 << 31) - 20),
            Op::Step(10, 5),
            Op::Native(20),
            Op::Native(1 << 31),
            Op::GasAndNative(1, 20),
            Op::GasAndNative((1 << 23) + 1, (1 << 31) + 1),
            Op::Outlined(2, 100),
            Op::InfiniteErgs(1 << 31),
            Op::CallAndReturn(700),
            Op::GrowthFailsAfterCharge(3, 40),
            Op::Rerun,
        ];
        let budgets = budgets();
        let mut x = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 33) as usize
        };
        for &ergs in &budgets {
            for &native in &budgets {
                let mut pair = Pair::new(amount::<R>(ergs, native));
                for _ in 0..steps {
                    let op = match ops[next() % ops.len()] {
                        // an exact exhaustion of what is left, now and then
                        Op::Native(_) if next() % 4 == 0 => {
                            Op::Native(pair.whole.native().as_u64())
                        }
                        op => op,
                    };
                    pair.apply(op);
                }
                pair.rerun();
            }
        }
    }

    #[test]
    fn sequences_match_the_original() {
        let steps = if cfg!(miri) { 12 } else { 200 };
        check_sequences::<BaseResources<DecreasingNative>>(steps);
        check_sequences::<GasOnlyResources>(steps);
        check_sequences::<NativeFirst>(steps / 4);
    }

    /// The borrow path: a low part of 0 with a high part, charges of exactly 2^31, and a
    /// budget that runs out to exactly 0 across a borrow
    #[test]
    fn borrows_at_the_boundaries() {
        type R = BaseResources<DecreasingNative>;
        for (ergs, native) in [(1 << 31, 1 << 31), (2 << 31, 3 << 31), (1 << 32, 1 << 31)] {
            let mut pair = Pair::new(amount::<R>(ergs, native));
            assert_eq!((pair.low.ergs, pair.low.native), (0, 0));
            pair.apply(Op::Step(0, 0)); // the step native borrows
            pair.apply(Op::Step(1, 0)); // so does the gas
            pair.apply(Op::Step(1 << 23, (1 << 31) - 20)); // 2^31 of each: borrows again
            let (ergs, native) = (pair.whole.ergs().0, pair.whole.native().as_u64());
            pair.apply(Op::GasAndNative(ergs / 256, native)); // down to (ergs % 256, 0)
            pair.apply(Op::GasAndNative(0, 1)); // out of native
            pair.apply(Op::Step(1, 0)); // out of gas after that
        }
    }

    /// A holder for the handle, as `HotFrameParts` is for `Hot`
    struct TestHolder {
        instruction_pointer: usize,
        resources: BaseResources<DecreasingNative>,
    }

    impl ResourcesHolder for TestHolder {
        type Resources = BaseResources<DecreasingNative>;
        fn resources(&self) -> &Self::Resources {
            &self.resources
        }
        fn resources_mut(&mut self) -> &mut Self::Resources {
            &mut self.resources
        }
    }

    /// `Hot::new`, `Hot::outlined` and `Hot::finish`, step for step, on the real handle (a
    /// `Hot` needs a whole system); run under Miri for the pointer's life
    struct TestHot<'h> {
        ip: usize,
        resources: InHolder<'h, TestHolder>,
        low: LowResources,
    }

    impl<'h> TestHot<'h> {
        fn new(holder: &'h mut TestHolder) -> Self {
            Self {
                ip: holder.instruction_pointer,
                low: LowResources::split(&mut holder.resources),
                resources: InHolder::new(holder),
            }
        }

        fn outlined<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
            self.low.suspend(self.resources.get_mut());
            self.resources.set_outlined(true);
            // SAFETY: as in `Hot::outlined` (`TestHot` has no drop glue either)
            let result = unsafe {
                let mut copy = core::ptr::read(self);
                let result = f(&mut copy);
                core::ptr::write(self, copy);
                result
            };
            self.resources.set_outlined(false);
            self.low.resume(self.resources.get_mut());
            result
        }

        fn finish(self) {
            let Self { ip, resources, low } = self;
            let holder = resources.into_holder();
            holder.instruction_pointer = ip;
            low.join(&mut holder.resources);
        }
    }

    #[test]
    fn handle_lifecycle() {
        type R = BaseResources<DecreasingNative>;
        let mut whole = amount::<R>((5 << 31) + 1000, (1 << 32) + 60);
        let mut holder = TestHolder {
            instruction_pointer: 7,
            resources: whole.clone(),
        };
        for run in 0..3 {
            let mut hot = TestHot::new(&mut holder);
            let view = hot.resources.get();
            let _ = view.native();
            for i in 0..4 {
                let (gas, native) = (1 + i, 20 * i);
                assert_eq!(
                    hot.low.charge_step(hot.resources.get_mut(), gas, native),
                    charge_step(&mut whole, gas, native)
                );
                assert_eq!(hot.low.joined(hot.resources.get()), whole);
                let got = hot.outlined(|h| {
                    let gas_left = h.low.legacy_gas(h.resources.get());
                    let result = charge_step(h.resources.whole(), 3, 1 << 31);
                    assert_eq!(gas_left, h.resources.whole().legacy_gas() + 3);
                    result
                });
                assert_eq!(got, charge_step(&mut whole, 3, 1 << 31));
            }
            let got = hot.outlined(|h| {
                h.ip += 1;
                h.resources.whole().take()
            });
            assert_eq!(got, whole.take());
            hot.finish();
            assert_eq!(holder.instruction_pointer, 8 + run);
            holder.resources.reclaim(got.clone());
            whole.reclaim(got);
            assert_eq!(holder.resources, whole);
        }
    }

    fn split_holder() -> TestHolder {
        TestHolder {
            instruction_pointer: 0,
            resources: amount((1 << 31) + 5, 1 << 40),
        }
    }

    /// The whole resources are there only in an outlined instruction
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "whole resources while split")]
    fn whole_while_split() {
        let mut holder = split_holder();
        let mut hot = TestHot::new(&mut holder);
        let _ = hot.resources.whole();
    }

    /// A charge on the holder while split, e.g. an outlined body inlined without
    /// `Hot::charge_step`: 256 ergs from the high part leave 2^31 - 256 there
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "charged on the high parts")]
    fn charge_on_the_high_parts() {
        let mut holder = split_holder();
        let mut hot = TestHot::new(&mut holder);
        assert_eq!(spend_gas(hot.resources.get_mut(), 1), Ok(()));
        let _ = hot.low.charge_step(hot.resources.get_mut(), 3, 0);
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct NativeFirst {
        ergs: Ergs<1>,
        native: DecreasingNative,
    }

    impl Resource for NativeFirst {
        const FORMAL_INFINITE: Self = Self {
            ergs: Ergs::<1>::FORMAL_INFINITE,
            native: DecreasingNative::FORMAL_INFINITE,
        };

        fn empty() -> Self {
            Self {
                ergs: Ergs(0),
                native: DecreasingNative::empty(),
            }
        }

        fn is_empty(&self) -> bool {
            self.ergs.is_empty() && self.native.is_empty()
        }
        fn charge(&mut self, cost: &Self) -> Result<(), SystemError> {
            self.native.charge(&cost.native)?;
            self.ergs.charge(&cost.ergs)
        }
        fn charge_unchecked(&mut self, cost: &Self) {
            self.native.charge_unchecked(&cost.native);
            self.ergs.charge_unchecked(&cost.ergs);
        }
        fn has_enough(&self, cost: &Self) -> bool {
            self.ergs.has_enough(&cost.ergs) && self.native.has_enough(&cost.native)
        }
        fn reclaim(&mut self, value: Self) {
            self.ergs.reclaim(value.ergs);
            self.native.reclaim(value.native);
        }
        fn reclaim_withheld(&mut self, value: Self) {
            self.ergs.reclaim_withheld(value.ergs);
            self.native.reclaim_withheld(value.native);
        }
        fn diff(&self, other: Self) -> Self {
            Self {
                ergs: self.ergs.diff(other.ergs),
                native: self.native.diff(other.native),
            }
        }
        fn remaining(&self) -> Self {
            self.clone()
        }
    }

    impl Resources for NativeFirst {
        type Native = DecreasingNative;
        type Ergs = Ergs<1>;

        fn from_ergs(ergs: Self::Ergs) -> Self {
            Self {
                ergs,
                native: DecreasingNative::empty(),
            }
        }
        fn from_native(native: Self::Native) -> Self {
            Self {
                ergs: Ergs(0),
                native,
            }
        }
        fn from_ergs_and_native(ergs: Self::Ergs, native: Self::Native) -> Self {
            Self { ergs, native }
        }
        fn add_ergs(&mut self, value: Self::Ergs) {
            self.ergs.0 += value.0;
        }
        fn ergs(&self) -> Self::Ergs {
            self.ergs
        }
        fn native(&self) -> Self::Native {
            self.native.clone()
        }
        fn exhaust_ergs(&mut self) {
            self.ergs = Ergs(0);
        }
        fn give_native_to(&mut self, other: &mut Self) {
            other.native = core::mem::replace(&mut self.native, DecreasingNative::empty());
        }
        fn take(&mut self) -> Self {
            core::mem::replace(self, Self::empty())
        }
        fn with_infinite_ergs<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
            let old = self.ergs;
            self.ergs = Ergs::<1>::FORMAL_INFINITE;
            let result = f(self);
            self.ergs = old;
            result
        }
    }
}
