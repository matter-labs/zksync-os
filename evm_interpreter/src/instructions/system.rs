// Adapted from https://github.com/bluealloy/revm/blob/main/crates/interpreter/src/instructions/system.rs

use crate::gas::gas_utils;

use super::*;
use native_resource_constants::*;
use zk_ee::memory::U256Builder;
use zk_ee::system::{EthereumLikeTypes, SystemFunctions};

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    const EMPTY_SLICE_SHA3: U256 = U256::from_limbs([
        0x7bfad8045d85a470,
        0xe500b653ca82273b,
        0x927e7db2dcc703c0,
        0xc5d2460186f7233c,
    ]);

    pub fn sha3(&mut self, system: &mut System<S>) -> InstructionResult {
        // Wrap the whole dispatch — including the early stack/length/base-cost
        // checks that may short-circuit via `?` — so the marker fires once per
        // SHA3 opcode dispatch, matching `EvmOpcodeStatsTracer`'s per-dispatch
        // sample count. Positional pairing in `cycles_per_native_report.py` /
        // `join_precompile_samples.py` relies on this 1:1 correspondence. The
        // inner `"keccak"` marker (from `Keccak256Impl::execute`) still fires
        // for the system-function call, so bootloader/intrinsic keccak
        // invocations remain attributed to `"keccak"` alone.
        //
        // `wrap!` (markers only) is used rather than `wrap_with_resources!`:
        // SHA3 gas/native are already captured by `EvmOpcodeStatsTracer`, so
        // the per-call resource diff would be redundant.
        cycle_marker::wrap!("keccak_execution_environment", {
            let (memory_offset, len) = self.stack.pop_2()?;
            self.gas.spend_gas_and_native(0, KECCAK256_NATIVE_COST)?;
            let len = Self::cast_to_usize(&len, EvmError::InvalidOperandOOG.into())?;

            // Eagerly cast `memory_offset` to an owned `usize` so the
            // `&memory_offset` borrow on `self.stack` ends here and does not
            // collide with the final `self.stack.push(&hash)` below.
            let memory_offset_usize: Option<usize> = if len > 0 {
                Some(Self::cast_to_usize(
                    &memory_offset,
                    EvmError::InvalidOperandOOG.into(),
                )?)
            } else {
                None
            };

            let hash = match memory_offset_usize {
                None => {
                    self.gas.spend_gas(gas_constants::SHA3)?;
                    Self::EMPTY_SLICE_SHA3
                }
                Some(memory_offset) => {
                    self.resize_heap(memory_offset, len)?;

                    let allocator = system.get_allocator();
                    let input = &self.heap[memory_offset..(memory_offset + len)];

                    let mut dst = U256Builder::default();
                    S::SystemFunctions::keccak256(
                        &input,
                        &mut dst,
                        self.gas.resources_mut(),
                        allocator,
                    )
                    .map_err(SystemError::from)?;

                    let hash_ruint = dst.build();

                    if Self::PRINT_OPCODES {
                        use core::fmt::Write;
                        use zk_ee::logger_log;
                        use zk_ee::system::logger::Logger;
                        let mut logger = system.get_logger();
                        let input = &self.heap()[memory_offset..(memory_offset + len)];
                        let input_iter = input.iter().copied();
                        logger_log!(logger, " input: ",);
                        let _ = logger.log_data(input_iter);
                        logger_log!(logger, " -> 0x{hash_ruint:0x}");
                    }

                    // Convert ruint::aliases::U256 to u256::U256
                    U256::from(hash_ruint)
                }
            };

            self.stack.push(&hash)
        })
    }

    pub fn address(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, ADDRESS_NATIVE_COST)?;
        self.stack.push_b160(self.address)
    }

    pub fn caller(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CALLER_NATIVE_COST)?;
        self.stack.push_b160(self.caller)
    }

    pub fn codesize(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CODESIZE_NATIVE_COST)?;
        self.stack
            .push_u64(self.bytecode_preprocessing.original_bytecode_len as u64)
    }

    pub fn codecopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len = Self::cast_to_usize(&len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len as u64)?;
        self.gas
            .spend_gas_and_native(gas_cost, native_cost + CODECOPY_NATIVE_COST)?;
        if len == 0 {
            return Ok(());
        }

        let memory_offset =
            Self::cast_to_usize(&memory_offset, EvmError::InvalidOperandOOG.into())?;
        Self::resize_heap_implementation(&mut self.heap, &mut self.gas, memory_offset, len)?;

        // now follow logic of calldatacopy
        let source = source_offset
            .try_to_usize()
            .and_then(|offset| self.bytecode.get(offset..))
            .unwrap_or(&[]);

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            use zk_ee::system_log;
            system_log!(
                system,
                " len {len}, source offset: {source_offset:?}, dest offset {memory_offset}"
            );
        }

        Ok(())
    }

    pub fn calldataload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, CALLDATALOAD_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let value = match stack_top.try_to_usize() {
            Some(index) => {
                if index < self.calldata.len() {
                    let have_bytes = 32.min(self.calldata.len() - index);
                    let mut bytes = Bytes32::ZERO;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            self.calldata.as_ptr().add(index),
                            bytes.as_u8_array_mut().as_mut_ptr(),
                            have_bytes,
                        )
                    }
                    U256::from_be_bytes(bytes.as_u8_array_ref())
                } else {
                    // virtual zero-pad
                    U256::zero()
                }
            }
            None => {
                // virtual zero-pad
                U256::zero()
            }
        };

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            use zk_ee::system_log;
            system_log!(
                system,
                " offset: {}, read value: 0x{:0x}",
                *stack_top,
                value
            );
        }

        *stack_top = value;

        Ok(())
    }

    pub fn calldatasize(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CALLDATASIZE_NATIVE_COST)?;
        let calldata_len = self.calldata().len();
        self.stack.push_u64(calldata_len as u64)
    }

    pub fn callvalue(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CALLVALUE_NATIVE_COST)?;
        self.stack.push(&self.call_value)
    }

    pub fn calldatacopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len = Self::cast_to_usize(&len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len as u64)?;
        self.gas
            .spend_gas_and_native(gas_cost, CALLDATACOPY_NATIVE_COST + native_cost)?;
        if len == 0 {
            return Ok(());
        }
        let memory_offset =
            Self::cast_to_usize(&memory_offset, EvmError::InvalidOperandOOG.into())?;
        Self::resize_heap_implementation(&mut self.heap, &mut self.gas, memory_offset, len)?;

        let source = &source_offset
            .try_to_usize()
            .and_then(|offset| self.calldata.get(offset..))
            .unwrap_or(&[]);

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            use zk_ee::system_log;
            system_log!(
                system,
                " len {len}, source offset: {source_offset:?}, dest offset {memory_offset}"
            );
        }

        Ok(())
    }

    pub fn returndatasize(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, RETURNDATASIZE_NATIVE_COST)?;
        let returndata_len = self.returndata.len();
        self.stack.push_u64(returndata_len as u64)
    }

    pub fn returndatacopy(&mut self) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len = Self::cast_to_usize(&len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len as u64)?;
        self.gas
            .spend_gas_and_native(gas_cost, RETURNDATACOPY_NATIVE_COST + native_cost)?;
        let source_offset =
            Self::cast_to_usize(&source_offset, EvmError::InvalidOperandOOG.into())?;
        let (end, of) = source_offset.overflowing_add(len);
        let returndata_len = self.returndata.len();
        if of || end > returndata_len {
            return Err(EvmError::ReturnDataOutOfBounds.into());
        }

        if len == 0 {
            return Ok(());
        }

        let memory_offset =
            Self::cast_to_usize(&memory_offset, EvmError::InvalidOperandOOG.into())?;
        self.resize_heap(memory_offset, len)?;

        copy_and_zeropad_nonoverlapping(
            self.returndata.get(source_offset..).unwrap_or(&[]),
            &mut self.heap[memory_offset..memory_offset + len],
        );

        Ok(())
    }

    pub fn gas(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, GAS_NATIVE_COST)?;
        self.stack.push_u64(self.gas.gas_left())
    }
}
