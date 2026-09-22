//! Instructions that run on the hot state (and the cold part read through its pointer):
//! inlined into the dispatch loop.

use u256::U256;
use zk_ee::system::evm::EvmError;
use zk_ee::system::EthereumLikeTypes;

use super::hot::{
    charge_step, read_be_word, spend_gas_and_native, spend_native, write_be_word, Hot,
};
use super::ops_cold::resize_heap;
use super::{cast_to_usize, ColdFrameParts};
use crate::gas_constants;
use crate::i256::i256_cmp;
use crate::instructions::bitwise::apply_sar;
use crate::native_resource_constants::*;
use crate::{ExitCode, InstructionResult};

/// JUMP / JUMPI charge the destination JUMPDEST and skip its dispatch on the proving target
/// only, so that host tracers keep seeing a JUMPDEST step (see `crate::instructions::control_flow`).
const INLINE_JUMPDEST: bool = cfg!(target_arch = "riscv32");

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

impl<'h, S: EthereumLikeTypes> Hot<'h, S> {
    // --- stack ---------------------------------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn pop(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::BASE, POP_NATIVE_COST)?;
        self.stack.pop_and_ignore()
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn push0(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::BASE, PUSH0_NATIVE_COST)?;
        self.stack.push_zero()
    }

    /// PUSH1..=PUSH8: the immediate is assembled into a `u64`. The pointer is advanced first
    /// and the bytes are read behind it, so only one code register is live. (Storing the
    /// bytes straight into a zeroed slot measured the same; the cost is the zeroing
    /// delegation, the stack bookkeeping and the charge, not the assembly.)
    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn push_small<const N: usize>(&mut self) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            PUSH_NATIVE_COSTS[N],
        )?;
        self.ip = self.ip.wrapping_add(N);
        let value = if self.ip <= self.code_end {
            let mut acc = 0u64;
            for i in 0..N {
                // SAFETY: the whole immediate is in the code
                acc = (acc << 8) | unsafe { self.ip.sub(N - i).read() } as u64;
            }
            acc
        } else {
            // SAFETY: bounds checked per byte
            unsafe { read_immediate_truncated::<N>(self.ip.wrapping_sub(N), self.code_end) }
        };
        self.stack.push_u64(value)
    }

    /// PUSH9..=PUSH32: the immediate is written straight into the slot, reversed
    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn push_wide<const N: usize>(&mut self) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            PUSH_NATIVE_COSTS[N],
        )?;
        let slot = self.stack.push_slot_zeroed()?;
        let dst = slot.cast::<u8>();
        self.ip = self.ip.wrapping_add(N);
        if self.ip <= self.code_end {
            for i in 0..N {
                // SAFETY: the whole immediate is in the code, `dst` is a 32-byte slot
                unsafe { dst.add(N - 1 - i).write(self.ip.sub(N - i).read()) };
            }
        } else {
            // SAFETY: bounds checked per byte
            unsafe { write_immediate_truncated::<N>(self.ip.wrapping_sub(N), self.code_end, dst) };
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn dup_op<const N: usize>(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, DUP_NATIVE_COST)?;
        self.stack.dup::<N>()
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn swap_op<const N: usize>(&mut self) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            SWAP_NATIVE_COST,
        )?;
        self.stack.swap::<N>()
    }

    // --- arithmetic without the oracle ---------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn add(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, ADD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::AddAssign::add_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn mul(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::LOW, MUL_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        op2.wrapping_mul_assign(op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn sub(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, SUB_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        // op1 - op2, stored in op2
        op2.overflowing_sub_assign_reversed(op1);
        Ok(())
    }

    // --- comparison and bitwise ----------------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn lt(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, LT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op1.lt(op2) {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn gt(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, GT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op1.gt(op2) {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn slt(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, SLT_NATIVE_COST)?;
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
        charge_step(&mut self.resources, gas_constants::VERYLOW, SGT_NATIVE_COST)?;
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
        charge_step(&mut self.resources, gas_constants::VERYLOW, EQ_NATIVE_COST)?;
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
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            ISZERO_NATIVE_COST,
        )?;
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
        charge_step(&mut self.resources, gas_constants::VERYLOW, AND_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitAndAssign::bitand_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn bitor(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, OR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitOrAssign::bitor_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn bitxor(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, XOR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitXorAssign::bitxor_assign(op2, op1);
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn not(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::VERYLOW, NOT_NATIVE_COST)?;
        let top = self.stack.top_mut()?;
        top.not_mut();
        Ok(())
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn byte(&mut self) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            BYTE_NATIVE_COST,
        )?;
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
        charge_step(&mut self.resources, gas_constants::VERYLOW, SHL_NATIVE_COST)?;
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
        charge_step(&mut self.resources, gas_constants::VERYLOW, SHR_NATIVE_COST)?;
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
        charge_step(&mut self.resources, gas_constants::VERYLOW, SAR_NATIVE_COST)?;
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
            spend_gas_and_native(
                &mut self.resources,
                gas_constants::JUMPDEST,
                JUMPDEST_NATIVE_COST,
            )?;
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
        charge_step(&mut self.resources, gas_constants::MID, JUMP_NATIVE_COST)?;
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
        charge_step(&mut self.resources, gas_constants::HIGH, JUMPI_NATIVE_COST)?;
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
        charge_step(
            &mut self.resources,
            gas_constants::JUMPDEST,
            JUMPDEST_NATIVE_COST,
        )
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn pc(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::BASE, PC_NATIVE_COST)?;
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
            self.outlined(|hot| resize_heap(cold, &mut hot.resources, max_offset))
        } else {
            Ok(())
        }
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn mload(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            MLOAD_NATIVE_COST,
        )?;
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
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            MSTORE_NATIVE_COST,
        )?;
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
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            MSTORE8_NATIVE_COST,
        )?;
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
        charge_step(&mut self.resources, gas_constants::BASE, MSIZE_NATIVE_COST)?;
        let len = cold.heap.len();
        debug_assert!(len.next_multiple_of(32) == len);
        self.stack.push_u64(len as u64)
    }

    // --- frame environment ---------------------------------------------------------------

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn address(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            ADDRESS_NATIVE_COST,
        )?;
        self.stack.push_b160(cold.address)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn caller(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::BASE, CALLER_NATIVE_COST)?;
        self.stack.push_b160(cold.caller)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn callvalue(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            CALLVALUE_NATIVE_COST,
        )?;
        self.stack.push(&cold.call_value)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn calldatasize(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            CALLDATASIZE_NATIVE_COST,
        )?;
        self.stack.push_u64(cold.calldata.len() as u64)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn calldataload(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::VERYLOW,
            CALLDATALOAD_NATIVE_COST,
        )?;
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
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            CODESIZE_NATIVE_COST,
        )?;
        self.stack
            .push_u64(cold.bytecode_preprocessing.original_bytecode_len as u64)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn returndatasize(&mut self, cold: &ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            RETURNDATASIZE_NATIVE_COST,
        )?;
        self.stack.push_u64(cold.returndata.len() as u64)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn gas(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::BASE, GAS_NATIVE_COST)?;
        let gas_left = self.gas_left();
        self.stack.push_u64(gas_left)
    }

    #[cfg_attr(not(opcode_profile), inline(always))]
    #[cfg_attr(opcode_profile, inline(never))]
    pub fn stop(&mut self) -> InstructionResult {
        spend_native(&mut self.resources, STEP_NATIVE_COST)?;
        Err(ExitCode::Stop)
    }
}
