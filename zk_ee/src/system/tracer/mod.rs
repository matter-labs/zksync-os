use evm_tracer::{EvmTracer, NopEvmTracer};

use crate::{
    execution_environment_type::ExecutionEnvironmentType, types_config::SystemIOTypesConfig,
};

use super::{CallOrDeployResultRef, ExecutionEnvironmentLaunchParams, SystemTypes};

pub mod evm_tracer;

pub trait Tracer<S: SystemTypes> {
    fn is_begin_tx_enabled(&self) -> bool;
    fn is_finish_tx_enabled(&self) -> bool;

    fn is_on_new_execution_frame_enabled(&self) -> bool;
    fn is_after_execution_frame_enabled(&self) -> bool;

    fn is_on_storage_read_enabled(&self) -> bool;
    fn is_on_storage_write_enabled(&self) -> bool;

    fn is_on_event_enabled(&self) -> bool;

    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S>;

    /// Hook before external call or deployment execution
    fn on_new_execution_frame(&mut self, request: &ExecutionEnvironmentLaunchParams<S>);

    /// Hook after external call or deployment execution or failure
    fn after_execution_frame_completed(
        &mut self,
        result: Option<(&S::Resources, CallOrDeployResultRef<S>)>,
    );

    fn on_storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        key: <S::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <S::IOTypes as SystemIOTypesConfig>::StorageValue,
    );

    fn on_storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        key: <S::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <S::IOTypes as SystemIOTypesConfig>::StorageValue,
    );

    fn on_event(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        topics: &[<S::IOTypes as SystemIOTypesConfig>::EventKey],
        data: &[u8],
    );

    fn begin_tx(&mut self, calldata: &[u8]);

    fn finish_tx(&mut self);
}

#[derive(Default)]
pub struct NopTracer {
    evm_tracer: NopEvmTracer,
}

impl<S: SystemTypes> Tracer<S> for NopTracer {
    #[inline(always)]
    fn is_begin_tx_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_finish_tx_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_on_new_execution_frame_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_after_execution_frame_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_on_storage_read_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_on_storage_write_enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_on_event_enabled(&self) -> bool {
        false
    }

    fn on_new_execution_frame(&mut self, _request: &ExecutionEnvironmentLaunchParams<S>) {}

    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(&S::Resources, CallOrDeployResultRef<S>)>,
    ) {
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {}

    fn finish_tx(&mut self) {}

    fn on_storage_read(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

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
