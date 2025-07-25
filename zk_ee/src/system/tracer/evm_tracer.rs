use crate::system::{evm::EvmFrameInterface, SystemTypes};

pub trait EvmTracer<S: SystemTypes> {
    fn is_on_evm_execution_step_enabled(&self) -> bool;
    fn is_after_evm_execution_step_enabled(&self) -> bool;

    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    );
    fn after_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    );
}

#[derive(Default)]
pub struct NopEvmTracer;

impl<S: SystemTypes> EvmTracer<S> for NopEvmTracer {
    #[inline(always)]
    fn is_on_evm_execution_step_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_after_evm_execution_step_enabled(&self) -> bool {
        false
    }

    fn before_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
        unreachable!()
    }

    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
        unreachable!()
    }
}
