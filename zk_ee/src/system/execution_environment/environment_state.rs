use core::fmt::Debug;

use crate::{
    common_structs::CalleeAccountProperties,
    memory::slice_vec::SliceVec,
    system::{system::SystemTypes, CallModifier, Ergs, MAX_SCRATCH_SPACE_USIZE_WORDS},
    types_config::SystemIOTypesConfig,
};

use super::{CallResult, ReturnValues};

/// Everything an execution environment needs to know to start execution
pub struct ExecutionEnvironmentLaunchParams<'a, S: SystemTypes> {
    pub external_call: ExternalCallRequest<'a, S>,
    pub environment_parameters: EnvironmentParameters<'a>,
}

pub enum ExecutionEnvironmentPreemptionPoint<'a, S: SystemTypes> {
    CallRequest {
        request: ExternalCallRequest<'a, S>,
        heap: SliceVec<'a, u8>,
    },
    End(CompletedExecution<'a, S>),
}

pub struct ExternalCallRequest<'a, S: SystemTypes> {
    pub available_resources: S::Resources,
    pub ergs_to_pass: Ergs,
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callee: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub modifier: CallModifier,
    pub input: &'a [u8],
    /// Base tokens attached to this call.
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub call_scratch_space:
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], S::Allocator>>,
}

impl<S: SystemTypes> Default for ExternalCallRequest<'_, S>
where
    S::Resources: Default,
{
    fn default() -> Self {
        Self {
            available_resources: S::Resources::default(),
            ergs_to_pass: Ergs::default(),
            caller: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            callee: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            modifier: CallModifier::NoModifier,
            input: &[],
            nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue::default(),
            call_scratch_space: None,
        }
    }
}

impl<S: SystemTypes> ExternalCallRequest<'_, S> {
    #[inline]
    pub fn is_transfer_allowed(&self) -> bool {
        self.modifier == CallModifier::NoModifier
        || self.modifier == CallModifier::Constructor
        || self.modifier == CallModifier::ZKVMSystem
        || self.modifier == CallModifier::EVMCallcode
        // Positive-value callcode calls are allowed in static context,
        // as the transfer is a self-transfer.
        || self.modifier == CallModifier::EVMCallcodeStatic
    }

    #[inline]
    pub fn is_delegate(&self) -> bool {
        self.modifier == CallModifier::Delegate || self.modifier == CallModifier::DelegateStatic
    }
    #[inline]
    pub fn is_callcode(&self) -> bool {
        self.modifier == CallModifier::EVMCallcode
            || self.modifier == CallModifier::EVMCallcodeStatic
    }

    #[inline]
    pub fn next_frame_self_address(&self) -> &<S::IOTypes as SystemIOTypesConfig>::Address {
        &self.callee
    }
}

pub struct CompletedExecution<'a, S: SystemTypes> {
    pub resources_returned: S::Resources,
    pub result: CallResult<'a, S>,
}

impl<'a, S: SystemTypes> CompletedExecution<'a, S> {
    #[inline]
    pub fn failed(&self) -> bool {
        self.result.failed()
    }

    #[inline]
    pub fn return_values(self) -> ReturnValues<'a, S> {
        self.result.return_values()
    }
}

impl<S: SystemTypes> Debug for ExternalCallRequest<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExternalCallRequest")
            .field("available_resources", &self.available_resources)
            .field("ergs_to_pass", &self.ergs_to_pass)
            .field("caller", &self.caller)
            .field("callee", &self.callee)
            .field("callers_caller", &self.callers_caller)
            .field("modifier", &self.modifier)
            .field("calldata", &self.input)
            .field("nominal_token_value", &self.nominal_token_value)
            .field("call_scratch_space", &self.call_scratch_space)
            .finish()
    }
}

pub struct EnvironmentParameters<'a> {
    pub scratch_space_len: u32,
    pub callstack_depth: usize,
    pub callee_account_properties: CalleeAccountProperties<'a>,
}
