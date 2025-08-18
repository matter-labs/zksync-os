// Reference implementation of call tracer.

use std::mem;

use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::{B160, U256};
use zk_ee::system::{
    evm::{errors::EvmError, EvmFrameInterface},
    tracer::{evm_tracer::EvmTracer, Tracer},
    CallModifier, CallResult, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, Resources,
    SystemTypes,
};
use zk_ee::types_config::SystemIOTypesConfig;

#[derive(Default, Debug)]
pub enum CallType {
    #[default]
    Call,
    Constructor,
    Delegate,
    Static,
    DelegateStatic,
    EVMCallcode,
    EVMCallcodeStatic,
    ZKVMSystem,       // Not used
    ZKVMSystemStatic, // Not used
    Selfdestruct,
}

impl From<CallModifier> for CallType {
    fn from(value: CallModifier) -> Self {
        // Note: in our implementation Selfdestruct isn't actually implemented as a "call". But in traces it should be treated like one
        match value {
            CallModifier::Constructor => CallType::Constructor,
            CallModifier::NoModifier => CallType::Call,
            CallModifier::Delegate => CallType::Delegate,
            CallModifier::Static => CallType::Static,
            CallModifier::DelegateStatic => CallType::DelegateStatic,
            CallModifier::EVMCallcode => CallType::EVMCallcode,
            CallModifier::EVMCallcodeStatic => CallType::EVMCallcodeStatic,
            CallModifier::ZKVMSystem => CallType::ZKVMSystem, // Not used
            CallModifier::ZKVMSystemStatic => CallType::ZKVMSystemStatic, // Not used
        }
    }
}

#[derive(Default, Debug)]
#[allow(dead_code)]
pub struct Call {
    call_type: CallType,
    from: B160,
    to: B160,
    value: U256,
    gas: u64,
    gas_used: u64,
    input: Vec<u8>,
    output: Vec<u8>,
    error: Option<CallError>,
    reverted: bool,
    calls: Vec<Call>,
}

#[derive(Debug)]
pub enum CallError {
    EvmError(EvmError),
    FatalError(String), // Some fatal internal error outside of EVM specification (ZKsync OS specific)
}

#[derive(Default)]
pub struct CallTracer {
    pub transactions: Vec<Call>,
    pub unfinished_calls: Vec<Call>,
    pub finished_calls: Vec<Call>,
    pub current_call_depth: usize,
}

impl<S: EthereumLikeTypes> Tracer<S> for CallTracer {
    fn on_new_execution_frame(&mut self, initial_state: &ExecutionEnvironmentLaunchParams<S>) {
        self.current_call_depth += 1;

        self.unfinished_calls.push(Call {
            call_type: CallType::from(initial_state.external_call.modifier),
            from: initial_state.external_call.caller,
            to: initial_state.external_call.callee,
            value: initial_state.external_call.nominal_token_value,
            gas: initial_state.external_call.available_resources.ergs().0 / ERGS_PER_GAS,
            gas_used: 0, // will be populated later
            input: initial_state.external_call.input.to_vec(),
            output: vec![],  // will be populated later
            error: None,     // can be populated later
            reverted: false, // will be populated later
            calls: vec![],   // will be populated later
        })
    }

    fn after_execution_frame_completed(&mut self, result: Option<(&S::Resources, &CallResult<S>)>) {
        assert_ne!(self.current_call_depth, 0);
        self.current_call_depth -= 1;

        let mut finished_call = self.unfinished_calls.pop().expect("Should exist");

        match result {
            Some(result) => {
                finished_call.gas_used = finished_call
                    .gas
                    .saturating_sub(result.0.ergs().0 / ERGS_PER_GAS);

                match &result.1 {
                    zk_ee::system::CallResult::PreparationStepFailed => {
                        panic!("Should not happen") // ZKsync OS should not call tracer in this case
                    }
                    zk_ee::system::CallResult::Failed { return_values } => {
                        finished_call.reverted = true;
                        finished_call.output = return_values.returndata.to_vec();
                    }
                    zk_ee::system::CallResult::Successful { return_values } => {
                        finished_call.output = return_values.returndata.to_vec();
                    }
                };
            }
            None => {
                // Some unexpected internal failure happened (maybe out of native resources)
                // Should revert whole tx
                finished_call.gas_used = finished_call.gas;
                finished_call.reverted = true;
                finished_call.error = Some(CallError::FatalError("Internal error".to_owned()));
            }
        }

        finished_call.calls = mem::take(&mut self.finished_calls);

        self.finished_calls.push(finished_call);
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {
        self.current_call_depth = 0;
    }

    fn finish_tx(&mut self) {
        assert_eq!(self.current_call_depth, 0);
        assert!(self.unfinished_calls.is_empty());
        assert_eq!(self.finished_calls.len(), 1);

        self.transactions
            .push(self.finished_calls.pop().expect("Should exist"));
    }

    #[inline(always)]
    fn on_storage_read(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_storage_write(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_event(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _address: &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _topics: &[<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::EventKey],
        _data: &[u8],
    ) {
    }

    #[inline(always)]
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        self
    }
}

impl<S: EthereumLikeTypes> EvmTracer<S> for CallTracer {
    #[inline(always)]
    fn before_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: &impl EvmFrameInterface<S>,
    ) {
    }

    /// Opcode failed for some reason. Note: call frame ends immediately
    fn on_opcode_error(&mut self, error: &EvmError, _frame_state: &impl EvmFrameInterface<S>) {
        let current_call = self.unfinished_calls.last_mut().expect("Should exist");
        current_call.error = Some(CallError::EvmError(error.clone()));
        current_call.reverted = true;
    }

    /// Special cases, when error happens in frame before any opcode is executed (unfortunately we can't provide access to state)
    /// Note: call frame ends immediately
    fn on_call_error(&mut self, error: &EvmError) {
        let current_call = self.unfinished_calls.last_mut().expect("Should exist");
        current_call.error = Some(CallError::EvmError(error.clone()));
        current_call.reverted = true;
    }

    /// We should treat selfdestruct as a special kind of a call
    fn on_selfdestruct(
        &mut self,
        beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        // Following Geth implementation: https://github.com/ethereum/go-ethereum/blob/2dbb580f51b61d7ff78fceb44b06835827704110/core/vm/instructions.go#L894
        self.finished_calls.push(Call {
            call_type: CallType::Selfdestruct,
            from: frame_state.address(),
            to: beneficiary,
            value: token_value,
            gas: 0,
            gas_used: 0,
            input: vec![],
            output: vec![],
            error: None,
            reverted: false,
            calls: vec![],
        })
    }
}
