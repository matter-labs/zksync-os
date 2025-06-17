// Adapted from https://github.com/bluealloy/revm/blob/main/crates/interpreter/src/instructions/system.rs

use super::*;
use native_resource_constants::*;
use zk_ee::memory::U256Builder;
use zk_ee::system::errors::SystemFunctionError;
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

        let memory_offset = Self::cast_to_usize(memory_offset, ExitCode::InvalidOperandOOG)?;
        let len = Self::cast_to_usize(len, ExitCode::InvalidOperandOOG)?;
        let (_, of) = memory_offset.overflowing_add(len);
        if of {
            return Err(ExitCode::MemoryLimitOOG);
        }
        self.spend_gas_and_native(0, KECCAK256_NATIVE_COST)?;

        let hash = if len == 0 {
            self.spend_gas(gas_constants::BASE)?;
            Self::EMPTY_SLICE_SHA3
        } else {
            self.resize_heap(memory_offset, len, system)?;

            let allocator = system.get_allocator();
            let input = &self.heap[memory_offset..(memory_offset + len)];

            let mut dst = U256Builder::default();
            S::SystemFunctions::keccak256(&input, &mut dst, &mut self.resources, allocator)
                .map_err(|e| match e {
                    SystemFunctionError::InvalidInput => todo!(),
                    SystemFunctionError::System(e) => e,
                })?;

            let hash = dst.build();

            if Self::PRINT_OPCODES {
                use core::fmt::Write;
                use zk_ee::system::logger::Logger;
                let mut logger = system.get_logger();
                let input = &self.heap()[memory_offset..(memory_offset + len)];
                let input_iter = input.iter().copied();
                let _ = logger.write_fmt(format_args!(" input: ",));
                let _ = logger.log_data(input_iter);
                let _ = logger.write_fmt(format_args!(" -> 0x{:0x}", hash));
            }

            hash
        };

        self.stack.push_unchecked(&hash);

        Ok(())
    }

    pub fn address(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, ADDRESS_NATIVE_COST)?;
        self.stack_push_one(b160_to_u256(self.address))
    }

    pub fn caller(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, CALLER_NATIVE_COST)?;
        self.stack_push_one(b160_to_u256(self.caller))
    }

    pub fn codesize(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, CODESIZE_NATIVE_COST)?;
        self.stack_push_one(U256::from(
            self.bytecode_preprocessing.original_bytecode_len as u64,
        ))
    }

    pub fn codecopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len = Self::cast_to_usize(len, ExitCode::InvalidOperandOOG)?;
        let maybe_memory_offset = Self::cast_to_usize(memory_offset, ExitCode::InvalidOperandOOG);
        let maybe_src_offset = u256_try_to_usize(source_offset);

        let (gas_cost, native_cost) = self.very_low_copy_cost(len as u64)?;
        self.spend_gas_and_native(gas_cost, native_cost + CODECOPY_NATIVE_COST)?;
        if len == 0 {
            return Ok(());
        }

        let memory_offset = maybe_memory_offset?;
        self.resize_heap(memory_offset, len, system)?;

        // now follow logic of calldatacopy
        let source = maybe_src_offset
            .and_then(|offset| self.bytecode.get(offset..))
            .unwrap_or(&[]);

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " len {}, source offset: {:?}, dest offset {}",
                len,
                maybe_src_offset.unwrap_or(usize::MAX),
                memory_offset
            ));
        }

        Ok(())
    }

    pub fn calldataload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, CALLDATALOAD_NATIVE_COST)?;
        self.spend_gas(gas_constants::VERYLOW)?;
        let t = {
            let index = self.stack.pop_1()?;
            u256_try_to_usize(index)
        };

        let value = match t {
            Some(index) => {
                if index < self.calldata.len() {
                    let have_bytes = 32.min(self.calldata.len() - index);
                    let mut value = U256::zero();
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            self.calldata().as_ptr().add(index),
                            value.as_limbs_mut().as_mut_ptr().cast::<u8>(),
                            have_bytes,
                        )
                    }
                    value.bytereverse();

                    if Self::PRINT_OPCODES {
                        use core::fmt::Write;
                        let _ = system.get_logger().write_fmt(format_args!(
                            " offset: {}, read value: 0x{:0x}",
                            index, &value
                        ));
                    }

                    value
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

        self.stack.push_unchecked(&value);

        Ok(())
    }

    pub fn calldatasize(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, CALLDATASIZE_NATIVE_COST)?;
        let calldata_len = self.calldata().len();
        self.stack.push_1(&U256::from(calldata_len as u64))
    }

    pub fn callvalue(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, CALLVALUE_NATIVE_COST)?;
        self.stack_push_one(self.call_value)
    }

    pub fn calldatacopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len = Self::cast_to_usize(len, ExitCode::InvalidOperandOOG)?;
        let maybe_memory_offset = Self::cast_to_usize(memory_offset, ExitCode::InvalidOperandOOG);
        let maybe_src_offset = u256_try_to_usize(source_offset);

        let (gas_cost, native_cost) = self.very_low_copy_cost(len as u64)?;
        self.spend_gas_and_native(gas_cost, CALLDATACOPY_NATIVE_COST + native_cost)?;
        if len == 0 {
            return Ok(());
        }
        let memory_offset = maybe_memory_offset?;
        self.resize_heap(memory_offset, len, system)?;

        let source = maybe_src_offset
            .and_then(|offset| self.calldata.get(offset..))
            .unwrap_or(&[]);

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " len {}, source offset: {:?}, dest offset {}",
                len,
                maybe_src_offset.unwrap_or(usize::MAX),
                memory_offset
            ));
        }

        Ok(())
    }

    pub fn returndatasize(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, RETURNDATASIZE_NATIVE_COST)?;
        let returndata_len = self.returndata.len();
        self.stack.push_1(&U256::from(returndata_len as u64))
    }

    pub fn returndatacopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let (memory_offset, source_offset, len) = self.stack.pop_3()?;
        let len = Self::cast_to_usize(len, ExitCode::InvalidOperandOOG)?;
        let maybe_memory_offset = Self::cast_to_usize(memory_offset, ExitCode::InvalidOperandOOG);
        let maybe_src_offset = Self::cast_to_usize(source_offset, ExitCode::InvalidOperandOOG);

        let (gas_cost, native_cost) = self.very_low_copy_cost(len as u64)?;
        self.spend_gas_and_native(gas_cost, RETURNDATACOPY_NATIVE_COST + native_cost)?;
        let source_offset = maybe_src_offset?;
        let (end, of) = source_offset.overflowing_add(len);
        let returndata_len = self.returndata().len();
        if of || end > returndata_len {
            return Err(ExitCode::OutOfOffset);
        }

        if len == 0 {
            return Ok(());
        }

        let memory_offset = maybe_memory_offset?;
        self.resize_heap(memory_offset, len, system)?;

        copy_and_zeropad_nonoverlapping(
            self.returndata.get(source_offset..).unwrap_or(&[]),
            &mut self.heap[memory_offset..memory_offset + len],
        );

        Ok(())
    }

    pub fn gas(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, GAS_NATIVE_COST)?;
        self.stack_push_one(U256::from(self.gas_left()))
    }
}
