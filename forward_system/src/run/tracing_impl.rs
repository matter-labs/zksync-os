use alloy::primitives::{Address, U256};
use std::marker::PhantomData;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::evm::EvmFrameInterface;
use zk_ee::system::tracer::evm_tracer::EvmTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{
    CallModifier, CallResult, Computational, EthereumLikeTypes, ExecutionEnvironmentLaunchParams,
    Resources, SystemTypes,
};
use zk_ee::types_config::SystemIOTypesConfig;
use zksync_os_evm_errors::EvmError;
use zksync_os_interface::tracing::{EvmRequest, EvmResources};

/// Wrapper around interface `EvmTracer` to make it compatible with `zk_ee` tracing API.
pub(crate) struct TracerWrapped<'a, T: zksync_os_interface::tracing::EvmTracer>(pub &'a mut T);

/// Wrapper around [`ExecutionEnvironmentLaunchParams`] to make it compatible with interface tracing API.
struct ExecutionEnvironmentLaunchParamsWrapped<'a, 'b, S: EthereumLikeTypes>(
    &'a ExecutionEnvironmentLaunchParams<'b, S>,
);

/// Wrapper around [`EvmFrameInterface`] to make it compatible with interface tracing API.
struct EvmFrameInterfaceWrapped<'a, S: EthereumLikeTypes, T: EvmFrameInterface<S>> {
    inner: &'a T,
    _phantom: PhantomData<S>,
}

impl<'a, S: EthereumLikeTypes, T: EvmFrameInterface<S>> From<&'a T>
    for EvmFrameInterfaceWrapped<'a, S, T>
{
    fn from(value: &'a T) -> Self {
        Self {
            inner: value,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: zksync_os_interface::tracing::EvmTracer, S: EthereumLikeTypes> Tracer<S>
    for TracerWrapped<'a, T>
{
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        self
    }

    fn on_new_execution_frame(&mut self, request: &ExecutionEnvironmentLaunchParams<S>) {
        self.0
            .on_new_execution_frame(ExecutionEnvironmentLaunchParamsWrapped(request));
    }

    fn after_execution_frame_completed(&mut self, result: Option<(&S::Resources, &CallResult<S>)>) {
        let result = result.map(|(resources, call_result)| {
            let call_result = match call_result {
                CallResult::PreparationStepFailed => {
                    panic!("Should not happen") // ZKsync OS should not call tracer in this case
                }
                CallResult::Failed { return_values } => {
                    zksync_os_interface::tracing::CallResult::Failed {
                        returndata: return_values.returndata,
                    }
                }
                CallResult::Successful { return_values } => {
                    zksync_os_interface::tracing::CallResult::Successful {
                        returndata: return_values.returndata,
                    }
                }
            };
            (
                EvmResources {
                    ergs: resources.ergs().0,
                    native: resources.native().as_u64(),
                },
                call_result,
            )
        });
        self.0.after_execution_frame_completed(result)
    }

    fn on_storage_read(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        key: <S::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <S::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
        self.0.on_storage_read(
            is_transient,
            address.to_be_bytes().into(),
            key.as_u8_array().into(),
            value.as_u8_array().into(),
        )
    }

    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        key: <S::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <S::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
        self.0.on_storage_write(
            is_transient,
            address.to_be_bytes().into(),
            key.as_u8_array().into(),
            value.as_u8_array().into(),
        )
    }

    fn on_bytecode_change(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        new_raw_bytecode: Option<&[u8]>,
        new_internal_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        new_observable_bytecode_length: u32,
    ) {
        self.0.on_bytecode_change(
            address.to_be_bytes().into(),
            new_raw_bytecode,
            new_internal_bytecode_hash.as_u8_array().into(),
            new_observable_bytecode_length,
        )
    }

    fn on_event(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        topics: &[<S::IOTypes as SystemIOTypesConfig>::EventKey],
        data: &[u8],
    ) {
        self.0.on_event(
            address.to_be_bytes().into(),
            topics
                .iter()
                .map(|b| b.as_u8_array().into())
                .collect::<Vec<_>>(),
            data,
        )
    }

    fn begin_tx(&mut self, calldata: &[u8]) {
        self.0.begin_tx(calldata)
    }

    fn finish_tx(&mut self) {
        self.0.finish_tx()
    }
}

impl<'a, T: zksync_os_interface::tracing::EvmTracer, S: EthereumLikeTypes> EvmTracer<S>
    for TracerWrapped<'a, T>
{
    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.0.before_evm_interpreter_execution_step(
            opcode,
            EvmFrameInterfaceWrapped::from(frame_state),
        )
    }

    fn after_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.0.after_evm_interpreter_execution_step(
            opcode,
            EvmFrameInterfaceWrapped::from(frame_state),
        )
    }

    fn on_opcode_error(&mut self, error: &EvmError, frame_state: &impl EvmFrameInterface<S>) {
        self.0
            .on_opcode_error(error, EvmFrameInterfaceWrapped::from(frame_state))
    }

    fn on_call_error(&mut self, error: &EvmError) {
        self.0.on_call_error(error)
    }

    fn on_selfdestruct(
        &mut self,
        beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.0.on_selfdestruct(
            beneficiary.to_be_bytes().into(),
            token_value,
            EvmFrameInterfaceWrapped::from(frame_state),
        )
    }

    fn on_create_request(&mut self, is_create2: bool) {
        self.0.on_create_request(is_create2)
    }
}

