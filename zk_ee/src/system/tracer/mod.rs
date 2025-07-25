use evm_state_for_tracer::EvmStateForTracer;

use crate::{
    execution_environment_type::ExecutionEnvironmentType, types_config::SystemIOTypesConfig,
};

use super::{CallOrDeployResultRef, ExecutionEnvironmentLaunchParams, SystemTypes};

pub mod evm_state_for_tracer;

pub trait Tracer<S: SystemTypes> {
    fn should_call_before_evm_execution_step(&self) -> bool;
    fn should_call_after_evm_execution_step(&self) -> bool;

    /// Flag for hook before external call or deployment execution
    fn should_call_on_call_or_deployment(&self) -> bool;
    /// Flag for hook after external call or deployment execution or failure
    fn should_call_after_call_or_deployment(&self) -> bool;

    fn is_on_storage_read_enabled(&self) -> bool;
    fn is_on_storage_write_enabled(&self) -> bool;

    fn before_interpreter_execution_step(
        &mut self,
        opcode: u8,
        interpreter_state: EvmStateForTracer<S>,
    );
    fn after_interpreter_execution_step(
        &mut self,
        opcode: u8,
        interpreter_state: EvmStateForTracer<S>,
    );

    /// Hook before external call or deployment execution
    fn external_call_or_deployment(&mut self, request: &ExecutionEnvironmentLaunchParams<S>);

    /// Hook after external call or deployment execution or failure
    fn external_call_or_deployment_completed(
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

    fn begin_tx(&mut self);

    fn finish_tx(&mut self);
}

pub struct NopTracer {}

impl<S: SystemTypes> Tracer<S> for NopTracer {
    #[inline(always)]
    fn should_call_before_evm_execution_step(&self) -> bool {
        false
    }

    #[inline(always)]
    fn should_call_after_evm_execution_step(&self) -> bool {
        false
    }

    #[inline(always)]
    fn should_call_on_call_or_deployment(&self) -> bool {
        false
    }

    #[inline(always)]
    fn should_call_after_call_or_deployment(&self) -> bool {
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

    fn before_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: EvmStateForTracer<S>,
    ) {
    }

    fn after_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: EvmStateForTracer<S>,
    ) {
    }

    fn external_call_or_deployment(&mut self, _request: &ExecutionEnvironmentLaunchParams<S>) {}

    fn external_call_or_deployment_completed(
        &mut self,
        _result: Option<(&S::Resources, CallOrDeployResultRef<S>)>,
    ) {
    }

    fn begin_tx(&mut self) {}

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
}
