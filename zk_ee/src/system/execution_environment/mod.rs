//! We want a very simple trait about execution environment.
//! It's simple in the sense that many of its functions
//! will be delegated back to the system itself.
//! We also want this trait to be object-safe to express that
//! it's a black box, but may be one of many such black boxes.

pub mod call_params;
pub mod environment_state;
pub mod interaction_params;
use alloc::boxed::Box;
use core::any::Any;

pub use self::call_params::*;
pub use self::environment_state::*;
pub use self::interaction_params::*;

use super::errors::internal::InternalError;
use super::errors::subsystem::Subsystem;
use super::errors::subsystem::SubsystemError;
use super::system::System;
use super::system::SystemTypes;
use super::IOSubsystemExt;
use crate::internal_error;
use crate::memory::slice_vec::SliceVec;
use crate::system::CallModifier;
use crate::system::Ergs;
use crate::types_config::*;

// we should consider some bound of amount of data that is deployment-specific,
// for now it's arbitrary
pub trait EEDeploymentExtraParameters<S: SystemTypes>: 'static + Sized + core::any::Any {
    fn from_box_dyn(src: alloc::boxed::Box<dyn Any, S::Allocator>) -> Result<Self, InternalError> {
        let box_self = src
            .downcast::<Self>()
            .map_err(|_| internal_error!("from_box_dyn"))?;
        Ok(alloc::boxed::Box::into_inner(box_self))
    }
}

///
/// Execution environment interface.
///
pub trait ExecutionEnvironment<'ee, S: SystemTypes, Es: Subsystem>: Sized {
    const NEEDS_SCRATCH_SPACE: bool;

    const EE_VERSION_BYTE: u8;

    type UsageError = <Es as Subsystem>::Interface;
    type SubsystemError = SubsystemError<Es>;

    ///
    /// Initialize a new (empty) EE state.
    ///
    fn new(system: &mut System<S>) -> Result<Self, Self::SubsystemError>;

    ///
    /// The contract address where the EE is being executed.
    ///
    fn self_address(&self) -> &<S::IOTypes as SystemIOTypesConfig>::Address;

    ///
    /// Available resources in the current frame.
    ///
    fn resources_mut(&mut self) -> &mut S::Resources;

    ///
    /// Whether this EE supports a given call modifier.
    ///
    fn is_modifier_supported(modifier: &CallModifier) -> bool;

    ///
    /// Whether the EE is running in a static context, i.e. in
    /// a context where state changes are not allowed.
    ///
    fn is_static_context(&self) -> bool;

    ///
    /// Start the execution of an EE frame in a given initial state.
    /// Returns a preemption point for the bootloader to handle.
    ///
    fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        frame_state: ExecutionEnvironmentLaunchParams<'i, S>,
        heap: SliceVec<'h, u8>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>;

    /// Continues after the bootloader handled a completed external call.
    fn continue_after_external_call<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_result: CallResult<'res, S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>;

    /// Continues after the bootloader handled a completed deployment.
    fn continue_after_deployment<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        deployment_result: DeploymentResult<'res, S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>;

    type DeploymentExtraParameters: EEDeploymentExtraParameters<S>;

    fn default_ee_deployment_options(
        system: &mut System<S>,
    ) -> Option<Box<dyn Any, <S as SystemTypes>::Allocator>>;

    ///
    /// Adjust resources passed from the caller to the callee.
    /// Some EE might have some additional rules in this situation,
    /// such as the 63/64 rule for EVM.
    ///
    fn clarify_and_take_passed_resources(
        resources_available_in_deployer_frame: &mut S::Resources,
        ergs_desired_to_pass: Ergs,
    ) -> Result<S::Resources, Self::SubsystemError>;

    /// Runs some pre-deployment preparation and checks.
    /// The result can be None to represent unsuccessful preparation for deployment.
    /// EE should prepare a new state to run as "constructor" and potentially OS/IO related data.
    /// OS then will perform it's own checks and decide whether deployment should proceed or not
    /// Returns the resources to give back to the deployer
    fn prepare_for_deployment<'a>(
        system: &mut System<S>,
        deployment_parameters: DeploymentPreparationParameters<'a, S>,
    ) -> Result<
        (
            S::Resources,
            Option<ExecutionEnvironmentLaunchParams<'a, S>>,
        ),
        Self::SubsystemError,
    >
    where
        S::IO: IOSubsystemExt;
}
