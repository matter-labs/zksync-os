use crate::{
    system::{system::SystemTypes, MAX_SCRATCH_SPACE_USIZE_WORDS},
    types_config::SystemIOTypesConfig,
};

use super::ReturnValues;

pub enum Bytecode<'a> {
    Decommitted {
        bytecode: &'a [u8],
        unpadded_code_len: u32,
        artifacts_len: u32,
        code_version: u8,
    },
    Constructor(&'a [u8]),
}

pub struct EnvironmentParameters<'a> {
    pub bytecode: Bytecode<'a>,
    pub scratch_space_len: u32,
}

/// All needed information for the bootloader and EEs to prepare
/// for deploying a contract.
pub struct DeploymentPreparationParameters<'a, S: SystemTypes> {
    pub address_of_deployer: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub call_scratch_space:
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], S::Allocator>>,
    pub deployment_code: &'a [u8],
    pub constructor_parameters: &'a [u8],
    pub ee_specific_deployment_processing_data:
        Option<alloc::boxed::Box<dyn core::any::Any, S::Allocator>>,
    pub deployer_full_resources: S::Resources,
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub deployer_nonce: Option<u64>,
}

/// Result of an attempted deployment.
pub enum DeploymentResult<'a, S: SystemTypes> {
    /// Deployment failed after preparation.
    Failed {
        return_values: ReturnValues<'a, S>,
        execution_reverted: bool,
    },
    /// Deployment succeeded.
    Successful {
        deployed_code: &'a [u8],
        return_values: ReturnValues<'a, S>,
        deployed_at: <S::IOTypes as SystemIOTypesConfig>::Address,
    },
}

impl<'a, S: SystemTypes> DeploymentResult<'a, S> {
    pub fn has_scratch_space(&self) -> bool {
        match self {
            DeploymentResult::Failed { return_values, .. }
            | DeploymentResult::Successful { return_values, .. } => {
                return_values.return_scratch_space.is_some()
            }
        }
    }

    pub fn returndata(&self) -> &'a [u8] {
        match self {
            DeploymentResult::Failed { return_values, .. }
            | DeploymentResult::Successful { return_values, .. } => return_values.returndata,
        }
    }
}
