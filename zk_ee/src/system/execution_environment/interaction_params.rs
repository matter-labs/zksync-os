use crate::{
    system::{system::SystemTypes, MAX_SCRATCH_SPACE_USIZE_WORDS},
    types_config::SystemIOTypesConfig,
};

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
    pub callstack_depth: usize,
}

/// All needed information for the bootloader and EEs to prepare
/// for deploying a contract.
pub struct DeploymentPreparationParameters<'a, S: SystemTypes> {
    pub address_of_deployer: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub address: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub call_scratch_space:
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], S::Allocator>>,
    pub deployment_code: &'a [u8],
    pub constructor_parameters: &'a [u8],
    pub ee_specific_deployment_processing_data:
        Option<alloc::boxed::Box<dyn core::any::Any, S::Allocator>>,
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub callstack_depth: usize,
}
