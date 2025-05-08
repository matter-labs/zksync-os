use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn jump(&mut self) -> InstructionResult {
<<<<<<< HEAD
        self.spend_gas_and_native(gas_constants::MID, JUMP_NATIVE_COST)?;
        let [dest] = self.pop_values::<1>()?;
        let dest = self.cast_to_usize(&dest, ExitCode::InvalidJump)?;
=======
        self.spend_gas(gas_constants::MID)?;
        let dest = self.stack.pop_1()?;
        let dest = Self::cast_to_usize(&dest, ExitCode::InvalidJump)?;
>>>>>>> try for perf run
        if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
            self.instruction_pointer = dest;
            Ok(())
        } else {
            Err(ExitCode::InvalidJump)
        }
    }

    pub fn jumpi(&mut self) -> InstructionResult {
<<<<<<< HEAD
        self.spend_gas_and_native(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        let [dest, value] = self.pop_values::<2>()?;
        if value != U256::ZERO {
            let dest = self.cast_to_usize(&dest, ExitCode::InvalidJump)?;
=======
        self.spend_gas(gas_constants::HIGH)?;
        let (dest, value) = self.stack.pop_2()?;
        if value != &U256::ZERO {
            let dest = Self::cast_to_usize(&dest, ExitCode::InvalidJump)?;
>>>>>>> try for perf run
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
<<<<<<< HEAD
        self.spend_gas_and_native(gas_constants::BASE, PC_NATIVE_COST)?;
        self.push_values(&[U256::from(self.instruction_pointer - 1)])?;
=======
        self.spend_gas(gas_constants::BASE)?;
        self.stack
            .push_1(&U256::from(self.instruction_pointer - 1))?;
>>>>>>> try for perf run
        Ok(())
    }

    pub fn ret(&mut self, system: &mut System<S>) -> InstructionResult {
<<<<<<< HEAD
        self.spend_gas_and_native(0, RETURN_NATIVE_COST)?;
        let [offset, len] = self.pop_values::<2>()?;
        let len = self.cast_to_usize(&len, ExitCode::InvalidOperandOOG)?;
=======
        // zero gas cost gas!(interp,gas::ZERO);
        let (offset, len) = self.stack.pop_2()?;
        let len = Self::cast_to_usize(&len, ExitCode::InvalidOperandOOG)?;
>>>>>>> try for perf run
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = Self::cast_to_usize(&offset, ExitCode::InvalidOperandOOG)?;
            self.resize_heap(offset, len, system)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(ExitCode::InvalidOperandOOG);
            }
            self.returndata_location = offset..end;
        }
        Err(ExitCode::Return)
    }

    pub fn revert(&mut self, system: &mut System<S>) -> InstructionResult {
<<<<<<< HEAD
        self.spend_gas_and_native(0, REVERT_NATIVE_COST)?;
        let [offset, len] = self.pop_values::<2>()?;
        let len = self.cast_to_usize(&len, ExitCode::InvalidOperandOOG)?;
=======
        let (offset, len) = self.stack.pop_2()?;
        let len = Self::cast_to_usize(&len, ExitCode::InvalidOperandOOG)?;
>>>>>>> try for perf run
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = Self::cast_to_usize(&offset, ExitCode::InvalidOperandOOG)?;
            self.resize_heap(offset, len, system)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(ExitCode::InvalidOperandOOG);
            }
            self.returndata_location = offset..end;
        }
        Err(ExitCode::Revert)
    }
}
