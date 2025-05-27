use crate::{
    system::{system::SystemTypes, MemorySubsystem, MAX_SCRATCH_SPACE_USIZE_WORDS},
    types_config::SystemIOTypesConfig,
};

use super::{BytecodeSource, OSImmutableSlice, ReturnValues};

pub struct EnvironmentParameters<S: SystemTypes> {
    pub decommitted_bytecode: BytecodeSource<S>,
    pub bytecode_len: u32,
    pub scratch_space_len: u32,
}

///
/// All needed information for the bootloader and EEs to prepare
/// for deploying a contract.
///
pub struct DeploymentPreparationParameters<S: SystemTypes> {
    pub address_of_deployer: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub call_scratch_space: Option<
        alloc::boxed::Box<
            [usize; MAX_SCRATCH_SPACE_USIZE_WORDS],
            <S::Memory as MemorySubsystem>::Allocator,
        >,
    >,
    pub deployment_code: OSImmutableSlice<S>,
    pub constructor_parameters: OSImmutableSlice<S>,
    pub ee_specific_deployment_processing_data:
        Option<alloc::boxed::Box<dyn core::any::Any, <S::Memory as MemorySubsystem>::Allocator>>,
    pub deployer_full_resources: S::Resources,
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub deployer_nonce: Option<u64>,
}

///
/// Result of an attempted deployment.
///
pub enum DeploymentResult<S: SystemTypes> {
    /// Preparation for deployment failed.
    DeploymentCallFailedToExecute,
    /// Deployment failed after preparation.
    Failed {
        return_values: ReturnValues<S>,
        execution_reverted: bool,
    },
    /// Deployment succeeded.
    Successful {
        bytecode: OSImmutableSlice<S>,
        bytecode_len: u32,
        artifacts_len: u32,
        return_values: ReturnValues<S>,
        deployed_at: <S::IOTypes as SystemIOTypesConfig>::Address,
    },
}

impl<S: SystemTypes> DeploymentResult<S> {
    pub fn has_scratch_space(&self) -> bool {
        match self {
            DeploymentResult::DeploymentCallFailedToExecute => false,
            DeploymentResult::Failed { return_values, .. }
            | DeploymentResult::Successful { return_values, .. } => {
                return_values.return_scratch_space.is_some()
            }
        }
    }

    pub fn returndata(&self) -> Option<&OSImmutableSlice<S>> {
        match self {
            DeploymentResult::DeploymentCallFailedToExecute => None,
            DeploymentResult::Failed { return_values, .. }
            | DeploymentResult::Successful { return_values, .. } => Some(&return_values.returndata),
        }
    }
}
