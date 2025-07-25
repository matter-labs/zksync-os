// Reference implementation of EVM opcodes logger. Not feature complete for production

use std::{collections::HashMap, marker::PhantomData, ops::Deref};

use evm_interpreter::{opcodes::OpCode, ERGS_PER_GAS};
use ruint::aliases::U256;
use serde::Serialize;
use zk_ee::{
    system::{
        tracer::{
            evm_tracer::{EvmFrameForTracer, EvmTracer},
            Tracer,
        },
        CallOrDeployResultRef, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, Resources,
        SystemTypes,
    },
    types_config::SystemIOTypesConfig,
    utils::Bytes32,
};

#[derive(Default, Debug, Serialize)]
pub struct EvmExecutionStep {
    pc: usize,
    opcode_raw: u8,
    opcode: Option<String>,
    gas: u64,
    memory: Option<Vec<u8>>,
    mem_size: usize,
    stack: Option<Vec<U256>>,
    return_data: Option<Vec<u8>>,
    storage: Option<Vec<(Bytes32, Bytes32)>>,
    transient_storage: Option<Vec<(Bytes32, Bytes32)>>,
    depth: usize,
    refund: u64, // Always zero for now
}

#[derive(Default, Debug, Serialize)]
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
            _marker: Default::default(),
        }
    }
}

impl<S: EthereumLikeTypes> EvmTracer<S> for EvmOpcodesLogger<S> {
    #[inline(always)]
    fn is_on_evm_execution_step_enabled(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_after_evm_execution_step_enabled(&self) -> bool {
        false
    }

    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        interpreter_state: EvmFrameForTracer<S>,
    ) {
        if self.limit != 0 && self.steps_counter > self.limit {
            return;
        }
        self.steps_counter += 1;

        let tx_log = self.transaction_logs.last_mut().expect("Should exist");

        let opcode_decoded = OpCode::try_from_u8(opcode).map(|x| x.as_str().to_owned());

        let memory = if self.enable_memory {
            Some(interpreter_state.heap.deref().to_vec())
        } else {
            None
        };

        let stack = if self.enable_stack {
            Some(interpreter_state.stack.to_slice().to_vec())
        } else {
            None
        };

        let return_data = if self.enable_returndata {
            Some(interpreter_state.returndata.to_vec())
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

        tx_log.steps.push(EvmExecutionStep {
            pc: interpreter_state.instruction_pointer,
            opcode_raw: opcode,
            opcode: opcode_decoded,
            gas: interpreter_state.resources.ergs().0 / ERGS_PER_GAS,
            memory,
            mem_size: interpreter_state.heap.len(),
            stack,
            return_data,
            storage,
            transient_storage,
            depth: self.current_call_depth,
            refund: 0, // Always zero for now
        })
    }

    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: EvmFrameForTracer<S>,
    ) {
        unreachable!()
    }
}

impl<S: EthereumLikeTypes> Tracer<S> for EvmOpcodesLogger<S> {
    #[inline(always)]
    fn is_begin_tx_enabled(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_finish_tx_enabled(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_on_new_execution_frame_enabled(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_after_execution_frame_enabled(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_on_storage_read_enabled(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_on_storage_write_enabled(&self) -> bool {
        true
    }

    #[inline(always)]
    fn is_on_event_enabled(&self) -> bool {
        false
    }

    fn on_new_execution_frame(&mut self, _request: &ExecutionEnvironmentLaunchParams<S>) {
        self.current_call_depth += 1;

        if self.enable_storage {
            self.storage_caches_for_frames.push(Default::default());
        }

        if self.enable_transient_storage {
            self.transient_storage_caches_for_frames
                .push(Default::default());
        }
    }

    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(&S::Resources, CallOrDeployResultRef<S>)>,
    ) {
        assert_ne!(self.current_call_depth, 0);
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

    fn on_event(
        &mut self,
        _ee_type: zk_ee::execution_environment_type::ExecutionEnvironmentType,
        _address: &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _topics: &[<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::EventKey],
        _data: &[u8],
    ) {
        unreachable!()
    }

    #[inline(always)]
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        self
    }
}
