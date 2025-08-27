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
        let (memory_offset, len) = self.stack.pop_2()?;

        let len = Self::cast_to_usize(&len, EvmError::InvalidOperandOOG.into())?;
        self.gas.spend_gas_and_native(0, KECCAK256_NATIVE_COST)?;

        let hash = if len == 0 {
            self.gas.spend_gas(gas_constants::SHA3)?;
            Self::EMPTY_SLICE_SHA3
        } else {
            let memory_offset =
                Self::cast_to_usize(&memory_offset, EvmError::InvalidOperandOOG.into())?;

            let (_, of) = memory_offset.overflowing_add(len);
            if of {
                return Err(EvmError::MemoryLimitOOG.into());
            }

            self.resize_heap(memory_offset, len)?;

            let allocator = system.get_allocator();
            let input = &self.heap[memory_offset..(memory_offset + len)];

            let mut dst = U256Builder::default();
            S::SystemFunctions::keccak256(&input, &mut dst, self.gas.resources_mut(), allocator)
                .map_err(SystemError::from)?;

            let hash = dst.build();

            if Self::PRINT_OPCODES {
                use core::fmt::Write;
                use zk_ee::system::logger::Logger;
                let mut logger = system.get_logger();
                let input = &self.heap()[memory_offset..(memory_offset + len)];
                let input_iter = input.iter().copied();
                let _ = logger.write_fmt(format_args!(" input: ",));
                let _ = logger.log_data(input_iter);
                let _ = logger.write_fmt(format_args!(" -> 0x{hash:0x}"));
            }

            hash
        };

        self.stack.push(&hash)
    }

    pub fn address(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, ADDRESS_NATIVE_COST)?;
        self.stack.push(&b160_to_u256(self.address))
    }

    pub fn caller(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CALLER_NATIVE_COST)?;
        self.stack.push(&b160_to_u256(self.caller))
    }

    pub fn codesize(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CODESIZE_NATIVE_COST)?;
        self.stack.push(&U256::from(
            self.bytecode_preprocessing.original_bytecode_len as u64,
        ))
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
        let source = u256_try_to_usize(source_offset)
            .and_then(|offset| self.bytecode.get(offset..))
            .unwrap_or(&[]);

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " len {len}, source offset: {source_offset:?}, dest offset {memory_offset}"
            ));
        }

        Ok(())
    }

    pub fn calldataload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, CALLDATALOAD_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let value = match u256_try_to_usize(stack_top) {
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
                    bytes.into_u256_be()
                } else {
                    // virtual zero-pad
                    U256::ZERO
                }
            }
            None => {
                // virtual zero-pad
                U256::ZERO
            }
        };

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " offset: {}, read value: 0x{:0x}",
                *stack_top, value
            ));
        }

        *stack_top = value;

        Ok(())
    }

    pub fn calldatasize(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CALLDATASIZE_NATIVE_COST)?;
        let calldata_len = self.calldata().len();
        self.stack.push(&U256::from(calldata_len))
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

        let source = u256_try_to_usize(&source_offset)
            .and_then(|offset| self.calldata.get(offset..))
            .unwrap_or(&[]);

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " len {len}, source offset: {source_offset:?}, dest offset {memory_offset}"
            ));
        }

        Ok(())
    }

    pub fn returndatasize(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, RETURNDATASIZE_NATIVE_COST)?;
        let returndata_len = self.returndata.len();
        self.stack.push(&U256::from(returndata_len))
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
        self.stack.push(&U256::from(self.gas.gas_left()))
    }
}
