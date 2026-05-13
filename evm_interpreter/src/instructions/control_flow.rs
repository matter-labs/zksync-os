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
    pub fn jump(&mut self) -> InstructionResult {
        let (gas_cost, native_cost) = if INLINE_JUMPDEST {
            (
                gas_constants::MID + gas_constants::JUMPDEST,
                JUMP_NATIVE_COST + JUMPDEST_NATIVE_COST,
            )
        } else {
            (gas_constants::MID, JUMP_NATIVE_COST)
        };
        self.gas.spend_gas_and_native(gas_cost, native_cost)?;
        let dest = self.stack.pop_1()?;
        let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
        if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
            // Advance past the JUMPDEST byte on RISC-V so the dispatcher
            // doesn't run it again; on host builds, land on `dest` so the
            // standard JUMPDEST handler fires and the opcode-stats tracer
            // observes a real JUMPDEST event.
            self.instruction_pointer = if INLINE_JUMPDEST { dest + 1 } else { dest };
            if INLINE_JUMPDEST {
                // Emit a synthetic cycle_marker pair so the proving-side
                // marker count balances the host-side LABELS Vec, which
                // still pushes a JUMPDEST entry from the (unoptimized)
                // forward dispatch. Per-opcode cycle attribution between
                // JUMP and JUMPDEST gets scrambled because this pair is
                // nested inside the dispatcher's JUMP bracket, but the
                // block-level effective-cycle total is unaffected.
                cycle_marker::opcode_start!();
                cycle_marker::opcode_end!("JUMPDEST");
            }
            Ok(())
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
                if INLINE_JUMPDEST {
                    // Charged separately from the JUMPI base so the
                    // not-taken path doesn't pay JUMPDEST gas (the OOG
                    // boundary for marginal-gas frames must not shift).
                    self.gas
                        .spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
                    self.instruction_pointer = dest + 1;
                    cycle_marker::opcode_start!();
                    cycle_marker::opcode_end!("JUMPDEST");
                } else {
                    self.instruction_pointer = dest;
                }
            } else {
                return Err(EvmError::InvalidJump.into());
            }
        }
        Ok(())
    }

    pub fn jumpdest(&mut self) -> InstructionResult {
        // On host builds this runs for every JUMP/JUMPI target. On RISC-V
        // builds it only runs for fall-through cases (e.g. JUMPI condition
        // false landing on a JUMPDEST byte).
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
