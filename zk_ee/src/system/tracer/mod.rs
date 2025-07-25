use evm_state_for_tracer::EvmFrameForTracer;

use crate::{
    execution_environment_type::ExecutionEnvironmentType, types_config::SystemIOTypesConfig,
};

use super::{CallOrDeployResultRef, ExecutionEnvironmentLaunchParams, SystemTypes};

pub mod evm_state_for_tracer;

pub trait Tracer<S: SystemTypes> {
    fn should_call_before_evm_execution_step(&self) -> bool;
    fn should_call_after_evm_execution_step(&self) -> bool;

    /// Flag for hook before external call or deployment execution
    fn should_call_on_new_execution_frame(&self) -> bool;
    /// Flag for hook after external call or deployment execution or failure
    fn should_call_after_execution_frame_completed(&self) -> bool;

    fn is_on_storage_read_enabled(&self) -> bool;
    fn is_on_storage_write_enabled(&self) -> bool;

    fn before_interpreter_execution_step(
        &mut self,
        opcode: u8,
        interpreter_state: EvmFrameForTracer<S>,
    );
    fn after_interpreter_execution_step(
        &mut self,
        opcode: u8,
        interpreter_state: EvmFrameForTracer<S>,
    );

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
    fn should_call_on_new_execution_frame(&self) -> bool {
        false
    }

    #[inline(always)]
    fn should_call_after_execution_frame_completed(&self) -> bool {
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
        _interpreter_state: EvmFrameForTracer<S>,
    ) {
    }

    fn after_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _interpreter_state: EvmFrameForTracer<S>,
    ) {
    }

    fn on_new_execution_frame(&mut self, _request: &ExecutionEnvironmentLaunchParams<S>) {}

    fn after_execution_frame_completed(
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
