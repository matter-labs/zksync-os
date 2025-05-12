use core::{fmt::Debug, ops::Deref};

use crate::{
    system::{system::SystemTypes, CallModifier, Ergs, MAX_SCRATCH_SPACE_USIZE_WORDS},
    types_config::SystemIOTypesConfig,
};

use super::{
    DeploymentPreparationParameters, DeploymentResult, EnvironmentParameters, OSAllocator,
    OSImmutableSlice, ReturnValues,
};

/// Everything an execution environment needs to know to start execution
pub struct ExecutionEnvironmentLaunchParams<S: SystemTypes> {
    pub external_call: ExternalCallRequest<S>,
    pub environment_parameters: EnvironmentParameters<S>,
}

pub enum ExecutionEnvironmentPreemptionPoint<S: SystemTypes> {
    RequestedExternalCall(ExternalCallRequest<S>),
    RequestedDeployment(DeploymentPreparationParameters<S>),
    CompletedDeployment(CompletedDeployment<S>),
    CompletedExecution(CompletedExecution<S>),
}

pub enum TransactionEndPoint<S: SystemTypes> {
    CompletedExecution(CompletedExecution<S>),
    CompletedDeployment(CompletedDeployment<S>),
}

pub struct ExternalCallRequest<S: SystemTypes> {
    pub available_resources: S::Resources,
    pub ergs_to_pass: Ergs,
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callee: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub modifier: CallModifier,
    pub calldata: OSImmutableSlice<S>,
    /// Base tokens attached to this call.
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub call_scratch_space:
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], OSAllocator<S>>>,
}

impl<S: SystemTypes> ExternalCallRequest<S> {
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

impl<S: SystemTypes> Debug for ExternalCallRequest<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExternalCallRequest")
            .field("available_resources", &self.available_resources)
            .field("ergs_to_pass", &self.ergs_to_pass)
            .field("caller", &self.caller)
            .field("callee", &self.callee)
            .field("callers_caller", &self.callers_caller)
            .field("modifier", &self.modifier)
            .field("calldata", &self.calldata.deref())
            .field("nominal_token_value", &self.nominal_token_value)
            .field("call_scratch_space", &self.call_scratch_space)
            .finish()
    }
}
