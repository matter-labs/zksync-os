use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn jump(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::MID, JUMP_NATIVE_COST)?;
        let [dest] = self.stack.pop_values::<1>()?;
        let dest = self.cast_to_usize(&dest, ExitCode::InvalidJump)?;
        if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
            self.instruction_pointer = dest;
            Ok(())
        } else {
            Err(ExitCode::InvalidJump)
        }
    }

    pub fn jumpi(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        let [dest, value] = self.stack.pop_values::<2>()?;
        if value != U256::ZERO {
            let dest = self.cast_to_usize(&dest, ExitCode::InvalidJump)?;
            if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
                self.instruction_pointer = dest;
            } else {
                return Err(ExitCode::InvalidJump);
            }
        }
        Ok(())
    }

    pub fn jumpdest(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
        Ok(())
    }

    pub fn pc(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, PC_NATIVE_COST)?;
        self.stack
            .push_values(&[U256::from(self.instruction_pointer - 1)])?;
        Ok(())
    }

    pub fn ret(&mut self) -> InstructionResult {
        self.spend_gas_and_native(0, RETURN_NATIVE_COST)?;
        let [offset, len] = self.stack.pop_values::<2>()?;
        let len = self.cast_to_usize(&len, ExitCode::InvalidOperandOOG)?;
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = self.cast_to_usize(&offset, ExitCode::InvalidOperandOOG)?;
            self.resize_heap(offset, len)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(ExitCode::InvalidOperandOOG);
            }
            self.returndata_location = offset..end;
        }
        Err(ExitCode::Return)
    }

    pub fn revert(&mut self) -> InstructionResult {
        self.spend_gas_and_native(0, REVERT_NATIVE_COST)?;
        let [offset, len] = self.stack.pop_values::<2>()?;
        let len = self.cast_to_usize(&len, ExitCode::InvalidOperandOOG)?;
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = self.cast_to_usize(&offset, ExitCode::InvalidOperandOOG)?;
            self.resize_heap(offset, len)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(ExitCode::InvalidOperandOOG);
            }
            self.returndata_location = offset..end;
        }
        Err(ExitCode::Revert)
    }
}
