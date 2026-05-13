use super::*;
use crate::InterpreterExternal;
use native_resource_constants::*;
use zk_ee::system::tracer::evm_tracer::EvmTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::System;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    pub fn jump(
        &mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
        cycles: &mut u64,
    ) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::MID, JUMP_NATIVE_COST)?;
        let dest = self.stack.pop_1()?;
        let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
        if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
            // `is_valid_jumpdest` already guarantees `bytecode[dest] == JUMPDEST`.
            // Skip its dispatch iteration but emit synthetic before/after
            // tracer hooks around its gas charge so any tracer that keys on
            // JUMPDEST events still sees them. Also bump the dispatch-loop
            // `cycles` counter so the "Instructions executed = N" log stays
            // consistent with the number of opcodes actually accounted for.
            self.instruction_pointer = dest;
            tracer.evm_tracer().before_evm_interpreter_execution_step(
                opcodes::JUMPDEST,
                &InterpreterExternal::new_from(&*self, system),
            );
            self.instruction_pointer = dest + 1;
            self.gas
                .spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
            tracer.evm_tracer().after_evm_interpreter_execution_step(
                opcodes::JUMPDEST,
                &InterpreterExternal::new_from(&*self, system),
            );
            *cycles += 1;
            Ok(())
        } else {
            Err(EvmError::InvalidJump.into())
        }
    }

    pub fn jumpi(
        &mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
        cycles: &mut u64,
    ) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        let (dest, value) = self.stack.pop_2()?;
        if !value.is_zero() {
            let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
            if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
                // Same JUMPDEST-skip optimization as JUMP, with synthetic
                // before/after hooks around the JUMPDEST gas charge and a
                // `cycles` bump for the synthetic iteration.
                self.instruction_pointer = dest;
                tracer.evm_tracer().before_evm_interpreter_execution_step(
                    opcodes::JUMPDEST,
                    &InterpreterExternal::new_from(&*self, system),
                );
                self.instruction_pointer = dest + 1;
                self.gas
                    .spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
                tracer.evm_tracer().after_evm_interpreter_execution_step(
                    opcodes::JUMPDEST,
                    &InterpreterExternal::new_from(&*self, system),
                );
                *cycles += 1;
            } else {
                return Err(EvmError::InvalidJump.into());
            }
        }
        Ok(())
    }

    pub fn jumpdest(&mut self) -> InstructionResult {
        // Reached only via fall-through (e.g. JUMPI condition false landing on
        // a JUMPDEST byte). JUMP and JUMPI inline this path for their own
        // targets, so this is exercised less often than before but still
        // required for correctness.
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
