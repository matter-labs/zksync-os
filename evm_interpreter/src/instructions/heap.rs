use super::*;
use native_resource_constants::*;
use zk_ee::system::System;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn mload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, MLOAD_NATIVE_COST)?;
        let [index] = self.pop_values::<1>()?;
        let index = self.cast_to_usize(&index, ExitCode::InvalidOperandOOG)?;
        self.resize_heap(index, 32)?;
        let mut value = U256::ZERO;
        unsafe {
            let src = self.heap().as_ptr().add(index);
            let dst = value.as_le_slice_mut().as_mut_ptr();
            core::ptr::copy_nonoverlapping(src, dst, 32);
            crate::utils::bytereverse_u256(&mut value);
        }

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " offset: {}, read value: 0x{:0x}",
                index, value
            ));
        }

        self.stack_push_one(value)
    }

    pub fn mstore(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, MSTORE_NATIVE_COST)?;
        let [index, value] = self.pop_values::<2>()?;
        let index = self.cast_to_usize(&index, ExitCode::InvalidOperandOOG)?;
        self.resize_heap(index, 32)?;

        unsafe {
            let mut le_value = value;
            crate::utils::bytereverse_u256(&mut le_value);
            let src = le_value.as_le_slice().as_ptr();
            let dst = self.heap().as_mut_ptr().add(index);
            core::ptr::copy_nonoverlapping(src, dst, 32);
        }

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " offset: {}, stored value: 0x{:0x}",
                index, value
            ));
        }

        Ok(())
    }

    pub fn mstore8(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, MSTORE8_NATIVE_COST)?;
        let [index, value] = self.pop_values::<2>()?;
        let index = self.cast_to_usize(&index, ExitCode::InvalidOperandOOG)?;
        self.resize_heap(index, 1)?;
        let value = value.byte(0);
        self.heap()[index] = value;

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " offset: {}, stored byte: 0x{:0x}",
                index, value
            ));
        }

        Ok(())
    }

    pub fn msize(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, MSIZE_NATIVE_COST)?;
        let len = self.memory_len();
        debug_assert!(len.next_multiple_of(32) == len);
        self.stack_push_one(U256::from(len))
    }

    pub fn mcopy(&mut self) -> InstructionResult {
        let [dst_offset, src_offset, len] = self.pop_values::<3>()?;

        let len = self.cast_to_usize(&len, ExitCode::InvalidOperandOOG)?;
        let (gas_cost, native_cost) = self.very_low_copy_cost(len as u64)?;
        self.spend_gas_and_native(gas_cost, native_cost)?;

        if len == 0 {
            return Ok(());
        }

        let dst_offset = self.cast_to_usize(&dst_offset, ExitCode::InvalidOperandOOG)?;
        let src_offset = self.cast_to_usize(&src_offset, ExitCode::InvalidOperandOOG)?;
        self.resize_heap(core::cmp::max(dst_offset, src_offset), len)?;
        unsafe {
            let src_ptr = self.heap().as_ptr().add(src_offset);
            let dst_ptr = self.heap().as_mut_ptr().add(dst_offset);
            // Potentially overlapping
            core::ptr::copy(src_ptr, dst_ptr, len);
        }

        Ok(())
    }
}
