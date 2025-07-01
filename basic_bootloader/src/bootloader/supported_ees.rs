use crate::bootloader::EVM_EE_BYTE;
use errors::{EESubsystemError, InterfaceError};
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    memory::slice_vec::SliceVec,
    system::{
        errors::{AsInterfaceError, SubsystemError},
        *,
    },
};

#[allow(type_alias_bounds)]
pub type SystemBoundEVMInterpreter<'a, S: EthereumLikeTypes> = evm_interpreter::Interpreter<'a, S>;

#[repr(u8)]
pub enum SupportedEEVMState<'a, S: EthereumLikeTypes> {
    EVM(SystemBoundEVMInterpreter<'a, S>) = EVM_EE_BYTE,
}

impl<'ee, S: EthereumLikeTypes> SupportedEEVMState<'ee, S> {
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
    ) -> Result<S::Resources, EESubsystemError> {
        match ee_version {
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::clarify_and_take_passed_resources(
                    resources_available_in_caller_frame,
                    desired_ergs_to_pass,
                )
                .map_err(SubsystemError::wrap)
            }
            _ => Err(AsInterfaceError(InterfaceError::UnsupportedExecutionEnvironment).into()),
        }
    }

    pub fn create_initial(
        ee_version: u8,
        system: &mut System<S>,
    ) -> Result<Self, EESubsystemError> {
        match ee_version {
            a if a == EVM_EE_BYTE => SystemBoundEVMInterpreter::new(system)
                .map(Self::EVM)
                .map_err(SubsystemError::wrap),
            _ => Err(AsInterfaceError(InterfaceError::UnsupportedExecutionEnvironment).into()),
        }
    }

    /// Starts executing a new frame within the current EE.
    /// initial_state contains all the necessary information - calldata, environment settings and resources passed.
    pub fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        initial_state: ExecutionEnvironmentLaunchParams<'i, S>,
        heap: SliceVec<'h, u8>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EESubsystemError> {
        match self {
            Self::EVM(evm_frame) => evm_frame
                .start_executing_frame(system, initial_state, heap)
                .map_err(SubsystemError::wrap),
        }
    }

    pub fn continue_after_external_call<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_result: CallResult<'res, S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EESubsystemError> {
        match self {
            Self::EVM(evm_frame) => evm_frame
                .continue_after_external_call(system, returned_resources, call_result)
                .map_err(SubsystemError::wrap),
        }
    }

    pub fn continue_after_deployment<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        deployment_result: DeploymentResult<'res, S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EESubsystemError> {
        match self {
            Self::EVM(evm_frame) => evm_frame
                .continue_after_deployment(system, returned_resources, deployment_result)
                .map_err(SubsystemError::wrap),
        }
    }

    pub fn prepare_for_deployment<'a>(
        ee_version: ExecutionEnvironmentType,
        system: &mut System<S>,
        deployment_parameters: DeploymentPreparationParameters<'a, S>,
    ) -> Result<
        (
            S::Resources,
            Option<ExecutionEnvironmentLaunchParams<'a, S>>,
        ),
        EESubsystemError,
    >
    where
        S::IO: IOSubsystemExt,
    {
        match ee_version {
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::prepare_for_deployment(
                    system,
                    deployment_parameters,
                )
                .map_err(SubsystemError::wrap)
            }
            _ => Err(AsInterfaceError(InterfaceError::UnsupportedExecutionEnvironment).into()),
        }
    }

    pub fn give_back_ergs(&mut self, resources: S::Resources) {
        assert!(resources.native().as_u64() == 0);
        match self {
            Self::EVM(evm_frame) => evm_frame.gas.reclaim_resources(resources),
        }
    }
}

pub mod errors {
    use evm_interpreter::errors::EvmSubsystemError;
    use zk_ee::system::errors::{SubsystemError, SubsystemErrorTypes};

    // TODO: This will eventually be extracted to a  separate EE subsystem, one to
    // rule interactions between EEs and bootloader.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct EEErrors;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum WrappedError {
        EvmError(EvmSubsystemError),
    }

    impl From<EvmSubsystemError> for WrappedError {
        fn from(v: EvmSubsystemError) -> Self {
            Self::EvmError(v)
        }
    }

    impl SubsystemErrorTypes for EEErrors {
        type Interface = InterfaceError;
        type Wrapped = WrappedError;
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum InterfaceError {
        UnsupportedExecutionEnvironment,
    }

    pub type EESubsystemError = SubsystemError<EEErrors>;
}
