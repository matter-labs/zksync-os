// Reference implementation of call tracer. Not feature complete for production

// TODO: SELFDESTRUCT should be tracked as well

use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::{B160, U256};
use zk_ee::system::{
    tracer::{evm_state_for_tracer::EvmFrameForTracer, Tracer},
    CallModifier, CallOrDeployResultRef, EthereumLikeTypes, ExecutionEnvironmentLaunchParams,
    Resources, SystemTypes,
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
    ZKVMSystem,
    ZKVMSystemStatic,
    Selfdestruct,
}

impl From<CallModifier> for CallType {
    fn from(value: CallModifier) -> Self {
        match value {
            CallModifier::Constructor => CallType::Constructor,
            CallModifier::NoModifier => CallType::Call,
            CallModifier::Delegate => CallType::Delegate,
            CallModifier::Static => CallType::Static,
            CallModifier::DelegateStatic => CallType::DelegateStatic,
            CallModifier::EVMCallcode => CallType::EVMCallcode,
            CallModifier::EVMCallcodeStatic => CallType::EVMCallcodeStatic,
            CallModifier::ZKVMSystem => CallType::ZKVMSystem,
            CallModifier::ZKVMSystemStatic => CallType::ZKVMSystemStatic,
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
    error: Option<String>,
    reverted: bool,
    calls: Vec<Call>,
}

#[derive(Default)]
pub struct CallTracer {
    pub transactions: Vec<Call>,
    pub unfinished_calls: Vec<Call>,
    pub finished_calls: Vec<Call>,
    pub current_call_depth: usize,
}

impl<S: EthereumLikeTypes> Tracer<S> for CallTracer {
    #[inline(always)]
    fn should_call_before_evm_execution_step(&self) -> bool {
        false
    }

    #[inline(always)]
    fn should_call_after_evm_execution_step(&self) -> bool {
        false
    }

    #[inline(always)]
    fn should_call_on_new_execution_frame(&self) -> bool {
        true
    }

    #[inline(always)]
    fn should_call_after_execution_frame_completed(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_on_storage_read_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_on_storage_write_enabled(&self) -> bool {
        false
    }

    fn before_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: EvmFrameForTracer<S>,
    ) {
    }

    fn after_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: EvmFrameForTracer<S>,
    ) {
    }

    fn on_new_execution_frame(&mut self, initial_state: &ExecutionEnvironmentLaunchParams<S>) {
        self.current_call_depth += 1;

        self.unfinished_calls.push(Call {
            call_type: CallType::from(initial_state.external_call.modifier),
            from: initial_state.external_call.caller,
            to: initial_state.external_call.callee,
            value: initial_state.external_call.nominal_token_value,
            gas: initial_state.external_call.available_resources.ergs().0 / ERGS_PER_GAS,
            gas_used: 0, // will be populated later
            input: initial_state.external_call.calldata.to_vec(),
            output: vec![], // will be populated later
            error: None,
            reverted: false, // will be populated later
            calls: vec![],   // will be populated later
        })
    }

    fn after_execution_frame_completed(
        &mut self,
        result: Option<(&S::Resources, CallOrDeployResultRef<S>)>,
    ) {
        assert_ne!(self.current_call_depth, 0);
        self.current_call_depth -= 1;

        let mut finished_call = self.unfinished_calls.pop().expect("Should exist");

        match result {
            Some(result) => {
                finished_call.gas_used = finished_call
                    .gas
                    .saturating_sub(result.0.ergs().0 / ERGS_PER_GAS);

                match &result.1 {
                    CallOrDeployResultRef::CallResult(call_result) => match call_result {
                        zk_ee::system::CallResult::CallFailedToExecute => {
                            finished_call.reverted = true;
                            finished_call.error =
                                Some("Unexpected failure before tx execution".to_owned());
                        }
                        zk_ee::system::CallResult::Failed { return_values } => {
                            finished_call.reverted = true;
                            finished_call.output = return_values.returndata.to_vec();
                        }
                        zk_ee::system::CallResult::Successful { return_values } => {
                            finished_call.output = return_values.returndata.to_vec();
                        }
                    },
                    CallOrDeployResultRef::DeploymentResult(deployment_result) => {
                        match deployment_result {
                            zk_ee::system::DeploymentResult::Failed {
                                return_values,
                                execution_reverted: _,
                            } => {
                                finished_call.reverted = true;
                                finished_call.output = return_values.returndata.to_vec();
                            }
                            zk_ee::system::DeploymentResult::Successful {
                                deployed_code: _,
                                return_values: _,
                                deployed_at: _,
                            } => {}
                        }
                    }
                }
            }
            None => {
                // Some unexpected internal failure happened
                // Should revert whole tx
                finished_call.gas_used = finished_call.gas;
                finished_call.reverted = true;
                finished_call.error = Some("Internal error".to_owned()); // TODO we could return better errors here
            }
        }

        self.finished_calls.push(finished_call);
    }

    fn begin_tx(&mut self) {
        self.current_call_depth = 0;
    }

    fn finish_tx(&mut self) {
        assert_eq!(self.current_call_depth, 0);
        assert!(self.unfinished_calls.is_empty());
        assert_eq!(self.finished_calls.len(), 1);

        self.transactions
            .push(self.finished_calls.pop().expect("Should exist"));
    }

    fn on_storage_read(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    fn on_storage_write(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }
}
