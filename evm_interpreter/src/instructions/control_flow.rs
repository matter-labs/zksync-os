use super::*;
use native_resource_constants::*;

/// `true` when building for the RISC-V proving target. JUMP / JUMPI inline the
/// JUMPDEST gas charge and skip its dispatch iteration only in this build, so
/// the optimization is invisible to host-mode tracers (`EvmOpcodeStatsTracer`,
/// `EvmOpcodesLogger`, etc.), which keep recording per-opcode JUMPDEST events
/// with correct gas/native deltas. The proving target has no live tracer that
/// keys on JUMPDEST, and `cycle_marker` measures the dispatch iteration as a
/// whole, so dropping the iteration is the right thing there.
const INLINE_JUMPDEST: bool = cfg!(target_arch = "riscv32");

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    /// The jump is taken to the valid `dest`
    #[inline(always)]
    fn land_on_jumpdest(&mut self, dest: usize) -> InstructionResult {
        if INLINE_JUMPDEST {
            // Charged separately from the JUMP / JUMPI base, so an invalid destination or the
            // not-taken branch doesn't pay JUMPDEST gas/native: it preserves host parity on the
            // OOG boundary for marginal-gas frames, and on native resources returned after
            // InvalidJump.
            self.gas
                .spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
            self.instruction_pointer = dest + 1;
            // Synthetic cycle_marker pair, so the proving-side marker count balances the
            // host-side one, where JUMPDEST is still dispatched.
            cycle_marker::opcode_start!();
            cycle_marker::opcode_end!("JUMPDEST");
        } else {
            self.instruction_pointer = dest;
        }

        Ok(())
    }

    pub fn jump(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::MID, JUMP_NATIVE_COST)?;
        let dest = self.stack.pop_1()?;
        let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
        if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
            self.land_on_jumpdest(dest)
        } else {
            Err(EvmError::InvalidJump.into())
        }
    }

    pub fn jumpi(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        let (dest, value) = self.stack.pop_2()?;
        if !value.is_zero() {
            let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
            if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
                self.land_on_jumpdest(dest)?;
            } else {
                return Err(EvmError::InvalidJump.into());
            }
        }
        Ok(())
    }

    pub fn jumpdest(&mut self) -> InstructionResult {
        // On host builds this runs for every JUMP/JUMPI target. On RISC-V builds it only runs
        // for fall-through cases (e.g. JUMPI condition false landing on a JUMPDEST byte).
        self.gas
            .spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
        Ok(())
    }

    pub fn pc(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, PC_NATIVE_COST)?;
        self.stack.push_u64((self.instruction_pointer - 1) as u64)?;
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
