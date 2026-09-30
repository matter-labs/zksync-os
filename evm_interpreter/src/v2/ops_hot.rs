//! Instructions that run on the hot state (and the cold part read through its pointer):
//! inlined into the dispatch loop.

use u256::U256;
use zk_ee::system::evm::EvmError;
use zk_ee::system::EthereumLikeTypes;

use super::hot::{read_be_word, write_be_word, Hot};
use super::ops_cold::resize_heap;
use super::{cast_to_usize, ColdFrameParts};
use crate::gas_constants;
use crate::i256::i256_cmp;
use crate::instructions::bitwise::apply_sar;
use crate::native_resource_constants::*;
use crate::opcodes;
use crate::{ExitCode, InstructionResult, STACK_SIZE};

/// JUMP / JUMPI charge the destination JUMPDEST and skip its dispatch on the proving target
/// only, so that host tracers keep seeing a JUMPDEST step (see `crate::instructions::control_flow`).
const INLINE_JUMPDEST: bool = cfg!(target_arch = "riscv32");

/// PUSH2 also runs a JUMP or JUMPI right after it on the proving target only (see
/// [`Hot::push2`]): the host dispatches both, so its tracers see every step.
/// `evm_fuse_push2_jumps` turns it on for host tests.
const FUSE_PUSH2_JUMPS: bool = cfg!(any(target_arch = "riscv32", evm_fuse_push2_jumps));

// `push2` tells the two jumps apart from their distance to JUMP
const _: () = assert!(opcodes::JUMPI == opcodes::JUMP + 1);

/// Reads an immediate of `N <= 8` bytes that runs past the end of the code (zero padded)
#[cold]
#[inline(never)]
unsafe fn read_immediate_truncated<const N: usize>(ip: *const u8, end: *const u8) -> u64 {
    let mut acc = 0u64;
    for i in 0..N {
        let byte = if ip.wrapping_add(i) < end {
            ip.add(i).read()
        } else {
            0
        };
        acc = (acc << 8) | byte as u64;
    }
    acc
}

/// Zeroes the uninitialized slot `dst`, then writes the truncated immediate into it as
/// [`write_immediate_truncated`] does
#[cold]
#[inline(never)]
unsafe fn zero_and_write_immediate_truncated<const N: usize>(
    ip: *const u8,
    end: *const u8,
    dst: *mut u8,
) {
    U256::write_zero_into_ptr(dst.cast());
    write_immediate_truncated::<N>(ip, end, dst);
}

/// Writes the `N`-byte immediate at `ip`, which runs past the end of the code, into the
/// zeroed slot `dst` (little-endian, i.e. reversed)
#[cold]
#[inline(never)]
unsafe fn write_immediate_truncated<const N: usize>(ip: *const u8, end: *const u8, dst: *mut u8) {
    for i in 0..N {
        if ip.wrapping_add(i) < end {
            dst.add(N - 1 - i).write(ip.add(i).read());
        }
    }
}

/// `b = (a < b) as U256`, using `a` (a popped operand) as scratch: `a - b` borrows iff `a < b`.
/// One subtraction, where a comparison is a copy to scratch, an equality test and a
/// subtraction; the flag is written with plain word stores.
#[inline(always)]
fn less_than_into(a: &mut U256, b: &mut U256) {
    let less = a.overflowing_sub_assign(b);
    *b = U256::from_limbs([less as u64, 0, 0, 0]);
}

/// `b = (a > b) as U256`, using `a` (a popped operand) as scratch: `a = b - a` borrows iff
/// `b < a`
#[inline(always)]
fn greater_than_into(a: &mut U256, b: &mut U256) {
    let greater = a.overflowing_sub_assign_reversed(b);
    *b = U256::from_limbs([greater as u64, 0, 0, 0]);
}

