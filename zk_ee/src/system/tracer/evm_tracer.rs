use crate::{
    system::{
        evm::{errors::EvmError, EvmFrameInterface},
        SystemTypes,
    },
    types_config::SystemIOTypesConfig,
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
    ///
    /// Note: for Create/Call opcodes this hook is called BEFORE new execution frame is created.
    /// Due to current design, EVM frame state can be changed after this hook (because of charging for reading callee's account properties).
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

    /// Called during selfdestruct execution
    fn on_selfdestruct(
        &mut self,
        beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        frame_state: &impl EvmFrameInterface<S>,
    );

    /// Called on CREATE/CREATE2 system request.
    /// Hook is called before new execution frame is created.
    /// Note: CREATE/CREATE2 opcode execution can fail after this hook (and call on_opcode_error correspondingly)
    /// Note: top-level deployment won't trigger this hook
    fn on_create_request(&mut self, is_create2: bool);
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

    #[inline(always)]
    fn on_selfdestruct(
        &mut self,
        _beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn on_create_request(&mut self, _is_create2: bool) {}
}
