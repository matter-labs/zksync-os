use super::*;
use native_resource_constants::*;
use zk_ee::system::System;

impl<S: EthereumLikeTypes> Interpreter<S> {
    pub fn mload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, MLOAD_NATIVE_COST)?;
        let index = self.stack.pop_1()?;
        let index = Self::cast_to_usize(&index, ExitCode::InvalidOperandOOG)?;
        self.resize_heap(index, 32, system)?;
        let value = unsafe {
            // we resized enough, so we can read as-if it's a pointer to array
            let src = self.heap().as_ptr().add(index);
            let value = U256::from_be_bytes(src.cast::<[u8; 32]>().as_ref_unchecked());

            value
        };

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " offset: {}, read value: 0x{:0x}",
                index, value
            ));
        }

        self.stack.push_unchecked(&value);

        Ok(())
    }

    pub fn mstore(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, MSTORE_NATIVE_COST)?;
        let (index, value) = self.stack.pop_2()?;
        let mut le_value = value.clone();
        let index = Self::cast_to_usize(&index, ExitCode::InvalidOperandOOG)?;
        self.resize_heap(index, 32, system)?;

        unsafe {
            le_value.bytereverse();
            let src = le_value.as_le_bytes_ref().as_ptr();
            let dst = self.heap().as_mut_ptr().add(index);
            core::ptr::copy_nonoverlapping(src, dst, 32);
        }

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " offset: {}, stored bytes: 0x{:0x}",
                index, le_value
            ));
        }

        Ok(())
    }

    pub fn mstore8(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, MSTORE8_NATIVE_COST)?;
        let (index, value) = self.stack.pop_2()?;
        let index = Self::cast_to_usize(&index, ExitCode::InvalidOperandOOG)?;
        let value = value.byte(0);
        self.resize_heap(index, 1, system)?;
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
        self.stack.push_1(&U256::from(len as u64))
    }

    pub fn mcopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let (dst_offset, src_offset, len) = self.stack.pop_3()?;

        let len = Self::cast_to_usize(len, ExitCode::InvalidOperandOOG)?;
        let dst_offset = Self::cast_to_usize(&dst_offset, ExitCode::InvalidOperandOOG)?;
        let src_offset = Self::cast_to_usize(&src_offset, ExitCode::InvalidOperandOOG)?;

        let (gas_cost, native_cost) = self.very_low_copy_cost(len as u64)?;
        self.spend_gas_and_native(gas_cost, native_cost)?;

        if len == 0 {
            return Ok(());
        }

        self.resize_heap(core::cmp::max(dst_offset, src_offset), len, system)?;
        unsafe {
            let src_ptr = self.heap().as_ptr().add(src_offset);
            let dst_ptr = self.heap().as_mut_ptr().add(dst_offset);
            // Potentially overlapping
            core::ptr::copy(src_ptr, dst_ptr, len);
        }

        Ok(())
    }
}
