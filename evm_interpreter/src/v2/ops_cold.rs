//! Instructions that need the system, the heap growth path or long bodies: outlined, run on
//! a copy of the hot state (see `Hot::outlined`) with the cold part of the frame.

use core::hint::unreachable_unchecked;

use ruint::aliases::B160;
use u256::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::evm::EvmError;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{
    CallModifier, ErgsResource, EthereumLikeTypes, IOSubsystem, IOSubsystemExt, Resources,
    SystemFunctions, SystemFunctionsExt, MAX_EVENT_TOPICS,
};
use zk_ee::utils::{copy_and_zeropad_nonoverlapping, Bytes32};
use zk_ee::wrap_error;

use super::hot::{
    charge_step, pay_for_memory_growth, spend_gas, spend_gas_and_native, spend_native, Hot,
};
use super::{
    cast_offset_and_len, cast_to_u64, cast_to_usize, fatal_exit, subsystem_error_exit,
    system_error_exit, ColdFrameParts, Env,
};
use crate::gas::gas_utils;
use crate::gas_constants::{self, CALL_STIPEND, INITCODE_WORD_COST, SHA3WORD};
use crate::i256::{i256_div, i256_mod};
use crate::instructions::arithmetic::{exp_cost, reduce_mod_max};
use crate::interpreter::{CallScheme, EVMCallRequest};
use crate::native_resource_constants::*;
use crate::{ExitCode, InstructionResult, PendingOsRequest, MAX_INITCODE_SIZE, THIS_EE_TYPE};

#[cfg(not(target_arch = "riscv32"))]
use zk_ee::system::tracer::evm_tracer::EvmTracer;

/// Address derivation and other helpers shared with the first interpreter
type V1<'a, S> = crate::Interpreter<'a, S>;

/// Grows the heap to cover `..max_offset` (the end of the accessed range, computed once by
/// the caller with a saturating add), charging the expansion
#[inline(never)]
pub(crate) fn resize_heap<S: EthereumLikeTypes>(
    cold: &mut ColdFrameParts<'_, S>,
    resources: &mut S::Resources,
    max_offset: usize,
) -> InstructionResult {
    let new_heap_size = if max_offset > ((u32::MAX - 31) as usize) {
        return Err(ExitCode::EvmError(EvmError::MemoryLimitOOG));
    } else {
        max_offset.next_multiple_of(32)
    };
    let current_heap_size = cold.heap.len();
    if new_heap_size > current_heap_size {
        pay_for_memory_growth(
            resources,
            &mut cold.gas_paid_for_heap_growth,
            current_heap_size,
            new_heap_size,
        )?;
        cold.heap
            .resize(new_heap_size, 0)
            .map_err(|_| ExitCode::EvmError(EvmError::OutOfGas))?;
    }
    Ok(())
}

/// Copies the returndata of a call into the range requested by the call
pub(crate) fn copy_returndata_to_heap<'ee, S: EthereumLikeTypes>(
    cold: &mut ColdFrameParts<'ee, S>,
    resources: &mut S::Resources,
    returndata_region: &'ee [u8],
) -> InstructionResult {
    if !cold.returndata_location.is_empty() {
        let to_copy = core::cmp::min(returndata_region.len(), cold.returndata_location.len());
        if to_copy > 0 {
            let (_, native_cost) = gas_utils::copy_cost(to_copy as u64)?;
            spend_gas_and_native(resources, 0, native_cost)?;
            // SAFETY: the location is in the heap (resized by the call instruction)
            unsafe {
                let src = returndata_region.as_ptr();
                let dst = cold.heap.as_mut_ptr().add(cold.returndata_location.start);
                core::ptr::copy_nonoverlapping(src, dst, to_copy);
            }
        }
    }
    cold.returndata = returndata_region;
    Ok(())
}

impl<'h, S: EthereumLikeTypes> Hot<'h, S> {
    #[inline(never)]
    pub(crate) fn invalid_opcode(&mut self, opcode: u8) -> InstructionResult {
        spend_native(&mut self.resources, STEP_NATIVE_COST)?;
        Err(EvmError::InvalidOpcode(opcode).into())
    }

    // --- arithmetic through the oracle ---------------------------------------------------

