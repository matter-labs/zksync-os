// Reference implementation of call tracer.

use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::{B160, U256};
use zk_ee::system::{
    evm::{errors::EvmError, EvmFrameInterface},
    tracer::{evm_tracer::EvmTracer, Tracer},
    CallModifier, CallResult, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, Resources,
    SystemTypes,
};
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::Bytes32;

#[derive(Default, Debug)]
pub enum CallType {
    #[default]
    Call,
    Create,
    Create2,
    Delegate,
    Static,
    DelegateStatic,
    EVMCallcode,
    EVMCallcodeStatic,
    ZKVMSystem,       // Not used
    ZKVMSystemStatic, // Not used
    Selfdestruct,
}

impl CallType {
    fn from(value: CallModifier, is_create: &Option<CreateType>) -> Self {
        // Note: in our implementation Selfdestruct isn't actually implemented as a "call". But in traces it should be treated like one
        match value {
            CallModifier::NoModifier => CallType::Call,
            CallModifier::Delegate => CallType::Delegate,
            CallModifier::Static => CallType::Static,
            CallModifier::DelegateStatic => CallType::DelegateStatic,
            CallModifier::EVMCallcode => CallType::EVMCallcode,
            CallModifier::EVMCallcodeStatic => CallType::EVMCallcodeStatic,
            CallModifier::ZKVMSystem => CallType::ZKVMSystem,
            CallModifier::ZKVMSystemStatic => CallType::ZKVMSystemStatic,
            CallModifier::Constructor => match is_create.as_ref().expect("Should exist") {
                CreateType::Create => CallType::Create,
                CreateType::Create2 => CallType::Create2,
            },
        }
    }
}

#[derive(Default, Debug)]
pub struct Call {
    pub call_type: CallType,
    pub from: B160,
    pub to: B160,
    pub value: U256,
    pub gas: u64,
    pub gas_used: u64,
    pub input: Vec<u8>,
    pub output: Vec<u8>,
    pub error: Option<CallError>,
    pub reverted: bool,
    pub calls: Vec<Call>,
    pub logs: Vec<CallLogFrame>,
}

#[derive(Default, Debug)]
pub struct CallLogFrame {
    pub address: B160,
    pub topics: Vec<Bytes32>,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum CallError {
    EvmError(EvmError),
    FatalError(String), // Some fatal internal error outside of EVM specification (ZKsync OS specific)
}

#[derive(Debug)]
pub enum CreateType {
    Create,
    Create2,
}

#[derive(Default)]
pub struct CallTracer {
    pub transactions: Vec<Call>,
    pub unfinished_calls: Vec<Call>,
    pub finished_calls: Vec<Call>,
    pub current_call_depth: usize,
    pub collect_logs: bool,
    pub only_top_call: bool,

    create_operation_requested: Option<CreateType>,
}

impl CallTracer {
    pub fn new_with_config(collect_logs: bool, only_top_call: bool) -> Self {
        Self {
            transactions: vec![],
            unfinished_calls: vec![],
            finished_calls: vec![],
            current_call_depth: 0,
            collect_logs,
            only_top_call,
            create_operation_requested: None,
        }
    }
}

impl<S: EthereumLikeTypes> Tracer<S> for CallTracer {
    fn on_new_execution_frame(&mut self, initial_state: &ExecutionEnvironmentLaunchParams<S>) {
        self.current_call_depth += 1;

        if !self.only_top_call || self.current_call_depth == 1 {
            // Top-level deployment (initiated by EOA) won't trigger `on_create_request` hook
            // This is always a CREATE
            if self.current_call_depth == 1
                && initial_state.external_call.modifier == CallModifier::Constructor
            {
                self.create_operation_requested = Some(CreateType::Create);
            }

            let call_type = CallType::from(
                initial_state.external_call.modifier,
                &self.create_operation_requested,
            );

            self.unfinished_calls.push(Call {
                call_type,
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
                logs: vec![],    // will be populated later
            })
        }

        // Reset flag, required data is consumed
        if self.create_operation_requested.is_some() {
            self.create_operation_requested = None;
        }
    }