impl<'a, 'b, S: EthereumLikeTypes> EvmRequest
    for ExecutionEnvironmentLaunchParamsWrapped<'a, 'b, S>
{
    fn resources(&self) -> EvmResources {
        let resources = &self.0.external_call.available_resources;
        EvmResources {
            ergs: resources.ergs().0,
            native: resources.native().as_u64(),
        }
    }

    fn caller(&self) -> Address {
        self.0.external_call.caller.to_be_bytes().into()
    }

    fn callee(&self) -> Address {
        self.0.external_call.callee.to_be_bytes().into()
    }

    fn modifier(&self) -> zksync_os_interface::tracing::CallModifier {
        match self.0.external_call.modifier {
            CallModifier::NoModifier => zksync_os_interface::tracing::CallModifier::NoModifier,
            CallModifier::Constructor => zksync_os_interface::tracing::CallModifier::Constructor,
            CallModifier::Delegate => zksync_os_interface::tracing::CallModifier::Delegate,
            CallModifier::Static => zksync_os_interface::tracing::CallModifier::Static,
            CallModifier::DelegateStatic => {
                zksync_os_interface::tracing::CallModifier::DelegateStatic
            }
            CallModifier::ZKVMSystem => zksync_os_interface::tracing::CallModifier::ZKVMSystem,
            CallModifier::ZKVMSystemStatic => {
                zksync_os_interface::tracing::CallModifier::ZKVMSystemStatic
            }
            CallModifier::EVMCallcode => zksync_os_interface::tracing::CallModifier::EVMCallcode,
            CallModifier::EVMCallcodeStatic => {
                zksync_os_interface::tracing::CallModifier::EVMCallcodeStatic
            }
        }
    }

    fn input(&self) -> &[u8] {
        self.0.external_call.input
    }

    fn nominal_token_value(&self) -> U256 {
        self.0.external_call.nominal_token_value
    }
}

impl<'a, S: EthereumLikeTypes, T: EvmFrameInterface<S>>
    zksync_os_interface::tracing::EvmFrameInterface for EvmFrameInterfaceWrapped<'a, S, T>
{
    fn instruction_pointer(&self) -> usize {
        self.inner.instruction_pointer()
    }

    fn resources(&self) -> EvmResources {
        let resources = self.inner.resources();
        EvmResources {
            ergs: resources.ergs().0,
            native: resources.native().as_u64(),
        }
    }

    fn caller(&self) -> Address {
        self.inner.caller().to_be_bytes().into()
    }

    fn address(&self) -> Address {
        self.inner.address().to_be_bytes().into()
    }

    fn calldata(&self) -> &[u8] {
        self.inner.calldata()
    }

    fn return_data(&self) -> &[u8] {
        self.inner.return_data()
    }

    fn heap(&self) -> &[u8] {
        self.inner.heap()
    }

    fn bytecode(&self) -> &[u8] {
        self.inner.bytecode()
    }

    fn call_value(&self) -> &U256 {
        self.inner.call_value()
    }

    fn refund_counter(&self) -> u32 {
        self.inner.refund_counter()
    }

    fn is_static(&self) -> bool {
        self.inner.is_static()
    }

    fn is_constructor(&self) -> bool {
        self.inner.is_constructor()
    }
}