impl<'h, S: EthereumLikeTypes> Hot<'h, S> {
    // --- stack ---------------------------------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn pop(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::BASE, POP_NATIVE_COST)?;
        self.stack.pop_and_ignore()
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn push0(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::BASE, PUSH0_NATIVE_COST)?;
        self.stack.push_zero()
    }

    /// PUSH1..=PUSH8: the immediate is assembled into a `u64`. The pointer is advanced first
    /// and the bytes are read behind it, so only one code register is live. (Storing the
    /// bytes straight into a zeroed slot measured the same; the cost is the zeroing
    /// delegation, the stack bookkeeping and the charge, not the assembly.)
    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn push_small<const N: usize>(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, PUSH_NATIVE_COSTS[N])?;
        self.ip = self.ip.wrapping_add(N);
        let value = if S::CODE_IS_PADDED || self.ip <= self.code_end {
            let mut acc = 0u64;
            for i in 0..N {
                // SAFETY: the whole immediate is in the code, or in its zero padding
                acc = (acc << 8) | unsafe { self.ip.sub(N - i).read() } as u64;
            }
            acc
        } else {
            // SAFETY: bounds checked per byte
            unsafe { read_immediate_truncated::<N>(self.ip.wrapping_sub(N), self.code_end) }
        };
        self.stack.push_u64(value)
    }

    /// PUSH2, which on the proving target also runs a JUMP or JUMPI right after it. The
    /// destination then stays in a register: no slot is written, and there is no second
    /// dispatch and no 256-bit destination check. Each instruction is still charged by its own
    /// `charge_step`, and fails with the same error at the same point and with the same stack,
    /// as when the two are dispatched one after the other.
    ///
    /// Keep it in sync with `push_small::<2>`. It differs in the bound on unpadded code
    /// (strict, for the peek), the depth test before the peek and the peek itself.
    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn push2(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        if !FUSE_PUSH2_JUMPS {
            return self.push_small::<2>();
        }
        self.charge_step(gas_constants::VERYLOW, PUSH_NATIVE_COSTS[2])?;
        self.ip = self.ip.wrapping_add(2);
        // Unpadded code: the byte after the immediate must be in the code too, since it is
        // peeked below. An immediate that ends the code exactly takes the other branch, which
        // reads it the same.
        if S::CODE_IS_PADDED || self.ip < self.code_end {
            // SAFETY: the whole immediate is in the code, or in its zero padding
            let value =
                unsafe { ((self.ip.sub(2).read() as u64) << 8) | self.ip.sub(1).read() as u64 };
            // Only if the push itself would succeed, and only on a code byte (not on the
            // padding of padded code): that byte is the one the next fetch would dispatch.
            if self.stack.depth < STACK_SIZE && (!S::CODE_IS_PADDED || self.ip < self.code_end) {
                // SAFETY: `ip < code_end`
                let next = unsafe { self.ip.read() };
                if next.wrapping_sub(opcodes::JUMP) < 2 {
                    let fused = if next == opcodes::JUMP {
                        self.jump_after_push2(cold, value as usize)
                    // JUMPI also pops the value under the destination: on an empty stack it
                    // is left to the dispatch, which fails it after both charges
                    } else if self.stack.depth != 0 {
                        self.jumpi_after_push2(cold, value as usize)
                    } else {
                        Ok(self.stack.push_u64(value))
                    };
                    return match fused {
                        Ok(result) => result,
                        Err(error) => {
                            // the jump's charge failed, when the dispatched PUSH2 has pushed
                            // its value (there is room: see above)
                            core::hint::cold_path();
                            let pushed = self.stack.push_u64(value);
                            debug_assert!(pushed.is_ok());
                            Err(error)
                        }
                    };
                }
            }
            self.stack.push_u64(value)
        } else {
            // the immediate ends at or past the end of the code: never fused
            // SAFETY: bounds checked per byte
            let value =
                unsafe { read_immediate_truncated::<2>(self.ip.wrapping_sub(2), self.code_end) };
            self.stack.push_u64(value)
        }
    }

    /// The JUMP at `ip`, after a PUSH2 of `dest` that was charged and whose stack check passed
    /// ([`Self::push2`]). The destination doesn't go to the stack, since the jump would pop it
    /// at once. The outer error is a failed charge of the JUMP, for which `push2` pushes the
    /// destination after all; the inner result is that of the jump.
    #[inline(always)]
    fn jump_after_push2(
        &mut self,
        cold: &ColdFrameParts<'_, S>,
        dest: usize,
    ) -> Result<InstructionResult, ExitCode> {
        // where the PUSH2 ends and the JUMP starts when the two are dispatched (so that the
        // marker count is the same)
        cycle_marker::opcode_end!("PUSH2");
        cycle_marker::opcode_start!();
        // past the JUMP, as its fetch leaves `ip`
        self.ip = self.ip.wrapping_add(1);
        self.charge_step(gas_constants::MID, JUMP_NATIVE_COST)?;
        // a pushed `u16` always passes `cast_to_usize`
        Ok(if cold.is_valid_jumpdest(dest) {
            self.land_on_jumpdest(cold, dest)
        } else {
            Err(EvmError::InvalidJump.into())
        })
    }

    /// [`Self::jump_after_push2`] for a JUMPI, on a stack of at least one value
    #[inline(always)]
    fn jumpi_after_push2(
        &mut self,
        cold: &ColdFrameParts<'_, S>,
        dest: usize,
    ) -> Result<InstructionResult, ExitCode> {
        cycle_marker::opcode_end!("PUSH2");
        cycle_marker::opcode_start!();
        self.ip = self.ip.wrapping_add(1);
        self.charge_step(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        // the condition is the value under the destination, i.e. JUMPI's second pop (`push2`
        // checked that there is one)
        let condition = match self.stack.pop_1() {
            Ok(condition) => condition,
            Err(error) => return Ok(Err(error)),
        };
        if condition.is_zero() {
            // the destination is not checked
            return Ok(Ok(()));
        }
        Ok(if cold.is_valid_jumpdest(dest) {
            self.land_on_jumpdest(cold, dest)
        } else {
            Err(EvmError::InvalidJump.into())
        })
    }

    /// PUSH9..=PUSH32: the immediate is written straight into the slot, reversed
    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn push_wide<const N: usize>(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, PUSH_NATIVE_COSTS[N])?;
        // On the proving target PUSH20 and PUSH32 assemble the immediate with `rev8`: they
        // carry almost all of the measured saving, other sizes save under a cycle per
        // push. Misaligned immediates take the helper's shift/glue path; the byte loop
        // below handles the other PUSH sizes. Their slot is not zeroed up front:
        // every byte is written below.
        let use_rev8 = const { cfg!(target_arch = "riscv32") && (N == 20 || N == 32) };
        let slot = if use_rev8 {
            self.stack.push_slot_uninit()?
        } else {
            self.stack.push_slot_zeroed()?
        };
        let dst = slot.cast::<u8>();
        self.ip = self.ip.wrapping_add(N);
        if S::CODE_IS_PADDED || self.ip <= self.code_end {
            if use_rev8 {
                // SAFETY: the whole immediate is readable, and `slot` is a 32-byte-aligned
                // stack slot that does not overlap the code. PUSH20 zeroes the high 12 bytes,
                // the helper writes the low N (a multiple of four, so no partial word).
                unsafe {
                    u256::byte_order::read_be_into_words::<N>(self.ip.sub(N), slot.cast());
                    if N == 20 {
                        for i in 5..8 {
                            slot.cast::<u32>().add(i).write(0);
                        }
                    }
                };
            } else {
                for i in 0..N {
                    // SAFETY: the whole immediate is in the code or in its zero padding, `dst`
                    // is a 32-byte slot
                    unsafe { dst.add(N - 1 - i).write(self.ip.sub(N - i).read()) };
                }
            }
        } else {
            // SAFETY: bounds checked per byte
            unsafe {
                if use_rev8 {
                    zero_and_write_immediate_truncated::<N>(
                        self.ip.wrapping_sub(N),
                        self.code_end,
                        dst,
                    )
                } else {
                    write_immediate_truncated::<N>(self.ip.wrapping_sub(N), self.code_end, dst)
                }
            };
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn dup_op<const N: usize>(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, DUP_NATIVE_COST)?;
        self.stack.dup::<N>()
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn swap_op<const N: usize>(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, SWAP_NATIVE_COST)?;
        self.stack.swap::<N>()
    }

    // --- arithmetic without the oracle ---------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn add(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, ADD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::AddAssign::add_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn mul(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::LOW, MUL_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        op2.wrapping_mul_assign(op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn sub(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, SUB_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        // op1 - op2, stored in op2
        op2.overflowing_sub_assign_reversed(op1);
        Ok(())
    }

    // --- comparison and bitwise ----------------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn lt(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, LT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        less_than_into(op1, op2);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn gt(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, GT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        greater_than_into(op1, op2);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn slt(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, SLT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if i256_cmp(op1, op2) == core::cmp::Ordering::Less {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn sgt(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, SGT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if i256_cmp(op1, op2) == core::cmp::Ordering::Greater {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn eq(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, EQ_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op1.eq(op2) {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn iszero(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, ISZERO_NATIVE_COST)?;
        let top = self.stack.top_mut()?;
        if top.is_zero() {
            U256::write_one(top);
        } else {
            U256::write_zero(top);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn bitand(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, AND_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitAndAssign::bitand_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn bitor(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, OR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitOrAssign::bitor_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn bitxor(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, XOR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitXorAssign::bitxor_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn not(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, NOT_NATIVE_COST)?;
        let top = self.stack.top_mut()?;
        top.not_mut();
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn byte(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, BYTE_NATIVE_COST)?;
        let (offset, src) = self.stack.pop_1_and_peek_mut()?;
        if let Some(offset) = offset.try_to_usize_capped::<32>() {
            let ret = src.byte(31 - offset);
            U256::write_zero(src);
            src.as_limbs_mut()[0] = ret as u64;
        } else {
            U256::write_zero(src);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn shl(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, SHL_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        match op1.try_to_usize() {
            Some(shift) if shift < 256 => *op2 <<= shift as u32,
            _ => U256::write_zero(op2),
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn shr(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, SHR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        match op1.try_to_usize() {
            Some(shift) if shift < 256 => *op2 >>= shift as u32,
            _ => U256::write_zero(op2),
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn sar(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, SAR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        apply_sar(op1, op2);
        Ok(())
    }

    // --- control flow --------------------------------------------------------------------

    /// The jump is taken to the valid `dest`
    #[inline(always)]
    fn land_on_jumpdest(&mut self, cold: &ColdFrameParts<'_, S>, dest: usize) -> InstructionResult {
        let start = cold.bytecode.as_ptr();
        if INLINE_JUMPDEST {
            // Charged separately from the JUMP / JUMPI base, so an invalid destination or the
            // not-taken branch doesn't pay JUMPDEST gas/native.
            self.spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
            self.ip = start.wrapping_add(dest + 1);
            // Synthetic cycle_marker pair, so the proving-side marker count balances the
            // host-side one, where JUMPDEST is still dispatched.
            cycle_marker::opcode_start!();
            cycle_marker::opcode_end!("JUMPDEST");
        } else {
            self.ip = start.wrapping_add(dest);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn jump(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::MID, JUMP_NATIVE_COST)?;
        let dest = self.stack.pop_1()?;
        let dest = cast_to_usize(dest, EvmError::InvalidJump.into())?;
        if cold.is_valid_jumpdest(dest) {
            self.land_on_jumpdest(cold, dest)
        } else {
            Err(EvmError::InvalidJump.into())
        }
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn jumpi(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        let (dest, value) = self.stack.pop_2()?;
        if value.is_zero() {
            return Ok(());
        }
        let dest = cast_to_usize(dest, EvmError::InvalidJump.into())?;
        if cold.is_valid_jumpdest(dest) {
            self.land_on_jumpdest(cold, dest)
        } else {
            Err(EvmError::InvalidJump.into())
        }
    }

    /// Dispatched on the host for every jump target, and on the proving target only when
    /// execution falls through onto a JUMPDEST byte
    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn jumpdest(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn pc(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, PC_NATIVE_COST)?;
        let pc = self.ip.addr() - cold.bytecode.as_ptr().addr() - 1;
        self.stack.push_u64(pc as u64)
    }

    // --- memory --------------------------------------------------------------------------

    /// Makes sure the heap covers `..max_offset` (the end of the accessed range, computed
    /// once with a saturating add), growing (and charging) if needed
    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn ensure_heap(
        &mut self,
        cold: &mut ColdFrameParts<'_, S>,
        max_offset: usize,
    ) -> InstructionResult {
        // the heap length is a multiple of 32, so this is the same test as after rounding up
        if max_offset > cold.heap.len() {
            self.suspended(|resources| resize_heap(cold, resources, max_offset))
        } else {
            Ok(())
        }
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn mload(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, MLOAD_NATIVE_COST)?;
        let top = self.stack.top_ptr()?;
        // SAFETY: an initialized slot of the stack
        let index = cast_to_usize(unsafe { &*top }, EvmError::InvalidOperandOOG.into())?;
        self.ensure_heap(cold, index.saturating_add(32))?;
        // SAFETY: the heap covers `index..index + 32`
        unsafe { read_be_word(cold.heap.as_ptr().add(index), top) };
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn mstore(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, MSTORE_NATIVE_COST)?;
        let (index, value) = self.stack.pop_2_ptr()?;
        // SAFETY: initialized slots of the stack, still valid after the pop
        let index = cast_to_usize(unsafe { &*index }, EvmError::InvalidOperandOOG.into())?;
        self.ensure_heap(cold, index.saturating_add(32))?;
        // SAFETY: the heap covers `index..index + 32`; the popped value slot is scratch
        unsafe { write_be_word(value.cast_mut(), cold.heap.as_mut_ptr().add(index)) };
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn mstore8(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, MSTORE8_NATIVE_COST)?;
        let (index, value) = self.stack.pop_2()?;
        let index = cast_to_usize(index, EvmError::InvalidOperandOOG.into())?;
        let value = value.byte(0);
        self.ensure_heap(cold, index.saturating_add(1))?;
        // SAFETY: the heap covers `index`
        unsafe { cold.heap.as_mut_ptr().add(index).write(value) };
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn msize(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, MSIZE_NATIVE_COST)?;
        let len = cold.heap.len();
        debug_assert!(len.next_multiple_of(32) == len);
        self.stack.push_u64(len as u64)
    }

    // --- frame environment ---------------------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn address(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, ADDRESS_NATIVE_COST)?;
        self.stack.push_b160(cold.address)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn caller(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, CALLER_NATIVE_COST)?;
        self.stack.push_b160(cold.caller)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn callvalue(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, CALLVALUE_NATIVE_COST)?;
        self.stack.push(&cold.call_value)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn calldatasize(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, CALLDATASIZE_NATIVE_COST)?;
        self.stack.push_u64(cold.calldata.len() as u64)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn calldataload(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::VERYLOW, CALLDATALOAD_NATIVE_COST)?;
        let top = self.stack.top_ptr()?;
        let calldata = cold.calldata;
        // SAFETY: an initialized slot of the stack
        let index = unsafe { &*top }.try_to_usize();
        match index {
            Some(index) if index < calldata.len() => {
                let have = 32.min(calldata.len() - index);
                if have == 32 {
                    // SAFETY: 32 bytes of calldata from `index`
                    unsafe { read_be_word(calldata.as_ptr().add(index), top) };
                } else {
                    // SAFETY: the slot is zeroed, then `have` bytes of calldata are written
                    // reversed at the top of the value
                    unsafe {
                        U256::write_zero_into_ptr(top);
                        let src = calldata.as_ptr().add(index);
                        let dst = top.cast::<u8>();
                        for i in 0..have {
                            dst.add(31 - i).write(src.add(i).read());
                        }
                    }
                }
            }
            // virtual zero-pad
            _ => unsafe { U256::write_zero_into_ptr(top) },
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn codesize(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, CODESIZE_NATIVE_COST)?;
        self.stack
            .push_u64(cold.bytecode_preprocessing.original_bytecode_len as u64)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn returndatasize(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        self.charge_step(gas_constants::BASE, RETURNDATASIZE_NATIVE_COST)?;
        self.stack.push_u64(cold.returndata.len() as u64)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn gas(&mut self) -> InstructionResult {
        self.charge_step(gas_constants::BASE, GAS_NATIVE_COST)?;
        let gas_left = self.gas_left();
        self.stack.push_u64(gas_left)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn stop(&mut self) -> InstructionResult {
        self.spend_native(STEP_NATIVE_COST)?;
        Err(ExitCode::Stop)
    }
}

#[cfg(test)]
mod tests {
    use super::{greater_than_into, less_than_into};
    use u256::U256;

    /// The in-place comparisons against `Ord`, around the limb boundaries
    #[test]
    fn comparisons_in_place_match_ord() {
        let mut values = vec![
            U256::ZERO,
            U256::ONE,
            U256::from_limbs([u64::MAX, 0, 0, 0]),
            U256::from_limbs([0, 1, 0, 0]),
            U256::from_limbs([0, 0, 1, 0]),
            U256::from_limbs([u64::MAX, u64::MAX, 0, 0]),
            U256::from_limbs([0, 0, 0, 1 << 63]),
            U256::from_limbs([u64::MAX - 1, u64::MAX, u64::MAX, u64::MAX]),
            U256::from_limbs([u64::MAX; 4]),
        ];
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            x
        };
        for _ in 0..24 {
            let limbs = [next(), next(), next(), next()];
            values.push(U256::from_limbs(limbs));
            // the same high limbs, a different low limb
            values.push(U256::from_limbs([next(), limbs[1], limbs[2], limbs[3]]));
        }
        for a in &values {
            for b in &values {
                let (mut scratch, mut result) = (a.clone(), b.clone());
                less_than_into(&mut scratch, &mut result);
                assert_eq!(result, if a < b { U256::ONE } else { U256::ZERO });
                let (mut scratch, mut result) = (a.clone(), b.clone());
                greater_than_into(&mut scratch, &mut result);
                assert_eq!(result, if a > b { U256::ONE } else { U256::ZERO });
            }
        }
    }
}
