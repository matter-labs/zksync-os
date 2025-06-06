use core::fmt::Debug;

use crate::{
    system::{system::SystemTypes, CallModifier, Ergs, MAX_SCRATCH_SPACE_USIZE_WORDS},
    types_config::SystemIOTypesConfig,
};

use super::{
    DeploymentPreparationParameters, DeploymentResult, EnvironmentParameters, OSAllocator,
    ReturnValues,
};

/// Everything an execution environment needs to know to start execution
pub struct ExecutionEnvironmentLaunchParams<'a, S: SystemTypes> {
    pub external_call: ExternalCallRequest<'a, S>,
    pub environment_parameters: EnvironmentParameters<'a>,
}

pub enum ExecutionEnvironmentPreemptionPoint<'a, S: SystemTypes> {
    Spawn(ExecutionEnvironmentSpawnRequest<'a, S>),
    End(TransactionEndPoint<S>),
}

pub enum ExecutionEnvironmentSpawnRequest<'a, S: SystemTypes> {
    RequestedExternalCall(ExternalCallRequest<'a, S>),
    RequestedDeployment(DeploymentPreparationParameters<'a, S>),
}

pub enum TransactionEndPoint<S: SystemTypes> {
    CompletedExecution(CompletedExecution<S>),
    CompletedDeployment(CompletedDeployment<S>),
}

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
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], OSAllocator<S>>>,
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

pub struct CompletedExecution<S: SystemTypes> {
    pub resources_returned: S::Resources,
    pub return_values: ReturnValues<S>,
    pub reverted: bool,
}

pub struct CompletedDeployment<S: SystemTypes> {
    pub resources_returned: S::Resources,
    pub deployment_result: DeploymentResult<S>,
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
