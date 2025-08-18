use crate::bootloader::EVM_EE_BYTE;
use errors::{EESubsystemError, InterfaceError};
use zk_ee::{
    common_structs::CalleeAccountProperties,
    execution_environment_type::ExecutionEnvironmentType,
    interface_error,
    memory::slice_vec::SliceVec,
    system::{tracer::Tracer, *},
    wrap_error,
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

    pub fn calculate_resources_passed_in_external_call(
        ee_version: ExecutionEnvironmentType,
        resources_in_caller_frame: &mut S::Resources,
        call_request: &ExternalCallRequest<S>,
        callee_account_properties: &CalleeAccountProperties,
    ) -> Result<S::Resources, EESubsystemError> {
        match ee_version {
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::calculate_resources_passed_in_external_call(
                    resources_in_caller_frame,
                    call_request,
                    callee_account_properties,
                )
                .map_err(wrap_error!())
            }
            ExecutionEnvironmentType::NoEE => Err(interface_error!(
                InterfaceError::UnsupportedExecutionEnvironment
            )),
        }
    }

    pub fn create_initial(
        ee_type: ExecutionEnvironmentType,
        system: &mut System<S>,
    ) -> Result<Self, EESubsystemError> {
        match ee_type {
            ExecutionEnvironmentType::EVM => SystemBoundEVMInterpreter::new(system)
                .map(Self::EVM)
                .map_err(wrap_error!()),
            ExecutionEnvironmentType::NoEE => Err(interface_error!(
                InterfaceError::UnsupportedExecutionEnvironment
            )),
        }
    }

    /// Pre-checks and operations that should not be rolled back if call fails
    pub fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        ee_version: ExecutionEnvironmentType,
        system: &mut System<S>,
        frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<bool, EESubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        match ee_version {
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::before_executing_frame(system, frame_state, tracer)
                    .map_err(wrap_error!())
            }
            ExecutionEnvironmentType::NoEE => Err(interface_error!(
                InterfaceError::UnsupportedExecutionEnvironment
            )),
        }
    }

    /// Starts executing a new frame within the current EE.
    /// initial_state contains all the necessary information - calldata, environment settings and resources passed.
    pub fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        initial_state: ExecutionEnvironmentLaunchParams<'i, S>,
        heap: SliceVec<'h, u8>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EESubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        match self {
            Self::EVM(evm_frame) => evm_frame
                .start_executing_frame(system, initial_state, heap, tracer)
                .map_err(wrap_error!()),
        }
    }

    pub fn continue_after_preemption<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_result: CallResult<'res, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EESubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        match self {
            Self::EVM(evm_frame) => evm_frame
                .continue_after_preemption(system, returned_resources, call_result, tracer)
                .map_err(wrap_error!()),
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
    use zk_ee::define_subsystem;

    // TODO: This will eventually be extracted to a  separate EE subsystem, one to
    // rule interactions between EEs and bootloader.
    //
    define_subsystem!(
        EE,
        interface InterfaceError {
            UnsupportedExecutionEnvironment,
        },
        cascade WrappedError {
            EvmError(EvmSubsystemError),
        }
    );
}
