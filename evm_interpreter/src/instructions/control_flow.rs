use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn jump(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::MID, JUMP_NATIVE_COST)?;
        let dest = self.stack.pop_1()?;
        let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
        if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
            self.instruction_pointer = dest;
            Ok(())
        } else {
            Err(EvmError::InvalidJump.into())
        }
    }

    pub fn jumpi(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        let (dest, value) = self.stack.pop_2()?;
        if *value != U256::ZERO {
            let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
            if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
                self.instruction_pointer = dest;
            } else {
                return Err(EvmError::InvalidJump.into());
            }
        }
        Ok(())
    }

    pub fn jumpdest(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
        Ok(())
    }

    pub fn pc(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, PC_NATIVE_COST)?;
        self.stack.push(&U256::from(self.instruction_pointer - 1))?;
        Ok(())
    }

    pub fn ret(&mut self) -> InstructionResult {
        self.gas.spend_gas_and_native(0, RETURN_NATIVE_COST)?;
        let (offset, len) = self.stack.pop_2()?;
        let len = Self::cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = Self::cast_to_usize(&offset, EvmError::InvalidOperandOOG.into())?;
            self.resize_heap(offset, len)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(EvmError::InvalidOperandOOG.into());
            }
            self.returndata_location = offset..end;
        }
        Err(ExitCode::Return)
    }

    pub fn revert(&mut self) -> InstructionResult {
        self.gas.spend_gas_and_native(0, REVERT_NATIVE_COST)?;
        let (offset, len) = self.stack.pop_2()?;
        let len = Self::cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = Self::cast_to_usize(&offset, EvmError::InvalidOperandOOG.into())?;
            self.resize_heap(offset, len)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(EvmError::InvalidOperandOOG.into());
            }
            self.returndata_location = offset..end;
        }
        Err(EvmError::Revert.into())
    }
}
