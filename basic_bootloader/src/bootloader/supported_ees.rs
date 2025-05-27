use crate::bootloader::EVM_EE_BYTE;
use errors::FatalError;
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{
        errors::{InternalError, SystemError},
        *,
    },
};

#[allow(type_alias_bounds)]
pub type SystemBoundEVMInterpreter<S: EthereumLikeTypes> = evm_interpreter::Interpreter<S>;

#[repr(u8)]
pub enum SupportedEEVMState<S: EthereumLikeTypes> {
    EVM(SystemBoundEVMInterpreter<S>) = EVM_EE_BYTE,
}

impl<S: EthereumLikeTypes> SupportedEEVMState<S> {
    pub fn needs_scratch_space(&self) -> bool {
        match self {
            SupportedEEVMState::EVM(..) => SystemBoundEVMInterpreter::<S>::NEEDS_SCRATCH_SPACE,
        }
    }

    pub fn ee_type(&self) -> ExecutionEnvironmentType {
        match self {
            Self::EVM(..) => ExecutionEnvironmentType::EVM,
        }
    }

    pub fn ee_version(&self) -> u8 {
        match self {
            Self::EVM(..) => ExecutionEnvironmentType::EVM as u8,
        }
    }

    pub fn clarify_and_take_passed_resources(
        ee_version: ExecutionEnvironmentType,
        resources_available_in_caller_frame: &mut S::Resources,
        desired_ergs_to_pass: Ergs,
    ) -> Result<S::Resources, SystemError> {
        match ee_version {
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::clarify_and_take_passed_resources(
                    resources_available_in_caller_frame,
                    desired_ergs_to_pass,
                )
            }
            _ => Err(InternalError("Unsupported EE").into()),
        }
    }

    pub fn create_initial(
        ee_version: u8,
        system: &mut System<S>,
    ) -> Result<SupportedEEVMState<S>, InternalError> {
        match ee_version {
            a if a == EVM_EE_BYTE => SystemBoundEVMInterpreter::<S>::new(system).map(Self::EVM),
            _ => Err(InternalError("Unknown EE")),
        }
    }

    /// Starts executing a new frame within the current EE.
    /// initial_state contains all the necessary information - calldata, environment settings and resources passed.
    pub fn start_executing_frame(
        &mut self,
        system: &mut System<S>,
        initial_state: ExecutionEnvironmentLaunchParams<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<S>, FatalError> {
        match self {
            Self::EVM(evm_frame) => evm_frame.start_executing_frame(system, initial_state),
        }
    }

    pub fn continue_after_external_call(
        &mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_result: CallResult<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<S>, FatalError> {
        match self {
            Self::EVM(evm_frame) => {
                evm_frame.continue_after_external_call(system, returned_resources, call_result)
            }
        }
    }

    pub fn continue_after_deployment(
        &mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        deployment_result: DeploymentResult<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<S>, FatalError> {
        match self {
            Self::EVM(evm_frame) => {
                evm_frame.continue_after_deployment(system, returned_resources, deployment_result)
            }
        }
    }

    pub fn prepare_for_deployment(
        ee_version: ExecutionEnvironmentType,
        system: &mut System<S>,
        deployment_parameters: DeploymentPreparationParameters<S>,
    ) -> Result<(S::Resources, Option<ExecutionEnvironmentLaunchParams<S>>), SystemError>
    where
        S::IO: IOSubsystemExt,
    {
        match ee_version {
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::prepare_for_deployment(
                    system,
                    deployment_parameters,
                )
            }
            _ => Err(InternalError("Unsupported EE").into()),
        }
    }

    pub fn give_back_ergs(&mut self, resources: S::Resources) {
        assert!(resources.native().as_u64() == 0);
        match self {
            Self::EVM(evm_frame) => evm_frame.resources.reclaim(resources),
        }
    }
}
