use core::fmt::Debug;

use crate::{
    memory::slice_vec::SliceVec,
    system::{system::SystemTypes, CallModifier, Ergs, MAX_SCRATCH_SPACE_USIZE_WORDS},
    types_config::SystemIOTypesConfig,
};

use super::{
    DeploymentPreparationParameters, DeploymentResult, EnvironmentParameters, ReturnValues,
};

/// Everything an execution environment needs to know to start execution
pub struct ExecutionEnvironmentLaunchParams<'a, S: SystemTypes> {
    pub external_call: ExternalCallRequest<'a, S>,
    pub environment_parameters: EnvironmentParameters<'a>,
}

pub enum ExecutionEnvironmentPreemptionPoint<'a, S: SystemTypes> {
    Spawn {
        request: ExecutionEnvironmentSpawnRequest<'a, S>,
        heap: SliceVec<'a, u8>,
    },
    End(TransactionEndPoint<'a, S>),
}

pub enum ExecutionEnvironmentSpawnRequest<'a, S: SystemTypes> {
    RequestedExternalCall(ExternalCallRequest<'a, S>),
    RequestedDeployment(DeploymentPreparationParameters<'a, S>),
}

impl<S: SystemTypes> Default for ExecutionEnvironmentSpawnRequest<'_, S>
where
    S::Resources: Default,
{
    fn default() -> Self {
        Self::RequestedExternalCall(ExternalCallRequest {
            available_resources: S::Resources::default(),
            ergs_to_pass: Ergs::default(),
            caller: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            callee: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            modifier: CallModifier::NoModifier,
            calldata: &[],
            nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue::default(),
            call_scratch_space: None,
        })
    }
}

pub enum TransactionEndPoint<'a, S: SystemTypes> {
    CompletedExecution(CompletedExecution<'a, S>),
    CompletedDeployment(CompletedDeployment<'a, S>),
}

#[derive(Clone)]
pub struct ExternalCallRequest<'a, S: SystemTypes> {
    pub available_resources: S::Resources,
    pub ergs_to_pass: Ergs,
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callee: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub modifier: CallModifier,
    pub calldata: &'a [u8],
    /// Base tokens attached to this call.
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub call_scratch_space:
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], S::Allocator>>,
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
    pub return_values: ReturnValues<'a, S>,
    pub reverted: bool,
}

pub struct CompletedDeployment<'a, S: SystemTypes> {
    pub resources_returned: S::Resources,
    pub deployment_result: DeploymentResult<'a, S>,
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
            .field("calldata", &self.calldata)
            .field("nominal_token_value", &self.nominal_token_value)
            .field("call_scratch_space", &self.call_scratch_space)
            .finish()
    }
}
