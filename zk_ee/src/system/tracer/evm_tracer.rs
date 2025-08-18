use crate::system::{
    evm::{errors::EvmError, EvmFrameInterface},
    SystemTypes,
};

pub trait EvmTracer<S: SystemTypes> {
    /// Called before opcode execution
    /// EE provides an access to EVM frame state, but it is not possible to read global state (storage etc) now
    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    );

    /// Called after opcode execution
    /// EE provides an access to EVM frame state, but it is not possible to read global state (storage etc) now
    fn after_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    );

    /// Called if some failure happens during opcode execution
    fn on_opcode_error(&mut self, error: &EvmError, frame_state: &impl EvmFrameInterface<S>);

    /// Called if some call-specific failure happened
    /// Note: unfortunately we can't provide frame state here by design (frame technically doesn't exist yet)
    fn on_call_error(&mut self, error: &EvmError);
}

#[derive(Default)]
pub struct NopEvmTracer;

impl<S: SystemTypes> EvmTracer<S> for NopEvmTracer {
    #[inline(always)]
    fn before_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn on_opcode_error(&mut self, _error: &EvmError, _frame_state: &impl EvmFrameInterface<S>) {}

    #[inline(always)]
    fn on_call_error(&mut self, _error: &EvmError) {}
}
