// Reference implementation of EVM opcodes logger. Not feature complete for production

use std::{collections::HashMap, marker::PhantomData};

use evm_interpreter::{opcodes::OpCode, ERGS_PER_GAS};
use ruint::aliases::U256;
use zk_ee::{
    system::{
        evm::{EvmFrameInterface, EvmStackInterface},
        tracer::{evm_tracer::EvmTracer, Tracer},
        CallResult, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, Resources, SystemTypes,
    },
    types_config::SystemIOTypesConfig,
    utils::Bytes32,
};
use zksync_os_evm_errors::EvmError;

#[derive(Default, Debug)]
#[allow(dead_code)]
pub struct EvmExecutionStep {
    pc: usize,
    opcode_raw: u8,
    opcode: Option<String>,
    gas: u64,
    /// Gas used for opcode execution, None means we can't derive this value
    gas_used: Option<u64>,
    memory: Option<Vec<u8>>,
    mem_size: usize,
    stack: Option<Vec<U256>>,
    return_data: Option<Vec<u8>>,
    storage: Option<Vec<(Bytes32, Bytes32)>>,
    transient_storage: Option<Vec<(Bytes32, Bytes32)>>,
    depth: usize,
    refund: u64,
    error: Option<EvmError>,
}

#[derive(Default, Debug)]
pub struct TransactionLog {
    pub finished: bool,
    pub steps: Vec<EvmExecutionStep>,
}

pub struct EvmOpcodesLogger<S: SystemTypes> {
    pub transaction_logs: Vec<TransactionLog>,
    pub current_call_depth: usize,
    pub steps_counter: usize,

    storage_caches_for_frames: Vec<HashMap<Bytes32, Bytes32>>,
    transient_storage_caches_for_frames: Vec<HashMap<Bytes32, Bytes32>>,

    enable_memory: bool,
    enable_stack: bool,
    enable_returndata: bool,
    enable_storage: bool,
    enable_transient_storage: bool,

    limit: usize,

    // Block of dirty hacks to track gas used by call-like opcodes
    last_known_gas_left: u64,
    gas_used_by_last_call: u64,
    gas_used_by_calls: Vec<u64>,
    /// depth -> (index_of_execution_step, gas_before_execution_step)
    pending_call_opcodes: HashMap<usize, (usize, u64)>,

    _marker: PhantomData<S>,
}

