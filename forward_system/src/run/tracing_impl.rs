use crate::run::convert::IntoInterface;
use crate::run::convert_alloy::IntoAlloy;
use alloy::primitives::{Address, U256};
use std::{cell::OnceCell, marker::PhantomData};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::evm::{EvmError as ZkEEEvmError, EvmFrameInterface, EvmStackInterface};
use zk_ee::system::tracer::evm_tracer::EvmTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{
    CallModifier, CallResult, Computational, EthereumLikeTypes, ExecutionEnvironmentLaunchParams,
    Resources, SystemTypes,
};
use zk_ee::types_config::SystemIOTypesConfig;
use zksync_os_evm_errors::EvmError as InterfaceEvmError;
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
    stack_wrapper: EvmStackInterfaceWrapped<'a>,
    /// Cached conversion of call_value from u256::U256 to alloy U256, so we can return a
    /// reference without relying on layout transmute.
    call_value_converted: U256,
    _phantom: PhantomData<S>,
}

/// Wrapper around internal [`EvmStackInterface`] to make it compatible with interface tracing API.
struct EvmStackInterfaceWrapped<'a> {
    cached_values: OnceCell<Vec<U256>>,
    inner: &'a dyn EvmStackInterface,
}

impl<'a, S: EthereumLikeTypes + 'a, T: EvmFrameInterface<S>> From<&'a T>
    for EvmFrameInterfaceWrapped<'a, S, T>
{
    fn from(value: &'a T) -> Self {
        let call_value_converted: ruint::aliases::U256 = value.call_value().clone().into();
        Self {
            inner: value,
            stack_wrapper: EvmStackInterfaceWrapped {
                cached_values: OnceCell::new(),
                inner: value.stack(),
            },
            call_value_converted,
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
            address.into_alloy(),
            key.into_alloy(),
            value.into_alloy(),
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
            address.into_alloy(),
            key.into_alloy(),
            value.into_alloy(),
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
            address.into_alloy(),
            new_raw_bytecode,
            new_internal_bytecode_hash.into_alloy(),
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
            (*address).into_alloy(),
            topics.iter().map(|b| b.into_alloy()).collect::<Vec<_>>(),
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

    fn on_opcode_error(&mut self, error: &ZkEEEvmError, frame_state: &impl EvmFrameInterface<S>) {
        let interface_error: InterfaceEvmError = error.clone().into_interface();
        self.0.on_opcode_error(
            &interface_error,
            EvmFrameInterfaceWrapped::from(frame_state),
        )
    }

    fn on_call_error(&mut self, error: &ZkEEEvmError) {
        let interface_error: InterfaceEvmError = error.clone().into_interface();
        self.0.on_call_error(&interface_error)
    }

    fn on_selfdestruct(
        &mut self,
        beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.0.on_selfdestruct(
            beneficiary.into_alloy(),
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
        self.0.external_call.caller.into_alloy()
    }

    fn callee(&self) -> Address {
        self.0.external_call.callee.into_alloy()
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
        self.inner.caller().into_alloy()
    }

    fn address(&self) -> Address {
        self.inner.address().into_alloy()
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
        &self.call_value_converted
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

    fn stack(&self) -> &impl zksync_os_interface::tracing::EvmStackInterface {
        &self.stack_wrapper
    }
}

impl<'a> zksync_os_interface::tracing::EvmStackInterface for EvmStackInterfaceWrapped<'a> {
    fn to_slice(&self) -> &[U256] {
        self.cached_values
            .get_or_init(|| {
                self.inner
                    .to_slice()
                    .iter()
                    .map(|value| -> U256 { value.clone().into() })
                    .collect()
            })
            .as_slice()
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn peek_n(&self, index: usize) -> Result<&U256, InterfaceEvmError> {
        let offset = self
            .inner
            .len()
            .checked_sub(index + 1)
            .ok_or(InterfaceEvmError::StackUnderflow)?;
        self.to_slice()
            .get(offset)
            .ok_or(InterfaceEvmError::StackUnderflow)
    }
}
