use evm_tracer::{EvmTracer, NopEvmTracer};

use crate::{
    execution_environment_type::ExecutionEnvironmentType, types_config::SystemIOTypesConfig,
};

use super::{CallOrDeployResultRef, ExecutionEnvironmentLaunchParams, SystemTypes};

pub mod evm_tracer;

pub trait Tracer<S: SystemTypes> {
    /// Should return EVM-specific tracer implementation
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S>;

    /// Hook immediately before external call or deployment frame execution
    fn on_new_execution_frame(&mut self, request: &ExecutionEnvironmentLaunchParams<S>);

    /// Hook immediately after external call or deployment frame execution
    fn after_execution_frame_completed(
        &mut self,
        result: Option<(&S::Resources, CallOrDeployResultRef<S>)>,
    );

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
        _result: Option<(&S::Resources, CallOrDeployResultRef<S>)>,
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