    fn after_execution_frame_completed(&mut self, result: Option<(&S::Resources, &CallResult<S>)>) {
        assert_ne!(self.current_call_depth, 0);

        if !self.only_top_call || self.current_call_depth == 1 {
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
                            match finished_call.call_type {
                                CallType::Create | CallType::Create2 => {
                                    // output should be already populated in `on_bytecode_change` hook
                                }
                                _ => {
                                    finished_call.output = return_values.returndata.to_vec();
                                }
                            }
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

            if let Some(parent_call) = self.unfinished_calls.last_mut() {
                parent_call.calls.push(finished_call);
            } else {
                self.finished_calls.push(finished_call);
            }
        }

        self.current_call_depth -= 1;

        // Reset flag in case if frame terminated due to out-of-native / other internal ZKsync OS error
        if self.create_operation_requested.is_some() {
            self.create_operation_requested = None;
        }
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {
        self.current_call_depth = 0;

        // Sanity check
        assert!(self.create_operation_requested.is_none());
    }

    fn finish_tx(&mut self) {
        assert_eq!(self.current_call_depth, 0);
        assert!(self.unfinished_calls.is_empty());
        assert_eq!(self.finished_calls.len(), 1);

        // Sanity check
        assert!(self.create_operation_requested.is_none());

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

    fn on_event(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        address: &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        topics: &[<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::EventKey],
        data: &[u8],
    ) {
        if self.collect_logs {
            let call = self.unfinished_calls.last_mut().expect("Should exist");
            call.logs.push(CallLogFrame {
                address: *address,
                topics: topics.to_vec(),
                data: data.to_vec(),
            })
        }
    }

    /// Is called on a change of bytecode for some account.
    /// `new_bytecode` can be None if bytecode is unknown at the moment of change (e.g. force deploy by hash in system hook)
    fn on_bytecode_change(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        new_bytecode: Option<&[u8]>,
        _new_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        new_observable_bytecode_length: u32,
    ) {
        let call = self.unfinished_calls.last_mut().expect("Should exist");

        match call.call_type {
            CallType::Create | CallType::Create2 => {
                assert_eq!(address, call.to);
                let deployed_raw_bytecode = new_bytecode.expect("Should be present");

                assert!(deployed_raw_bytecode.len() >= new_observable_bytecode_length as usize);

                // raw bytecode may include internal artifacts (jumptable), so we need to trim it
                call.output =
                    deployed_raw_bytecode[..new_observable_bytecode_length as usize].to_vec();
            }
            _ => {
                // should not happen now (system hooks currently do not trigger this hook)
            }
        }
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

        // In case we fail after `on_create_request` hook, but before `on_new_execution_frame` hook
        if self.create_operation_requested.is_some() {
            self.create_operation_requested = None;
        }
    }

    /// Special cases, when error happens in frame before any opcode is executed (unfortunately we can't provide access to state)
    /// Note: call frame ends immediately
    fn on_call_error(&mut self, error: &EvmError) {
        let current_call = self.unfinished_calls.last_mut().expect("Should exist");
        current_call.error = Some(CallError::EvmError(error.clone()));
        current_call.reverted = true;

        assert!(self.create_operation_requested.is_none());
    }

    /// We should treat selfdestruct as a special kind of a call
    fn on_selfdestruct(
        &mut self,
        beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        // Following Geth implementation: https://github.com/ethereum/go-ethereum/blob/2dbb580f51b61d7ff78fceb44b06835827704110/core/vm/instructions.go#L894
        let call_frame = Call {
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
            logs: vec![],
        };

        if let Some(parent_call) = self.unfinished_calls.last_mut() {
            parent_call.calls.push(call_frame);
        } else {
            self.finished_calls.push(call_frame);
        }
    }

    /// Called on CREATE/CREATE2 system request.
    /// Hook is called *before* new execution frame is created.
    /// Note: CREATE/CREATE2 opcode execution can fail after this hook (and call on_opcode_error correspondingly)
    /// Note: top-level deployment won't trigger this hook
    fn on_create_request(&mut self, is_create2: bool) {
        // Can't be some - `on_new_execution_frame` or `on_opcode_error` should reset flag
        assert!(self.create_operation_requested.is_none());

        self.create_operation_requested = if is_create2 {
            Some(CreateType::Create)
        } else {
            Some(CreateType::Create2)
        };
    }
}
