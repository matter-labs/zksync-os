use evm_tracer::{EvmTracer, NopEvmTracer};

use crate::{
    execution_environment_type::ExecutionEnvironmentType, types_config::SystemIOTypesConfig,
};

use super::{CallResult, ExecutionEnvironmentLaunchParams, SystemTypes};

pub mod evm_tracer;

pub trait Tracer<S: SystemTypes> {
    /// Should return EVM-specific tracer implementation
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S>;

    /// Hook immediately before external call or deployment frame execution
    fn on_new_execution_frame(&mut self, request: &ExecutionEnvironmentLaunchParams<S>);

    /// Hook immediately after external call or deployment frame execution
    ///
    /// Note: `result` is None if execution is terminated due to internal ZKsync OS error (e.g. out-of-native-resources)
    fn after_execution_frame_completed(&mut self, result: Option<(&S::Resources, &CallResult<S>)>);

    /// Is called on storage read produced by bytecode execution in EE
    fn on_storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        key: <S::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <S::IOTypes as SystemIOTypesConfig>::StorageValue,
    );

    /// Is called on storage read produced by bytecode execution in EE
    fn on_storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        key: <S::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <S::IOTypes as SystemIOTypesConfig>::StorageValue,
    );

    /// Is called on a change of bytecode for some account.
    /// `new_raw_bytecode` can be None if bytecode is unknown at the moment of change (e.g. force deploy by hash in system hook)
    ///
    /// Note: currently is *not* triggered by system hooks
    fn on_bytecode_change(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        new_raw_bytecode: Option<&[u8]>,
        new_internal_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        new_observable_bytecode_length: u32,
    );

    /// Is called before EE emits and event
    fn on_event(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        topics: &[<S::IOTypes as SystemIOTypesConfig>::EventKey],
        data: &[u8],
    );

    /// Is called before bootloader starts execution of a transaction
    fn begin_tx(&mut self, calldata: &[u8]);

    /// Is called after bootloader finishes execution of a transaction
    fn finish_tx(&mut self);
}

#[derive(Default)]
pub struct NopTracer {
    evm_tracer: NopEvmTracer,
}

impl<S: SystemTypes> Tracer<S> for NopTracer {
    #[inline(always)]
    fn on_new_execution_frame(&mut self, _request: &ExecutionEnvironmentLaunchParams<S>) {}

    #[inline(always)]
    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(&S::Resources, &CallResult<S>)>,
    ) {
    }

    #[inline(always)]
    fn begin_tx(&mut self, _calldata: &[u8]) {}

    #[inline(always)]
    fn finish_tx(&mut self) {}

    #[inline(always)]
    fn on_storage_read(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_bytecode_change(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: <S::IOTypes as SystemIOTypesConfig>::Address,
        _new_bytecode: Option<&[u8]>,
        _new_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        _new_observable_bytecode_length: u32,
    ) {
    }

    #[inline(always)]
    fn on_event(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _topics: &[<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::EventKey],
        _data: &[u8],
    ) {
    }

    #[inline(always)]
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        &mut self.evm_tracer
    }
}