impl<S: SystemTypes> Default for EvmOpcodesLogger<S> {
    fn default() -> Self {
        Self {
            transaction_logs: Default::default(),
            current_call_depth: Default::default(),
            steps_counter: Default::default(),
            storage_caches_for_frames: Default::default(),
            transient_storage_caches_for_frames: Default::default(),
            enable_memory: false,
            enable_stack: true,
            enable_returndata: false,
            enable_storage: true,
            enable_transient_storage: true,

            limit: 0,

            last_known_gas_left: 0,
            gas_used_by_last_call: 0,
            gas_used_by_calls: vec![],
            pending_call_opcodes: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: SystemTypes> EvmOpcodesLogger<S> {
    pub fn new_with_config(
        enable_memory: bool,
        enable_stack: bool,
        enable_returndata: bool,
        enable_storage: bool,
        enable_transient_storage: bool,
        limit: usize,
    ) -> Self {
        Self {
            transaction_logs: Default::default(),
            current_call_depth: Default::default(),
            steps_counter: Default::default(),
            storage_caches_for_frames: Default::default(),
            transient_storage_caches_for_frames: Default::default(),
            enable_memory,
            enable_stack,
            enable_returndata,
            enable_storage,
            enable_transient_storage,
            limit,
            last_known_gas_left: 0,
            gas_used_by_last_call: 0,
            gas_used_by_calls: vec![],
            pending_call_opcodes: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<S: EthereumLikeTypes> EvmTracer<S> for EvmOpcodesLogger<S> {
    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        interpreter_state: &impl EvmFrameInterface<S>,
    ) {
        if self.limit != 0 && self.steps_counter > self.limit {
            return;
        }
        self.steps_counter += 1;

        let tx_log = self.transaction_logs.last_mut().expect("Should exist");

        let opcode_decoded = OpCode::try_from_u8(opcode).map(|x| x.as_str().to_owned());

        let memory = if self.enable_memory {
            Some(interpreter_state.heap().to_vec())
        } else {
            None
        };

        let stack = if self.enable_stack {
            Some(interpreter_state.stack().to_slice().to_vec())
        } else {
            None
        };

        let return_data = if self.enable_returndata {
            Some(interpreter_state.return_data().to_vec())
        } else {
            None
        };

        let storage = if self.enable_storage {
            Some(
                self.storage_caches_for_frames
                    .last()
                    .expect("Should exist")
                    .iter()
                    .map(|(key, value)| (*key, *value))
                    .collect(),
            )
        } else {
            None
        };

        let transient_storage = if self.enable_transient_storage {
            Some(
                self.transient_storage_caches_for_frames
                    .last()
                    .expect("Should exist")
                    .iter()
                    .map(|(key, value)| (*key, *value))
                    .collect(),
            )
        } else {
            None
        };

        self.last_known_gas_left = interpreter_state.resources().ergs().0 / ERGS_PER_GAS;

        tx_log.steps.push(EvmExecutionStep {
            pc: interpreter_state.instruction_pointer(),
            opcode_raw: opcode,
            opcode: opcode_decoded,
            gas: interpreter_state.resources().ergs().0 / ERGS_PER_GAS,
            gas_used: None, // will be populated later
            memory,
            mem_size: interpreter_state.heap().len(),
            stack,
            return_data,
            storage,
            transient_storage,
            depth: self.current_call_depth,
            refund: interpreter_state.refund_counter() as u64, // Always zero if refunds are disabled
            error: None, // Can be populated in `on_opcode_error` or `on_call_error`
        });

        // Hacking our way to track gas used by call-like opcodes
        if let Some((opcode_log_index, last_known_gas)) =
            self.pending_call_opcodes.remove(&self.current_call_depth)
        {
            // Looks like we continue execution after call

            let tx_log = self.transaction_logs.last_mut().expect("Should exist");
            let opcode_log = tx_log
                .steps
                .get_mut(opcode_log_index)
                .expect("Should exist");

            let gas_used = last_known_gas
                - interpreter_state.resources().ergs().0 / ERGS_PER_GAS
                - self.gas_used_by_last_call;
            opcode_log.gas_used = Some(gas_used);
        }
    }

    /// Calculate opcode gas cost after it's execution
    /// Note: call/create move control flow to a new frame AFTER this hook is called
    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        interpreter_state: &impl EvmFrameInterface<S>,
    ) {
        let gas_used = self
            .last_known_gas_left
            .checked_sub(interpreter_state.resources().ergs().0 / ERGS_PER_GAS)
            .expect("Unexpected gas value");

        let tx_log = self.transaction_logs.last_mut().expect("Should exist");
        let last_opcode_record = tx_log.steps.last_mut().expect("Should exist");

        last_opcode_record.gas_used = Some(gas_used);

        // Note: This will work for "simple" opcodes.
        // For calls and deployments For X we'll have to do something ugly. Since calls affect caller frame
        // after preemption to OS (they can have some post-effects AFTER this hook is called).
        // This can be confusing, and maybe in the future logic of this hook will be changed.
    }

    /// Opcode failed for some reason
    fn on_opcode_error(&mut self, error: &EvmError, _frame_state: &impl EvmFrameInterface<S>) {
        let tx_log = self.transaction_logs.last_mut().expect("Should exist");
        let last_opcode_record = tx_log.steps.last_mut().expect("Should exist");

        last_opcode_record.error = Some(error.clone());
    }

    /// Special cases, when error happens in frame before any opcode is executed (unfortunately we can't provide access to state)
    fn on_call_error(&mut self, error: &EvmError) {
        let tx_log = self.transaction_logs.last_mut().expect("Should exist");
        let last_opcode_record = tx_log.steps.last_mut().expect("Should exist");

        // Assume that last opcode is correct

        last_opcode_record.error = Some(error.clone());
    }

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

impl<S: EthereumLikeTypes> Tracer<S> for EvmOpcodesLogger<S> {
    fn on_new_execution_frame(&mut self, request: &ExecutionEnvironmentLaunchParams<S>) {
        // Hacking our way to track gas used by call-like opcodes
        let tx_log = self.transaction_logs.last_mut().expect("Should exist");
        if tx_log.steps.last_mut().is_some() {
            self.pending_call_opcodes.insert(
                self.current_call_depth,
                (tx_log.steps.len() - 1, self.last_known_gas_left),
            );
        }

        self.current_call_depth += 1;

        if self.enable_storage {
            self.storage_caches_for_frames.push(Default::default());
        }

        if self.enable_transient_storage {
            self.transient_storage_caches_for_frames
                .push(Default::default());
        }

        self.gas_used_by_calls
            .push(request.external_call.available_resources.ergs().0 / ERGS_PER_GAS);
        // Save passed amount of gas
    }

    fn after_execution_frame_completed(&mut self, result: Option<(&S::Resources, &CallResult<S>)>) {
        assert_ne!(self.current_call_depth, 0);

        // Hacking our way to track gas used by call-like opcodes
        if let Some((opcode_log_index, last_known_gas)) =
            self.pending_call_opcodes.remove(&self.current_call_depth)
        {
            // Looks like call frame finished immediately after call-like opcode

            let tx_log = self.transaction_logs.last_mut().expect("Should exist");
            let opcode_log = tx_log
                .steps
                .get_mut(opcode_log_index)
                .expect("Should exist");
            match result {
                Some((resources_to_return, _)) => {
                    // TODO: we blindly expect that `gas_used_by_last_call` calculation is correct
                    let gas_used = last_known_gas
                        - resources_to_return.ergs().0 / ERGS_PER_GAS
                        - self.gas_used_by_last_call;
                    opcode_log.gas_used = Some(gas_used);
                }
                None => {
                    // Something terrible happened. Unfortunately we can't derive gas used for the parent's call opcode
                    opcode_log.gas_used = None
                }
            }
        }

        if let Some(call_result) = result {
            let last_call_gas_record = self.gas_used_by_calls.pop().expect("Should exist");
            self.gas_used_by_last_call =
                last_call_gas_record - call_result.0.ergs().0 / ERGS_PER_GAS; // Save gas used by call
        } else {
            // Something terrible happened (fatal error)
        }

        self.current_call_depth -= 1;

        if self.enable_storage {
            self.storage_caches_for_frames.pop().expect("Should exist");
        }

        if self.enable_transient_storage {
            self.transient_storage_caches_for_frames
                .pop()
                .expect("Should exist");
        }
    }

    fn on_storage_read(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
        if is_transient {
            if self.enable_transient_storage {
                let _ = self
                    .transient_storage_caches_for_frames
                    .last_mut()
                    .expect("Should exist")
                    .insert(key, value);
            }
        } else if self.enable_storage {
            let _ = self
                .storage_caches_for_frames
                .last_mut()
                .expect("Should exist")
                .insert(key, value);
        }
    }

    fn on_storage_write(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
        if is_transient {
            if self.enable_transient_storage {
                let _ = self
                    .transient_storage_caches_for_frames
                    .last_mut()
                    .expect("Should exist")
                    .insert(key, value);
            }
        } else if self.enable_storage {
            let _ = self
                .storage_caches_for_frames
                .last_mut()
                .expect("Should exist")
                .insert(key, value);
        }
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {
        self.transaction_logs.push(TransactionLog::default());
        self.current_call_depth = 0;
    }

    fn finish_tx(&mut self) {
        assert_eq!(self.current_call_depth, 0);
        let tx_log = self.transaction_logs.last_mut().expect("Should exist");
        tx_log.finished = true;
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
    fn on_bytecode_change(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _address: <S::IOTypes as SystemIOTypesConfig>::Address,
        _new_bytecode: Option<&[u8]>,
        _new_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        _new_observable_bytecode_length: u32,
    ) {
    }

    #[inline(always)]
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        self
    }
}
