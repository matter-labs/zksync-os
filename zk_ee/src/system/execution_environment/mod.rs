//! We want a very simple trait about execution environment.
//! It's simple in the sense that many of its functions
//! will be delegated back to the system itself.
//! We also want this trait to be object-safe to express that
//! it's a black box, but may be one of many such black boxes.

pub mod call_params;
pub mod environment_state;
pub mod evm;
use core::any::Any;

pub use self::call_params::*;
pub use self::environment_state::*;

use super::errors::internal::InternalError;
use super::errors::subsystem::Subsystem;
use super::errors::subsystem::SubsystemError;
use super::system::System;
use super::system::SystemTypes;
use super::tracer::Tracer;
use super::IOSubsystemExt;
use crate::common_structs::CalleeAccountProperties;
use crate::internal_error;
use crate::memory::slice_vec::SliceVec;

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
    /// Pre-checks and operations that should not be rolled back if actual frame execution fails.
    ///
    fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        system: &mut System<S>,
        frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<bool, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt;

    ///
    /// Start the execution of an EE frame in a given initial state.
    /// Returns a preemption point for the runner to handle.
    ///
    fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        frame_state: ExecutionEnvironmentLaunchParams<'i, S>,
        heap: SliceVec<'h, u8>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt;

    ///
    /// EE can decide how to provide resources to the callee frame on external call.
    /// Returns resources for the callee frame. Native resource handled by OS itself.
    ///
    fn calculate_resources_passed_in_external_call(
        resources_in_caller_frame: &mut S::Resources,
        call_request: &ExternalCallRequest<S>,
        callee_account_properties: &CalleeAccountProperties,
    ) -> Result<S::Resources, Self::SubsystemError>;

    ///
    /// Continue the execution of an EE frame after preemtion.
    /// Returns a preemption point for the runner to handle.
    ///
    fn continue_after_preemption<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_request_result: CallResult<'res, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt;
}