    #[inline(never)]
    pub(crate) fn div<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, gas_constants::LOW, DIV_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if !op2.is_zero() {
            S::SystemFunctionsExt::u256_div_nonzero_divisor(op1, op2, env.system.io.oracle());
        }
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn sdiv<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, gas_constants::LOW, SDIV_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        i256_div(op1, op2, |a, b| {
            S::SystemFunctionsExt::u256_div_nonzero_divisor(a, b, env.system.io.oracle())
        });
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn rem<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, gas_constants::LOW, MOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if !op2.is_zero() {
            S::SystemFunctionsExt::u256_rem_nonzero_divisor(op1, op2, env.system.io.oracle());
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn smod<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, gas_constants::LOW, SMOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if !op2.is_zero() {
            i256_mod(op1, op2, |a, b| {
                S::SystemFunctionsExt::u256_rem_nonzero_divisor(a, b, env.system.io.oracle())
            })
        };
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn addmod<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, gas_constants::MID, ADDMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_mut_and_peek()?;
        if op3.is_zero() {
            return Ok(());
        }
        // the modulus slot (op3) receives the result; op1 is scratch
        S::SystemFunctionsExt::u256_addmod_nonzero_modulus(op1, op2, op3, env.system.io.oracle());
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn mulmod<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, gas_constants::MID, MULMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_mut_and_peek()?;
        if op3.is_zero() {
            return Ok(());
        }
        // modulus = 1: the result is 0
        if op3.is_one() {
            U256::write_zero(op3);
            return Ok(());
        }
        // modulus = 2^256 - 1: no wide division needed
        if op3.is_max() {
            // the popped slot of op1 holds the low half of the product in place
            let mut product_hi = op1.clone();
            op1.widening_mul_assign_into(&mut product_hi, op2);
            reduce_mod_max(op1, &product_hi, op3);
            return Ok(());
        }
        // the modulus slot (op3) receives the result; op1 is scratch
        S::SystemFunctionsExt::u256_mulmod_nonzero_modulus(op1, op2, op3, env.system.io.oracle());
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn exp(&mut self) -> InstructionResult {
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if let Some((gas_cost, native_cost)) = exp_cost(op2) {
            charge_step(&mut self.resources, gas_cost, native_cost)?;
        } else {
            return Err(ExitCode::EvmError(EvmError::OutOfGas));
        }
        let exp = op2.clone();
        U256::pow(op1, &exp, op2);
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn signextend(&mut self) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::LOW,
            SIGNEXTEND_NATIVE_COST,
        )?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if let Some(shift) = op1.try_to_usize_capped::<32>() {
            let bit_index = 8 * shift + 7;
            let bit = op2.bit(bit_index);
            let mut mask = U256::one();
            core::ops::ShlAssign::shl_assign(&mut mask, bit_index as u32);
            let one = U256::one();
            core::ops::SubAssign::sub_assign(&mut mask, &one);
            if bit {
                mask.not_mut();
                core::ops::BitOrAssign::bitor_assign(op2, &mask);
            } else {
                core::ops::BitAndAssign::bitand_assign(op2, &mask);
            }
        }
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn clz(&mut self) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::LOW, CLZ_NATIVE_COST)?;
        let op = self.stack.top_mut()?;
        *op = if op.is_zero() {
            U256::from(256u64)
        } else {
            U256::from(op.leading_zeros() as u64)
        };
        Ok(())
    }

    // --- hashing -------------------------------------------------------------------------

    const EMPTY_SLICE_SHA3: U256 = U256::from_limbs([
        0x7bfad8045d85a470,
        0xe500b653ca82273b,
        0x927e7db2dcc703c0,
        0xc5d2460186f7233c,
    ]);

    #[inline(never)]
    pub(crate) fn sha3<T: Tracer<S>>(
        &mut self,
        cold: &mut ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult {
        // The marker wraps the whole instruction, matching the per-dispatch sample count of
        // `EvmOpcodeStatsTracer` (see the v1 `sha3`).
        cycle_marker::wrap!("keccak_execution_environment", {
            // `len` stays on the stack: that is the slot the hash goes to
            let (memory_offset, len) = self.stack.pop_1_and_peek_mut()?;
            charge_step(&mut self.resources, 0, KECCAK256_NATIVE_COST)?;
            let len = cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
            let memory_offset: Option<usize> = if len > 0 {
                Some(cast_to_usize(
                    memory_offset,
                    EvmError::InvalidOperandOOG.into(),
                )?)
            } else {
                None
            };

            match memory_offset {
                None => {
                    spend_gas(&mut self.resources, gas_constants::SHA3)?;
                    *self.stack.top_mut()? = Self::EMPTY_SLICE_SHA3;
                }
                Some(memory_offset) => {
                    resize_heap(cold, &mut self.resources, memory_offset.saturating_add(len))?;
                    let allocator = env.system.get_allocator();
                    let input = &cold.heap[memory_offset..(memory_offset + len)];
                    let dst = self.stack.top_mut()?;
                    S::SystemFunctions::keccak256_with_closure(
                        input,
                        |hash| dst.assign_from_be_bytes(hash),
                        &mut self.resources,
                        allocator,
                    )
                    .map_err(SystemError::from)
                    .map_err(system_error_exit)?;
                }
            };
            Ok(())
        })
    }

    // --- copies --------------------------------------------------------------------------

    #[inline(never)]
    pub(crate) fn codecopy(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len_u64 = cast_to_u64(len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len_u64)?;
        charge_step(
            &mut self.resources,
            gas_cost,
            native_cost + CODECOPY_NATIVE_COST,
        )?;
        let len = cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            return Ok(());
        }
        let memory_offset = cast_to_usize(memory_offset, EvmError::InvalidOperandOOG.into())?;
        let source_offset = source_offset.try_to_usize();
        resize_heap(cold, &mut self.resources, memory_offset.saturating_add(len))?;
        let source = source_offset
            .and_then(|offset| cold.bytecode.get(offset..))
            .unwrap_or(&[]);
        copy_and_zeropad_nonoverlapping(source, &mut cold.heap[memory_offset..memory_offset + len]);
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn calldatacopy(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len_u64 = cast_to_u64(len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len_u64)?;
        charge_step(
            &mut self.resources,
            gas_cost,
            CALLDATACOPY_NATIVE_COST + native_cost,
        )?;
        let len = cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            return Ok(());
        }
        let memory_offset = cast_to_usize(memory_offset, EvmError::InvalidOperandOOG.into())?;
        let source_offset = source_offset.try_to_usize();
        resize_heap(cold, &mut self.resources, memory_offset.saturating_add(len))?;
        let source = source_offset
            .and_then(|offset| cold.calldata.get(offset..))
            .unwrap_or(&[]);
        copy_and_zeropad_nonoverlapping(source, &mut cold.heap[memory_offset..memory_offset + len]);
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn returndatacopy(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len_u64 = cast_to_u64(len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len_u64)?;
        charge_step(
            &mut self.resources,
            gas_cost,
            RETURNDATACOPY_NATIVE_COST + native_cost,
        )?;
        let len = cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        let source_offset = cast_to_usize(source_offset, EvmError::InvalidOperandOOG.into())?;
        let (end, of) = source_offset.overflowing_add(len);
        let returndata_len = cold.returndata.len();
        if of || end > returndata_len {
            return Err(EvmError::ReturnDataOutOfBounds.into());
        }
        if len == 0 {
            return Ok(());
        }
        let memory_offset = cast_to_usize(memory_offset, EvmError::InvalidOperandOOG.into())?;
        resize_heap(cold, &mut self.resources, memory_offset.saturating_add(len))?;
        copy_and_zeropad_nonoverlapping(
            cold.returndata.get(source_offset..).unwrap_or(&[]),
            &mut cold.heap[memory_offset..memory_offset + len],
        );
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn mcopy(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        let (dst_offset, src_offset, len) = self.stack.pop_3()?;
        let len_u64 = cast_to_u64(len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len_u64)?;
        charge_step(
            &mut self.resources,
            gas_cost,
            native_cost + MCOPY_NATIVE_COST,
        )?;
        let len = cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            return Ok(());
        }
        let dst_offset = cast_to_usize(dst_offset, EvmError::InvalidOperandOOG.into())?;
        let src_offset = cast_to_usize(src_offset, EvmError::InvalidOperandOOG.into())?;
        resize_heap(
            cold,
            &mut self.resources,
            core::cmp::max(dst_offset, src_offset).saturating_add(len),
        )?;
        // SAFETY: both ranges are in the heap; they may overlap
        unsafe {
            let src_ptr = cold.heap.as_ptr().add(src_offset);
            let dst_ptr = cold.heap.as_mut_ptr().add(dst_offset);
            core::ptr::copy(src_ptr, dst_ptr, len);
        }
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn extcodecopy<T: Tracer<S>>(
        &mut self,
        cold: &mut ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        let (address, memory_offset, source_offset, len) = self.stack.pop_4()?;
        let address = address.to_b160();
        let (memory_offset, len) =
            cast_offset_and_len(memory_offset, len, EvmError::InvalidOperandOOG.into())?;
        let source_offset = source_offset.try_to_usize();
        resize_heap(cold, &mut self.resources, memory_offset.saturating_add(len))?;
        let bytecode = env
            .system
            .io
            .get_observable_bytecode(THIS_EE_TYPE, &mut self.resources, &address)
            .map_err(system_error_exit)?;
        let source = source_offset
            .and_then(|offset| bytecode.get(offset..))
            .unwrap_or(&[]);
        let (gas_cost, native_cost) = gas_utils::copy_cost(len as u64)?;
        charge_step(
            &mut self.resources,
            gas_cost,
            native_cost + EXTCODECOPY_NATIVE_COST,
        )?;
        copy_and_zeropad_nonoverlapping(source, &mut cold.heap[memory_offset..memory_offset + len]);
        Ok(())
    }

    // --- returns -------------------------------------------------------------------------

    /// Sets the returndata range of RETURN / REVERT
    fn set_return_range(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        let (offset, len) = self.stack.pop_2()?;
        let len = cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            cold.returndata_location = 0..0;
        } else {
            let offset = cast_to_usize(offset, EvmError::InvalidOperandOOG.into())?;
            resize_heap(cold, &mut self.resources, offset.saturating_add(len))?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(EvmError::InvalidOperandOOG.into());
            }
            cold.returndata_location = offset..end;
        }
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn ret(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(&mut self.resources, 0, RETURN_NATIVE_COST)?;
        self.set_return_range(cold)?;
        Err(ExitCode::Return)
    }

    #[inline(never)]
    pub(crate) fn revert(&mut self, cold: &mut ColdFrameParts<'_, S>) -> InstructionResult {
        charge_step(&mut self.resources, 0, REVERT_NATIVE_COST)?;
        self.set_return_range(cold)?;
        Err(EvmError::Revert.into())
    }

    // --- block environment ---------------------------------------------------------------

    #[inline(never)]
    pub(crate) fn chainid<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            CHAINID_NATIVE_COST,
        )?;
        self.stack.push_u64(env.system.get_chain_id())
    }

    #[inline(never)]
    pub(crate) fn coinbase<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            COINBASE_NATIVE_COST,
        )?;
        self.stack.push_b160(env.system.get_coinbase())
    }

    #[inline(never)]
    pub(crate) fn timestamp<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            TIMESTAMP_NATIVE_COST,
        )?;
        self.stack.push_u64(env.system.get_timestamp())
    }

    #[inline(never)]
    pub(crate) fn number<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::BASE, NUMBER_NATIVE_COST)?;
        self.stack.push_u64(env.system.get_block_number())
    }

    #[inline(never)]
    pub(crate) fn difficulty<T: Tracer<S>>(
        &mut self,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            DIFFICULTY_NATIVE_COST,
        )?;
        // the mix hash holds prevRandao
        let value = U256::from_be_bytes(
            env.system
                .get_mix_hash()
                .map_err(fatal_exit)?
                .as_u8_array_ref(),
        );
        self.stack.push(&value)
    }

    #[inline(never)]
    pub(crate) fn gaslimit<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(&mut self.resources, gas_constants::BASE, GAS_NATIVE_COST)?;
        self.stack.push_u64(env.system.get_gas_limit())
    }

    #[inline(never)]
    pub(crate) fn gasprice<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            GASPRICE_NATIVE_COST,
        )?;
        let price = U256::from(env.system.get_gas_price());
        self.stack.push(&price)
    }

    #[inline(never)]
    pub(crate) fn basefee<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            BASEFEE_NATIVE_COST,
        )?;
        let fee = U256::from(env.system.get_eip1559_basefee());
        self.stack.push(&fee)
    }

    #[inline(never)]
    pub(crate) fn origin<T: Tracer<S>>(
        &mut self,
        cold: &ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult {
        #[cfg(feature = "eip-7645")]
        {
            let _ = env;
            charge_step(&mut self.resources, gas_constants::BASE, CALLER_NATIVE_COST)?;
            self.stack.push_b160(cold.caller)
        }
        #[cfg(not(feature = "eip-7645"))]
        {
            let _ = cold;
            charge_step(&mut self.resources, gas_constants::BASE, ORIGIN_NATIVE_COST)?;
            self.stack.push_b160(env.system.get_tx_origin())
        }
    }

    #[inline(never)]
    pub(crate) fn blockhash<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BLOCKHASH,
            BLOCKHASH_NATIVE_COST,
        )?;
        let block_number = self.stack.pop_1()?.to_u64_saturated();
        let block_hash = U256::from_be_bytes(
            env.system
                .get_blockhash(block_number)
                .map_err(fatal_exit)?
                .as_u8_array_ref(),
        );
        self.stack.push(&block_hash)
    }

    #[inline(never)]
    pub(crate) fn blobhash<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BLOBHASH,
            BLOBHASH_NATIVE_COST,
        )?;
        let stack_top = self.stack.top_mut()?;
        match stack_top
            .try_to_usize()
            .and_then(|index| env.system.get_blob_hash(index))
        {
            Some(blob_hash) => *stack_top = U256::from_be_bytes(blob_hash.as_u8_array_ref()),
            None => U256::write_zero(stack_top),
        }
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn blobbasefee<T: Tracer<S>>(
        &mut self,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult {
        charge_step(
            &mut self.resources,
            gas_constants::BASE,
            BLOBBASEFEE_NATIVE_COST,
        )?;
        let fee = U256::from(env.system.get_blob_base_fee_per_gas());
        self.stack.push(&fee)
    }

    // --- accounts ------------------------------------------------------------------------

    #[inline(never)]
    pub(crate) fn balance<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, 0, BALANCE_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let address = stack_top.to_b160();
        let value = env
            .system
            .io
            .get_nominal_token_balance(THIS_EE_TYPE, &mut self.resources, &address)
            .map_err(system_error_exit)?;
        *stack_top = U256::from(value);
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn selfbalance<T: Tracer<S>>(
        &mut self,
        cold: &ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, 0, SELFBALANCE_NATIVE_COST)?;
        let value = env
            .system
            .io
            .get_selfbalance(THIS_EE_TYPE, &mut self.resources, &cold.address)
            .map_err(system_error_exit)?;
        let value = U256::from(value);
        self.stack.push(&value)
    }

    #[inline(never)]
    pub(crate) fn extcodesize<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, 0, EXTCODESIZE_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let address = stack_top.to_b160();
        let value = env
            .system
            .io
            .get_observable_bytecode_size(THIS_EE_TYPE, &mut self.resources, &address)
            .map_err(system_error_exit)?;
        *stack_top = U256::from(value as u64);
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn extcodehash<T: Tracer<S>>(&mut self, env: &mut Env<'_, S, T>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(&mut self.resources, 0, EXTCODEHASH_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let address = stack_top.to_b160();
        let value = env
            .system
            .io
            .get_observable_bytecode_hash(THIS_EE_TYPE, &mut self.resources, &address)
            .map_err(system_error_exit)?;
        // SAFETY: `stack_top` is a slot of the stack
        unsafe {
            U256::write_be_bytes_into_slot(
                (value.as_u8_array_ref()).as_ptr(),
                stack_top as *mut U256,
            )
        };
        Ok(())
    }

    // --- storage -------------------------------------------------------------------------

    #[inline(never)]
    pub(crate) fn storage_read<const TRANSIENT: bool, T: Tracer<S>>(
        &mut self,
        cold: &ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(
            &mut self.resources,
            0,
            if TRANSIENT {
                TLOAD_NATIVE_COST
            } else {
                SLOAD_NATIVE_COST
            },
        )?;
        let stack_head = self.stack.top_mut()?;
        let le: bool = <S::IO as zk_ee::system::IOSubsystem>::STORAGE_SLOTS_LE;
        // the slot receives the value below, so the key conversion may mangle it; with a
        // little-endian model the key is the slot's own bytes
        let mut key = core::mem::MaybeUninit::<Bytes32>::uninit();
        if le {
            // SAFETY: an initialized slot and 32 writable bytes
            unsafe {
                U256::copy_slot_to_bytes(
                    stack_head as *const U256,
                    crate::utils::bytes32_as_mut_array(&mut key).as_mut_ptr(),
                )
            };
        } else {
            stack_head.bytereverse_and_write_le(crate::utils::bytes32_as_mut_array(&mut key));
        }
        // SAFETY: fully written
        let key = unsafe { key.assume_init_ref() };
        let slot = stack_head as *mut U256;
        env.system
            .io
            .storage_read_raw_and_place::<TRANSIENT>(
                THIS_EE_TYPE,
                &mut self.resources,
                &cold.address,
                key,
                |value| {
                    // SAFETY: `slot` is a slot of the stack; the value goes straight from
                    // the cache into it
                    unsafe {
                        if le {
                            U256::copy_bytes_into_slot(value.as_u8_array_ref().as_ptr(), slot)
                        } else {
                            U256::write_be_bytes_into_slot(value.as_u8_array_ref().as_ptr(), slot)
                        }
                    }
                },
            )
            .map_err(system_error_exit)?;
        trace!(env, |t| {
            let mut traced_key = *key;
            if le {
                traced_key.bytereverse();
            }
            t.on_storage_read(
                THIS_EE_TYPE,
                TRANSIENT,
                cold.address,
                traced_key,
                // SAFETY: the slot was just written
                Bytes32::from_array(unsafe { &*slot }.to_be_bytes()),
            )
        });
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn storage_write<const TRANSIENT: bool, T: Tracer<S>>(
        &mut self,
        cold: &ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(
            &mut self.resources,
            0,
            if TRANSIENT {
                TSTORE_NATIVE_COST
            } else {
                SSTORE_NATIVE_COST
            },
        )?;
        if cold.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }
        if !TRANSIENT && self.gas_left() <= CALL_STIPEND {
            return Err(EvmError::InvalidOperandOOG.into());
        }
        let le: bool = <S::IO as zk_ee::system::IOSubsystem>::STORAGE_SLOTS_LE;
        // the popped slots are scratch, so the conversions may mangle them; with a
        // little-endian model the key and the value are the slots' own bytes
        let (index, value) = self.stack.pop_2_mut()?;
        let mut index_bytes = core::mem::MaybeUninit::<Bytes32>::uninit();
        let mut value_bytes = core::mem::MaybeUninit::<Bytes32>::uninit();
        if le {
            // SAFETY: initialized slots and 32 writable bytes each
            unsafe {
                U256::copy_slot_to_bytes(
                    index as *const U256,
                    crate::utils::bytes32_as_mut_array(&mut index_bytes).as_mut_ptr(),
                );
                U256::copy_slot_to_bytes(
                    value as *const U256,
                    crate::utils::bytes32_as_mut_array(&mut value_bytes).as_mut_ptr(),
                );
            }
        } else {
            index.bytereverse_and_write_le(crate::utils::bytes32_as_mut_array(&mut index_bytes));
            value.bytereverse_and_write_le(crate::utils::bytes32_as_mut_array(&mut value_bytes));
        }
        // SAFETY: fully written
        let (index, value) =
            unsafe { (index_bytes.assume_init_ref(), value_bytes.assume_init_ref()) };
        env.system
            .io
            .storage_write_raw::<TRANSIENT>(
                THIS_EE_TYPE,
                &mut self.resources,
                &cold.address,
                index,
                value,
            )
            .map_err(system_error_exit)?;
        trace!(env, |t| {
            let (mut traced_index, mut traced_value) = (*index, *value);
            if le {
                traced_index.bytereverse();
                traced_value.bytereverse();
            }
            t.on_storage_write(
                THIS_EE_TYPE,
                TRANSIENT,
                cold.address,
                traced_index,
                traced_value,
            )
        });
        Ok(())
    }

    // --- events, destruction -------------------------------------------------------------

    #[inline(never)]
    pub(crate) fn log<const N: usize, T: Tracer<S>>(
        &mut self,
        cold: &mut ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        assert!(N <= MAX_EVENT_TOPICS);
        charge_step(&mut self.resources, 0, LOG_NATIVE_COST)?;
        if cold.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }
        let (mem_offset, len) = self.stack.pop_2()?;
        let (mem_offset, len) =
            cast_offset_and_len(mem_offset, len, EvmError::InvalidOperandOOG.into())?;
        let mut topics: arrayvec::ArrayVec<Bytes32, 4> = arrayvec::ArrayVec::new();
        for _ in 0..N {
            let topic_val = self.stack.pop_1()?;
            let mut buf = [0u8; 32];
            topic_val.write_be_bytes_into(&mut buf);
            topics.push(Bytes32::from_array(buf));
        }
        resize_heap(cold, &mut self.resources, mem_offset.saturating_add(len))?;
        let data = &cold.heap[mem_offset..mem_offset + len];
        trace!(env, |t| t.on_event(
            THIS_EE_TYPE,
            &cold.address,
            &topics,
            data
        ));
        env.system
            .emit_event(
                env.hooks,
                ExecutionEnvironmentType::EVM,
                &mut self.resources,
                &cold.address,
                &topics,
                data,
            )
            .map_err(system_error_exit)?;
        Ok(())
    }

    #[inline(never)]
    pub(crate) fn selfdestruct<T: Tracer<S>>(
        &mut self,
        cold: &ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(
            &mut self.resources,
            gas_constants::SELFDESTRUCT,
            SELFDESTRUCT_NATIVE_COST,
        )?;
        if cold.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }
        let beneficiary = self.stack.pop_1()?.to_b160();
        let amount_transferred = env
            .system
            .io
            .mark_for_deconstruction(
                THIS_EE_TYPE,
                &mut self.resources,
                &cold.address,
                &beneficiary,
            )
            .map_err(wrap_error!())
            .map_err(subsystem_error_exit)?;
        #[cfg(not(target_arch = "riscv32"))]
        {
            env.tracer.evm_tracer().on_selfdestruct(
                beneficiary,
                amount_transferred,
                &super::FrameView::from_hot(&*self, cold, &*env.system),
            );
        }
        #[cfg(target_arch = "riscv32")]
        let _ = amount_transferred;
        Err(ExitCode::SelfDestruct)
    }

    // --- calls and deployments -----------------------------------------------------------

    #[inline(never)]
    pub(crate) fn create<const IS_CREATE2: bool, T: Tracer<S>>(
        &mut self,
        cold: &mut ColdFrameParts<'_, S>,
        env: &mut Env<'_, S, T>,
    ) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        charge_step(
            &mut self.resources,
            gas_constants::CREATE,
            if IS_CREATE2 {
                CREATE2_NATIVE_COST
            } else {
                CREATE_NATIVE_COST
            },
        )?;
        if cold.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }
        cold.clear_last_returndata();

        let (value, code_offset, len) = self.stack.pop_3()?;
        let value: ruint::aliases::U256 = ruint::aliases::U256::from_limbs(*value.as_limbs());
        let (code_offset, len) =
            cast_offset_and_len(code_offset, len, EvmError::InvalidOperandOOG.into())?;
        resize_heap(cold, &mut self.resources, code_offset.saturating_add(len))?;

        if len > MAX_INITCODE_SIZE {
            return Err(EvmError::CreateInitcodeSizeLimit.into());
        }
        let cost_per_word = if IS_CREATE2 {
            INITCODE_WORD_COST + SHA3WORD
        } else {
            INITCODE_WORD_COST
        };
        let initcode_cost = cost_per_word * (len as u64).div_ceil(32);
        spend_gas(&mut self.resources, initcode_cost)?;
        // can not overflow: the heap was resized with the same values
        let deployment_code = code_offset..code_offset + len;

        let deployed_address = if IS_CREATE2 {
            let salt = self.stack.pop_1()?;
            V1::<S>::derive_address_for_deployment_create2(
                env.system,
                &mut self.resources,
                salt,
                &cold.address,
                &cold.heap[deployment_code.clone()],
            )
            .map_err(subsystem_error_exit)?
        } else {
            let deployer_nonce = self
                .resources
                .with_infinite_ergs(|inf_resources| {
                    env.system
                        .io
                        .read_nonce(THIS_EE_TYPE, inf_resources, &cold.address)
                })
                .map_err(system_error_exit)?;
            V1::<S>::derive_address_for_deployment_create(
                &mut self.resources,
                &cold.address,
                deployer_nonce,
            )
            .map_err(subsystem_error_exit)?
        };

        // at this preemption point all resources go to the system
        let all_resources = self.resources.take();
        cold.pending_os_request = Some(PendingOsRequest::Create(deployed_address));
        trace!(env, |t| t.evm_tracer().on_create_request(IS_CREATE2));
        cold.pending_call = Some(EVMCallRequest {
            ergs_to_pass: all_resources.ergs(),
            call_value: value,
            destination_address: deployed_address,
            input_data: deployment_code,
            modifier: CallModifier::Constructor,
            full_caller_resources: all_resources,
        });
        Err(ExitCode::ExternalCall)
    }

    #[inline(never)]
    pub(crate) fn call(
        &mut self,
        cold: &mut ColdFrameParts<'_, S>,
        scheme: CallScheme,
    ) -> InstructionResult {
        charge_step(&mut self.resources, 0, CALL_NATIVE_COST)?;
        cold.clear_last_returndata();
        let (gas_to_pass, to) = self.stack.pop_2()?;
        let to: B160 = to.to_b160();
        let gas_to_pass = gas_to_pass.to_u64_saturated();

        let value: ruint::aliases::U256 = match scheme {
            CallScheme::CallCode => {
                let value = self.stack.pop_1()?;
                ruint::aliases::U256::from_limbs(*value.as_limbs())
            }
            CallScheme::Call => {
                let value = self.stack.pop_1()?;
                if cold.is_static && !value.is_zero() {
                    return Err(EvmError::CallNotAllowedInsideStatic.into());
                }
                ruint::aliases::U256::from_limbs(*value.as_limbs())
            }
            CallScheme::DelegateCall => {
                ruint::aliases::U256::from_limbs(*cold.call_value.as_limbs())
            }
            CallScheme::StaticCall => ruint::aliases::U256::ZERO,
        };

        let (in_offset, in_len, out_offset, out_len) = self.stack.pop_4()?;
        let (in_offset, in_len) =
            cast_offset_and_len(in_offset, in_len, EvmError::InvalidOperandOOG.into())?;
        let (out_offset, out_len) =
            cast_offset_and_len(out_offset, out_len, EvmError::InvalidOperandOOG.into())?;
        resize_heap(cold, &mut self.resources, in_offset.saturating_add(in_len))?;
        resize_heap(
            cold,
            &mut self.resources,
            out_offset.saturating_add(out_len),
        )?;
        let calldata = in_offset..(in_offset + in_len);

        let is_static = matches!(scheme, CallScheme::StaticCall) || cold.is_static;
        let call_modifier = if is_static {
            match scheme {
                CallScheme::DelegateCall => CallModifier::DelegateStatic,
                CallScheme::CallCode => CallModifier::EVMCallcodeStatic,
                _ => CallModifier::Static,
            }
        } else {
            match scheme {
                CallScheme::Call => CallModifier::NoModifier,
                CallScheme::DelegateCall => CallModifier::Delegate,
                CallScheme::CallCode => CallModifier::EVMCallcode,
                // SAFETY: a static call sets `is_static`
                CallScheme::StaticCall => unsafe { unreachable_unchecked() },
            }
        };

        // where the returndata gets copied to
        cold.returndata_location = out_offset..(out_offset + out_len);
        cold.pending_os_request = Some(PendingOsRequest::Call);
        // at this preemption point all resources go to the system
        let all_resources = self.resources.take();
        cold.pending_call = Some(EVMCallRequest {
            ergs_to_pass: <S::Resources as Resources>::Ergs::from_legacy_gas_saturating(
                gas_to_pass,
            ),
            call_value: value,
            destination_address: to,
            input_data: calldata,
            modifier: call_modifier,
            full_caller_resources: all_resources,
        });
        Err(ExitCode::ExternalCall)
    }
}
