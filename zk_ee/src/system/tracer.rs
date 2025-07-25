use core::{mem::MaybeUninit, ops::Range};

use ruint::aliases::U256;

use crate::{
    execution_environment_type::ExecutionEnvironmentType, memory::slice_vec::SliceVec,
    types_config::SystemIOTypesConfig,
};

use super::{CallOrDeployResultRef, ExecutionEnvironmentLaunchParams, SystemTypes};

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

pub struct EvmStateForTracer<'a, S: SystemTypes> {
    /// Instruction pointer
    pub instruction_pointer: usize,
    /// Resources left
    pub resources: &'a S::Resources,
    /// Stack view
    pub stack: EvmStackForTracer<'a>,
    /// Caller address
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// Callee address
    pub address: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// calldata
    pub calldata: &'a [u8],
    /// returndata is available from here if it exists
    pub returndata: &'a [u8],
    /// Heap that belongs to this interpreter frame
    pub heap: &'a SliceVec<'a, u8>,
    /// returndata location serves to save range information at various points
    pub returndata_location: Range<usize>,
    /// Bytecode
    pub bytecode: &'a [u8],
    /// Call value
    pub call_value: &'a U256,
    /// Is interpreter call static.
    pub is_static: bool,
    /// Is interpreter call executing construction code.
    pub is_constructor: bool,
}

pub struct EvmStackForTracer<'a> {
    buffer: &'a [MaybeUninit<U256>; 1024],
    // our length both indicates how many elements are there, and
    // at least how many of them are initialized
    len: usize,
}

impl<'a> EvmStackForTracer<'a> {
    /// # Safety
    /// First `len` elements of buffer are expected to be initialized
    pub unsafe fn from_parts(buffer: &'a [MaybeUninit<U256>; 1024], len: usize) -> Self {
        assert!(len <= 1024);
        Self { buffer, len }
    }

    pub fn to_slice(&self) -> &[U256] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast::<U256>(), self.len) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn peek_n(&self, index: usize) -> Result<&'a U256, EvmStackError> {
        unsafe {
            if self.len < index + 1 {
                return Err(EvmStackError::StackUnderflow);
            }
            let offset = self.len - (index + 1);
            let p0 = self
                .buffer
                .as_ptr()
                .add(offset)
                .as_ref()
                .expect("Should not be null")
                .assume_init_ref();

            Ok(p0)
        }
    }
}

pub enum EvmStackError {
    StackUnderflow,
}
