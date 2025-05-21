use crate::bootloader::constants::SPECIAL_ADDRESS_SPACE_BOUND;
use crate::bootloader::supported_ees::SupportedEEVMState;
use crate::bootloader::DEBUG_OUTPUT;
use core::fmt::Write;
use either::Either;
use either::Either::Left;
use either::Either::Right;
use errors::CallPreparationError;
use errors::FatalError;
use evm_interpreter::gas_constants::CALLVALUE;
use evm_interpreter::gas_constants::CALL_STIPEND;
use evm_interpreter::gas_constants::NEWACCOUNT;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::B160;
use ruint::aliases::U256;
use system_hooks::addresses_constants::BOOTLOADER_FORMAL_ADDRESS;
use system_hooks::*;
use zk_ee::common_structs::CalleeParameters;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::stack_trait::Stack;
use zk_ee::system::{
    errors::{InternalError, SystemError, UpdateQueryError},
    logger::Logger,
    *,
};

use super::StackFrame;

///
/// Type representing main loop's control flow.
/// The main loop always matches against the [preemption_reason]
/// until the callstack is empty (when we break out of the loop).
/// This type is introduced in order to be able to decouple
/// the looping and control flow from the actual logic to handle
/// the preemption point.
///
enum ControlFlow<S: EthereumLikeTypes> {
    /// Break out of the loop with the passed exit state.
    Break(TransactionEndPoint<S>),
    /// Just assign the new [preemption_reason].
    Normal(ExecutionEnvironmentPreemptionPoint<S>),
}

///
/// Helper to revert the caller's frame in case of a failure
/// while preparing to execute an external call.
/// If [callstack] is empty, then we're in the entry frame.
///
fn fail_external_call<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    finish_callee_frame: bool,
    mut resources_returned: S::Resources,
    callee_handle: Option<&SystemFrameSnapshot<S>>,
) -> Result<ControlFlow<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    resources_returned.exhaust_ergs();
    match callstack.top() {
        None => Ok(ControlFlow::Break(TransactionEndPoint::CompletedExecution(
            CompletedExecution {
                return_values: ReturnValues::empty(system),
                resources_returned,
                reverted: true,
            },
        ))),
        Some(frame) => {
            if finish_callee_frame {
                system.finish_global_frame(callee_handle)?
            }
            Ok(ControlFlow::Normal(frame.vm.continue_after_external_call(
                system,
                resources_returned,
                CallResult::CallFailedToExecute,
            )?))
        }
    }
}

///
/// Helper to handle a call result, which might need to be passed
/// to the caller's frame.
/// If [callstack] is empty, then we're in the entry frame, so we break
/// out of the main loop with an immediate result.
///
fn halt_or_continue_after_external_call<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    returned_resources: S::Resources,
    call_result: CallResult<S>,
) -> Result<ControlFlow<S>, FatalError>
where
    S::Memory: MemorySubsystemExt,
{
    match callstack.top() {
        None => {
            let (return_values, reverted) = match call_result {
                CallResult::Failed { return_values } => (return_values, true),
                CallResult::Successful { return_values } => (return_values, false),
                _ => unreachable!(),
            };
            Ok(ControlFlow::Break(TransactionEndPoint::CompletedExecution(
                CompletedExecution {
                    return_values,
                    reverted,
                    resources_returned: returned_resources,
                },
            )))
        }
        Some(frame) => Ok(ControlFlow::Normal(frame.vm.continue_after_external_call(
            system,
            returned_resources,
            call_result,
        )?)),
    }
}

///
/// Helper to handle a deployment result, which might need to be passed
/// to the caller's frame.
/// If [callstack] is empty, then we're in the entry frame, so we break
/// out of the main loop with an immediate result.
///
fn halt_or_continue_after_deployment<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    resources_returned: S::Resources,
    deployment_result: DeploymentResult<S>,
) -> Result<ControlFlow<S>, FatalError>
where
    S::Memory: MemorySubsystemExt,
{
    match callstack.top() {
        None => {
            // the final frame isn't finished because the caller will want to look at it
            Ok(ControlFlow::Break(
                TransactionEndPoint::CompletedDeployment(CompletedDeployment {
                    deployment_result,
                    resources_returned,
                }),
            ))
        }
        Some(frame) => Ok(ControlFlow::Normal(frame.vm.continue_after_deployment(
            system,
            resources_returned,
            deployment_result,
        )?)),
    }
}

///
/// Main execution loop.
/// Expects the caller to start and close the entry frame.
///
pub fn run_till_completion<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    initial_request: ExecutionEnvironmentPreemptionPoint<S>,
) -> Result<TransactionEndPoint<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    assert!(callstack.is_empty());

    // NOTE: we do not need to make a new frame as we are in the root already

    let _ = system
        .get_logger()
        .write_fmt(format_args!("Begin execution\n"));

    // This the main loop -- if we will keep running the EEs, until they complete execution or yield.
    // if they yield requesting external call or deployment, this loop will put the current entry on the callstack
    // and execute that external call.
    // When external call is finished, it would pop the latest entry from the callstack, and continue the execution.
    // It will break the loop, when the callstack is empty.
    dispatch_preemption_reason::<S, CS>(
        callstack,
        system,
        hooks,
        initial_request,
        initial_ee_version,
    )
}

///
/// Calls the right handler for the preemption reason and
/// returns the next control flow for the main loop.
///
#[inline(always)]
fn dispatch_preemption_reason<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    preemption_reason: ExecutionEnvironmentPreemptionPoint<S>,
    initial_ee_version: ExecutionEnvironmentType,
) -> Result<TransactionEndPoint<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    const SPECIAL_ADDRESS_BOUND: B160 = B160::from_limbs([SPECIAL_ADDRESS_SPACE_BOUND, 0, 0]);

    match match preemption_reason {
        // Contract requested a call to another contract (that potentially uses a different Execution Environment).
        ExecutionEnvironmentPreemptionPoint::RequestedExternalCall(call_request) => {
            handle_requested_external_call::<S, CS>(
                callstack,
                system,
                hooks,
                initial_ee_version,
                SPECIAL_ADDRESS_BOUND,
                call_request,
            )
        }
        ExecutionEnvironmentPreemptionPoint::RequestedDeployment(deployment_parameters) => {
            handle_requested_deployment::<S, CS>(
                callstack,
                system,
                deployment_parameters,
                initial_ee_version,
            )
        }
        ExecutionEnvironmentPreemptionPoint::CompletedExecution(CompletedExecution {
            return_values,
            resources_returned,
            reverted,
        }) => handle_completed_execution::<S, CS>(
            callstack,
            system,
            return_values,
            resources_returned,
            reverted,
        ),
        ExecutionEnvironmentPreemptionPoint::CompletedDeployment(CompletedDeployment {
            deployment_result,
            resources_returned,
        }) => handle_completed_deployment::<S, CS>(
            callstack,
            system,
            deployment_result,
            resources_returned,
        ),
    }? {
        ControlFlow::Normal(exit_state) => {
            dispatch_preemption_reason(callstack, system, hooks, exit_state, initial_ee_version)
        }
        ControlFlow::Break(exit_state) => Ok(exit_state),
    }
}

#[inline(always)]
fn handle_requested_external_call<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    special_address_bound: B160,
    call_request: ExternalCallRequest<S>,
) -> Result<ControlFlow<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
    #[cfg(not(target_arch = "riscv32"))]
    {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("External call to {:?}\n", call_request.callee));

        let _ = system.get_logger().write_fmt(format_args!(
            "External call with parameters:\n{:?}",
            &call_request,
        ));
    }

    // By default, code execution is disabled for calls in kernel space
    // (< special_address_bound). These calls will either be handled by
    // a system hook or behave like calls to an empty account otherwise.
    //
    // If the [code_in_kernel_space] feature is enabled, only calls to
    // addresses linked to a hook are considered special. Any other call
    // can execute code following the normal flow.
    //
    // NB: if we decide to make the latter behaviour the default, we
    // should refactor the logic to avoid the duplicated lookup into
    // the hook storage.
    #[cfg(not(feature = "code_in_kernel_space"))]
    let is_call_to_special_address =
        call_request.callee.as_uint() < special_address_bound.as_uint();

    #[cfg(feature = "code_in_kernel_space")]
    let is_call_to_special_address = call_request.callee.as_uint()
        < special_address_bound.as_uint()
        && hooks.has_hook_for(call_request.callee.as_limbs()[0] as u16);

    // The call is targeting the "system contract" space.
    if is_call_to_special_address {
        return handle_requested_external_call_to_special_address_space::<S, CS>(
            callstack,
            system,
            hooks,
            initial_ee_version,
            call_request,
        );
    }

    let is_entry_frame = callstack.top().is_none();

    // NOTE: on external call request caller doesn't spend resources,
    // but indicates how much he would want to pass at most. Here we can decide the rest

    // we should create next EE and push to callstack
    // only system knows next EE version

    // NOTE: we should move to the frame of the CALLEE now, even though we still use resources of
    // CALLER to perform some reads. If we bail, then we will roll back the frame and all
    // potential writes below, otherwise we will pass what's needed to caller

    // On entry frame we don't need to start a new frame for call
    let rollback_handle = (!is_entry_frame)
        .then(|| {
            system
                .start_global_frame()
                .map_err(|_| InternalError("must start a new frame for external call"))
        })
        .transpose()?;
    let should_finish_callee_frame_on_error = !is_entry_frame;

    let ee_type = match callstack.top() {
        Some(frame) => frame.vm.ee_type(),
        None => initial_ee_version,
    };

    let CallPreparationResult {
        next_ee_version,
        bytecode,
        bytecode_len,
        artifacts_len,
        actual_resources_to_pass,
    } = match run_call_preparation(
        callstack,
        system,
        ee_type,
        &call_request,
        should_finish_callee_frame_on_error,
        rollback_handle.as_ref(),
    ) {
        Ok(Left(r)) => r,
        Ok(Right(control_flow)) => return Ok(control_flow),
        Err(e) => return Err(e),
    };

    // If callee is empty - we should treat it as EOA.

    let calldata_slice = &call_request.calldata;
    let _ = system.get_logger().write_fmt(format_args!("Calldata = "));
    let _ = system.get_logger().log_data(calldata_slice.iter().copied());

    if bytecode.len() > 0 {
        if DEBUG_OUTPUT {
            let _ = system.get_logger().write_fmt(format_args!(
                "Bytecode len for `callee` = {}\n",
                bytecode.len(),
            ));
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Bytecode for `callee` = "));
            let _ = system
                .get_logger()
                .log_data(bytecode.as_ref().iter().copied());
        }

        // resources are checked and spent, so we continue with actual transition of control flow
        {
            // now grow callstack and prepare initial state
            let new_vm = SupportedEEVMState::create_initial(next_ee_version, system)?;
            match callstack.try_push(StackFrame::new(
                new_vm,
                rollback_handle.map(super::FrameRollbackHandle::Call),
            )) {
                Ok(_) => (),
                Err(_) => {
                    system
                        .finish_global_frame(None)
                        .map_err(|_| InternalError("must finish execution frame"))?;
                    let return_values = ReturnValues::empty(system);
                    return halt_or_continue_after_external_call(
                        callstack,
                        system,
                        actual_resources_to_pass,
                        CallResult::Failed { return_values },
                    );
                }
            }

            Ok(ControlFlow::Normal(
                callstack.top().unwrap().vm.start_executing_frame(
                    system,
                    ExecutionEnvironmentLaunchParams {
                        external_call: ExternalCallRequest {
                            available_resources: actual_resources_to_pass,
                            ..call_request
                        },
                        environment_parameters: EnvironmentParameters {
                            decommitted_bytecode: bytecode,
                            bytecode_len,
                            scratch_space_len: artifacts_len,
                        },
                    },
                )?,
            ))
        }
    } else {
        if !is_entry_frame {
            system
                .finish_global_frame(None)
                .map_err(|_| InternalError("must finish execution frame"))?;
        }
        let return_values = ReturnValues::empty(system);

        Ok(halt_or_continue_after_external_call(
            callstack,
            system,
            actual_resources_to_pass,
            CallResult::Successful { return_values },
        )?)
    }
}

#[inline(always)]
fn handle_requested_external_call_to_special_address_space<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    mut call_request: ExternalCallRequest<S>,
) -> Result<ControlFlow<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    let callee = call_request.callee;
    let address_low = callee.as_limbs()[0] as u16;

    let is_entry_frame = callstack.top().is_none();

    let _ = system.get_logger().write_fmt(format_args!(
        "Call to special address 0x{:04x}\n",
        address_low
    ));
    let calldata_slice = &call_request.calldata;
    let calldata_iter = calldata_slice.iter().copied();
    let _ = system.get_logger().write_fmt(format_args!("Calldata = "));
    let _ = system.get_logger().log_data(calldata_iter);

    // On entry frame we don't need to start a new frame for call
    let rollback_handle = (!is_entry_frame)
        .then(|| {
            system
                .start_global_frame()
                .map_err(|_| InternalError("must start a new frame for external call"))
        })
        .transpose()?;

    let should_finish_callee_frame_on_error = !is_entry_frame;

    // Transfers are forbidden to kernel space unless for bootloader or
    // explicitly allowed by a feature
    let transfer_allowed =
        callee == BOOTLOADER_FORMAL_ADDRESS || cfg!(feature = "transfers_to_kernel_space");

    let positive_value = !call_request.nominal_token_value.is_zero();

    if !transfer_allowed {
        call_request.nominal_token_value = U256::ZERO;
    }

    let ee_type = match callstack.top() {
        Some(frame) => frame.vm.ee_type(),
        None => initial_ee_version,
    };

    let CallPreparationResult {
        actual_resources_to_pass,
        ..
    } = match run_call_preparation(
        callstack,
        system,
        ee_type,
        &call_request,
        should_finish_callee_frame_on_error,
        rollback_handle.as_ref(),
    ) {
        Ok(Left(r)) => r,
        Ok(Right(control_flow)) => return Ok(control_flow),
        Err(e) => return Err(e),
    };

    let res = hooks.try_intercept(
        address_low,
        ExternalCallRequest {
            available_resources: actual_resources_to_pass.clone(),
            ..call_request
        },
        ee_type as u8,
        system,
    )?;
    if let Some(system_hook_run_result) = res {
        let CompletedExecution {
            return_values,
            resources_returned,
            reverted,
            ..
        } = system_hook_run_result;

        let _ = system.get_logger().write_fmt(format_args!(
            "Call to special address returned, success = {}\n",
            !reverted
        ));

        let returndata_slice = &return_values.returndata;
        let returndata_iter = returndata_slice.iter().copied();
        let _ = system.get_logger().write_fmt(format_args!("Returndata = "));
        let _ = system.get_logger().log_data(returndata_iter);

        if !is_entry_frame {
            system
                .finish_global_frame(reverted.then_some(&rollback_handle.unwrap()))
                .map_err(|_| InternalError("must finish execution frame"))?;
        }

        // return control and no need to create a new frame
        let call_result = if reverted {
            CallResult::Failed { return_values }
        } else {
            CallResult::Successful { return_values }
        };

        Ok(halt_or_continue_after_external_call(
            callstack,
            system,
            resources_returned,
            call_result,
        )?)
    } else {
        // it's an empty account for all the purposes, or default AA
        let _ = system.get_logger().write_fmt(format_args!(
            "Call to special address was not intercepted\n",
        ));

        let resources_returned = actual_resources_to_pass;

        if !is_entry_frame {
            system
                .finish_global_frame(None)
                .map_err(|_| InternalError("must finish execution frame"))?;
        }

        // return control and no need to create a new frame
        let return_values = ReturnValues::empty(system);
        let call_result = if positive_value && !transfer_allowed {
            CallResult::Failed { return_values }
        } else {
            CallResult::Successful { return_values }
        };

        Ok(halt_or_continue_after_external_call(
            callstack,
            system,
            resources_returned,
            call_result,
        )?)
    }
}

pub struct CallPreparationResult<S: SystemTypes> {
    pub next_ee_version: u8,
    pub bytecode:
        <<S::Memory as MemorySubsystem>::ManagedRegion as OSManagedRegion>::OSManagedImmutableSlice,
    pub bytecode_len: u32,
    pub artifacts_len: u32,
    pub actual_resources_to_pass: S::Resources,
}

///
/// Reads callee account and runs call preparation function
/// from the system. Additionally, does token transfer if needed.
/// The return value is Left(preparation_result) if preparation succeeds,
/// and Right(next_control_flow) if some check fails.
///
fn run_call_preparation<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
    call_request: &ExternalCallRequest<S>,
    should_finish_callee_frame_on_error: bool,
    callee_rollback_handle: Option<&SystemFrameSnapshot<S>>,
) -> Result<Either<CallPreparationResult<S>, ControlFlow<S>>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    let is_entry_frame = callstack.top().is_none();
    let mut resources_available = call_request.available_resources.clone();

    let r = if is_entry_frame {
        // For entry frame we don't charge ergs for call preparation,
        // as this is included in the intrinsic cost.
        resources_available.with_infinite_ergs(|inf_resources| {
            cycle_marker::wrap_with_resources!("prepare_for_call", inf_resources, {
                prepare_for_call(
                    system,
                    ee_version,
                    inf_resources,
                    &call_request,
                    is_entry_frame,
                )
            })
        })
    } else {
        cycle_marker::wrap_with_resources!("prepare_for_call", resources_available, {
            prepare_for_call(
                system,
                ee_version,
                &mut resources_available,
                &call_request,
                is_entry_frame,
            )
        })
    };

    let CalleeParameters {
        next_ee_version,
        bytecode,
        bytecode_len,
        artifacts_len,
        stipend,
    } = match r {
        Ok(x) => x,
        Err(CallPreparationError::System(SystemError::OutOfErgs)) => {
            return fail_external_call(
                callstack,
                system,
                should_finish_callee_frame_on_error,
                resources_available,
                callee_rollback_handle,
            )
            .map(Either::Right)
        }
        Err(CallPreparationError::System(SystemError::OutOfNativeResources)) => {
            return Err(FatalError::OutOfNativeResources)
        }
        Err(CallPreparationError::System(SystemError::Internal(e))) => return Err(e.into()),
        Err(CallPreparationError::InsufficientBalance { stipend }) => {
            match ee_version {
                ExecutionEnvironmentType::NoEE => {
                    unreachable!("Cannot be in NoEE deep in the callstack")
                }
                ExecutionEnvironmentType::EVM => {
                    // Following EVM, a call with insufficient balance is not a revert,
                    // but rather a normal failing call.
                    // Balance is validated for first frame, we must be deeper in
                    // the callstack, so it's safe to unwrap.
                    let mut resources_to_pass: S::Resources =
                        match SupportedEEVMState::<S>::clarify_and_take_passed_resources(
                            ee_version,
                            &mut resources_available,
                            call_request.ergs_to_pass,
                        ) {
                            Ok(resources_to_pass) => resources_to_pass,
                            Err(SystemError::OutOfErgs) => {
                                return fail_external_call(
                                    callstack,
                                    system,
                                    should_finish_callee_frame_on_error,
                                    resources_available,
                                    callee_rollback_handle,
                                )
                                .map(Either::Right)
                            }
                            Err(SystemError::OutOfNativeResources) => {
                                return Err(FatalError::OutOfNativeResources)
                            }
                            Err(SystemError::Internal(error)) => {
                                return Err(error.into());
                            }
                        };
                    // Give remaining ergs back to caller
                    callstack
                        .top()
                        .unwrap()
                        .vm
                        .give_back_ergs(resources_available);
                    // Add stipend
                    if let Some(stipend) = stipend {
                        resources_to_pass.add_ergs(stipend)
                    }
                    if should_finish_callee_frame_on_error {
                        system
                            .finish_global_frame(None)
                            .map_err(|_| InternalError("must finish execution frame"))?;
                    }
                    let return_values = ReturnValues::empty(system);
                    let return_values = CallResult::Failed { return_values };
                    return Ok(Right(halt_or_continue_after_external_call(
                        callstack,
                        system,
                        resources_to_pass,
                        return_values,
                    )?));
                }
                _ => return Err(InternalError("Unsupported EE").into()),
            }
        }
    };

    // If we're in the entry frame, i.e. not the execution of a CALL opcode,
    // we don't apply the CALL-specific gas charging, but instead set
    // actual_resources_to_pass equal to the available resources
    let mut actual_resources_to_pass = if !is_entry_frame {
        // now we should ask current EE for observable resource behavior if needed
        {
            let to_pass = match SupportedEEVMState::<S>::clarify_and_take_passed_resources(
                ee_version,
                &mut resources_available,
                call_request.ergs_to_pass,
            ) {
                Ok(x) => x,
                Err(SystemError::OutOfErgs) => {
                    return fail_external_call(
                        callstack,
                        system,
                        should_finish_callee_frame_on_error,
                        resources_available,
                        callee_rollback_handle,
                    )
                    .map(Either::Right)
                }
                Err(SystemError::OutOfNativeResources) => {
                    return Err(FatalError::OutOfNativeResources)
                }
                Err(SystemError::Internal(error)) => {
                    return Err(error.into());
                }
            };
            // Give remaining ergs back to caller
            callstack
                .top()
                .unwrap()
                .vm
                .give_back_ergs(resources_available);
            to_pass
        }
    } else {
        resources_available.take()
    };

    // Add stipend
    if let Some(stipend) = stipend {
        actual_resources_to_pass.add_ergs(stipend)
    }
    Ok(Left(CallPreparationResult {
        next_ee_version,
        bytecode,
        bytecode_len,
        artifacts_len,
        actual_resources_to_pass,
    }))
}

// TODO: all the gas computation in this function seems very EVM-specific.
// It should be split into EVM and generic part.
/// Run call preparation, which includes reading the callee parameters,
/// performing a token transfer and charging for resources.
fn prepare_for_call<S: EthereumLikeTypes>(
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
    resources: &mut S::Resources,
    call_request: &ExternalCallRequest<S>,
    is_entry_frame: bool,
) -> Result<CalleeParameters<S>, CallPreparationError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    // IO will follow the rules of the CALLER (`initial_ee_version`) here to charge for execution
    let account_properties = match system.io.read_account_properties(
        ee_version,
        resources,
        &call_request.callee,
        AccountDataRequest::empty()
            .with_ee_version()
            .with_bytecode_len()
            .with_artifacts_len()
            .with_bytecode()
            .with_nonce()
            .with_nominal_token_balance(),
    ) {
        Ok(account_properties) => account_properties,
        Err(SystemError::OutOfErgs) => {
            let _ = system.get_logger().write_fmt(format_args!(
                "Call failed: insufficient resources to read callee account data\n",
            ));
            return Err(CallPreparationError::System(SystemError::OutOfErgs));
        }
        Err(SystemError::OutOfNativeResources) => {
            return Err(CallPreparationError::System(
                SystemError::OutOfNativeResources,
            ))
        }
        Err(SystemError::Internal(e)) => return Err(CallPreparationError::System(e.into())),
    };

    // Now we charge for the rest of the CALL related costs
    let stipend = if !is_entry_frame {
        match ee_version {
            ExecutionEnvironmentType::EVM => {
                let is_delegate = call_request.is_delegate();
                let is_callcode = call_request.is_callcode();
                let is_callcode_or_delegate = is_callcode || is_delegate;

                // Positive value cost and stipend
                let stipend = if !is_delegate && !call_request.nominal_token_value.is_zero() {
                    // TODO: add native cost
                    let positive_value_cost =
                        S::Resources::from_ergs(Ergs(CALLVALUE * ERGS_PER_GAS));
                    resources.charge(&positive_value_cost)?;
                    Some(Ergs(CALL_STIPEND * ERGS_PER_GAS))
                } else {
                    None
                };

                // Account creation cost
                let callee_is_empty = account_properties.nonce.0 == 0
                    && account_properties.bytecode_len.0 == 0
                    && account_properties.nominal_token_balance.0.is_zero();
                if !is_callcode_or_delegate
                    && !call_request.nominal_token_value.is_zero()
                    && callee_is_empty
                {
                    let callee_creation_cost =
                        S::Resources::from_ergs(Ergs(NEWACCOUNT * ERGS_PER_GAS));
                    resources.charge(&callee_creation_cost)?
                }

                stipend
            }
            _ => return Err(InternalError("Unsupported EE").into()),
        }
    } else {
        None
    };

    // From now on, we use infinite ergs.

    // Read required data to perform a call
    let (next_ee_version, bytecode, bytecode_len, artifacts_len) = {
        // transfer base token balance if needed
        // Note that for delegate calls, call_request.nominal_token_value might be positive,
        // but it shouldn't be transferred. This is the value used for the
        // execution context.
        if call_request.nominal_token_value != U256::ZERO && !call_request.is_delegate() {
            if !call_request.is_transfer_allowed() {
                let _ = system.get_logger().write_fmt(format_args!(
                    "Call failed: positive value with modifier {:?}\n",
                    call_request.modifier
                ));
                return Err(CallPreparationError::System(SystemError::OutOfErgs));
            }

            // Adjust transfer target due to CALLCODE
            let transfer_target = match call_request.modifier {
                CallModifier::EVMCallcode | CallModifier::EVMCallcodeStatic => call_request.caller,
                _ => call_request.callee,
            };
            match resources.with_infinite_ergs(|inf_resources| {
                system.io.transfer_nominal_token_value(
                    ExecutionEnvironmentType::NoEE,
                    inf_resources,
                    &call_request.caller,
                    &transfer_target,
                    &call_request.nominal_token_value,
                )
            }) {
                Ok(x) => x,
                Err(UpdateQueryError::System(error)) => {
                    return Err(error.into());
                }
                Err(UpdateQueryError::NumericBoundsError) => {
                    return Err(CallPreparationError::InsufficientBalance { stipend });
                }
            }
        }

        let ee_version = account_properties.ee_version.0;
        let bytecode_len = account_properties.bytecode_len.0;
        let artifacts_len = account_properties.artifacts_len.0;
        let bytecode = account_properties.bytecode.0;
        let bytecode = unsafe {
            system
                .memory
                .construct_immutable_slice_from_static_slice(bytecode)
        };

        (ee_version, bytecode, bytecode_len, artifacts_len)
    };
    Ok(CalleeParameters {
        next_ee_version,
        bytecode,
        bytecode_len,
        artifacts_len,
        stipend,
    })
}

#[inline(always)]
fn handle_requested_deployment<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    deployment_parameters: DeploymentPreparationParameters<S>,
    initial_ee_version: ExecutionEnvironmentType,
) -> Result<ControlFlow<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    // Caller gave away all it's resources into deployment parameters, and in preparation function
    // we will charge for deployment, compute address and potentially increment nonce

    let is_entry_frame = callstack.top().is_none();

    let rollback_handle_prep = (!is_entry_frame)
        .then(|| {
            system
                .start_global_frame()
                .map_err(|_| InternalError("must start a new frame for external call"))
        })
        .transpose()?;

    let ee_type = match callstack.top() {
        Some(frame) => frame.vm.ee_type(),
        None => initial_ee_version,
    };

    match SupportedEEVMState::prepare_for_deployment(ee_type, system, deployment_parameters) {
        Ok((resources_for_deployer, Some(mut new_frame))) => {
            // resources returned back to caller
            match callstack.top() {
                Some(existing_frame) => existing_frame.vm.give_back_ergs(resources_for_deployer),
                None => {
                    // resources returned back to caller do not make sense, so we join them back
                    new_frame
                        .external_call
                        .available_resources
                        .reclaim(resources_for_deployer);
                }
            }

            // Now we start a frame for the constructor.
            let rollback_handle_ctor = system
                .start_global_frame()
                .map_err(|_| InternalError("must start a new frame for init code"))?;

            // and proceed further into callgraph

            // EE made all the preparations and we are in callee's frame already
            match callstack.try_push(StackFrame::new(
                SupportedEEVMState::create_initial(ee_type as u8, system)?,
                rollback_handle_prep.map(|prep| {
                    super::FrameRollbackHandle::Deploy(super::DeploymentHandle {
                        prep,
                        ctor: rollback_handle_ctor,
                    })
                }),
            )) {
                Ok(_) => (),
                Err(_) => {
                    system
                        .finish_global_frame(None)
                        .map_err(|_| InternalError("must finish deployment frame"))?;
                    let deployment_result = DeploymentResult::Failed {
                        return_values: ReturnValues::empty(system),
                        execution_reverted: false,
                    };
                    return halt_or_continue_after_deployment(
                        callstack,
                        system,
                        new_frame.external_call.available_resources,
                        deployment_result,
                    );
                }
            }

            let nominal_token_value = new_frame.external_call.nominal_token_value;

            // EIP-161: contracts should be initialized with nonce 1
            // Note: this has to be done before we actually deploy the bytecode,
            // as constructor execution should see the deployed_address as having
            // nonce = 1
            new_frame
                .external_call
                .available_resources
                .with_infinite_ergs(|inf_resources| {
                    system.io.increment_nonce(
                        initial_ee_version,
                        inf_resources,
                        &new_frame.external_call.callee,
                        1,
                    )
                })
                // TODO: make sure we don't capture out of native
                .map_err(|e| match e {
                    UpdateQueryError::System(SystemError::OutOfNativeResources) => {
                        FatalError::OutOfNativeResources
                    }
                    _ => InternalError("Failed to set deployed nonce to 1").into(),
                })?;

            if nominal_token_value != U256::ZERO {
                new_frame
                    .external_call
                    .available_resources
                    .with_infinite_ergs(|inf_resources| {
                        system.io.transfer_nominal_token_value(
                            initial_ee_version,
                            inf_resources,
                            &new_frame.external_call.caller,
                            &new_frame.external_call.callee,
                            &nominal_token_value,
                        )
                    })
                    // TODO: make sure we don't capture out of native
                    .map_err(|e| match e {
                        UpdateQueryError::System(SystemError::OutOfNativeResources) => {
                            FatalError::OutOfNativeResources
                        }
                        _ => InternalError(
                            "Must transfer value on deployment after check in preparation",
                        )
                        .into(),
                    })?;
            }

            Ok(ControlFlow::Normal(
                callstack
                    .top()
                    .unwrap()
                    .vm
                    .start_executing_frame(system, new_frame)?,
            ))
        }
        Ok((resources_for_deployer, None)) => {
            // preparation failed, and we should finish the frame
            if callstack.top().is_some() {
                system
                    .finish_global_frame(None)
                    .map_err(|_| InternalError("must finish deployment frame"))?;
            }
            let deployment_result = DeploymentResult::Failed {
                return_values: ReturnValues::empty(system),
                execution_reverted: false,
            };
            Ok(halt_or_continue_after_deployment(
                callstack,
                system,
                resources_for_deployer,
                deployment_result,
            )?)
        }
        Err(FatalError::OutOfNativeResources) => Err(FatalError::OutOfNativeResources),
        Err(FatalError::Internal(e)) => Err(e.into()),
    }
}

#[inline(always)]
fn handle_completed_execution<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    return_values: ReturnValues<S>,
    resources_returned: S::Resources,
    reverted: bool,
) -> Result<ControlFlow<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    let _ = system.get_logger().write_fmt(format_args!(
        "Return from external call, success = {}\n",
        !reverted
    ));

    let prev_stack = callstack
        .pop()
        .ok_or(InternalError("Empty callstack on completed execution"))?;

    if let Some(current_stack) = callstack.top() {
        // Remap and pass execution back to previous frame
        system
            .finish_global_frame(
                reverted
                    .then(|| {
                        prev_stack
                            .rollback_handle
                            .as_ref()
                            .map(|x| x.as_call())
                            .transpose()
                    })
                    .transpose()?
                    .flatten(),
            )
            .map_err(|_| InternalError("must finish execution frame"))?;

        let mut return_values = return_values;
        let returndata = system
            .memory
            .copy_into_return_memory(&return_values.returndata)?;
        let returndata = returndata.take_slice(0..returndata.len());
        return_values.returndata = returndata;

        let returndata_slice = &return_values.returndata;
        let returndata_iter = returndata_slice.iter().copied();
        let _ = system.get_logger().write_fmt(format_args!("Returndata = "));
        let _ = system.get_logger().log_data(returndata_iter);

        let call_result = if reverted {
            CallResult::Failed { return_values }
        } else {
            CallResult::Successful { return_values }
        };
        Ok(ControlFlow::Normal(
            current_stack.vm.continue_after_external_call(
                system,
                resources_returned,
                call_result,
            )?,
        ))
    } else {
        // the final frame isn't finished because the caller will want to look at it
        Ok(ControlFlow::Break(TransactionEndPoint::CompletedExecution(
            CompletedExecution {
                return_values,
                resources_returned,
                reverted,
            },
        )))
    }
}

#[inline(always)]
fn handle_completed_deployment<
    S: EthereumLikeTypes,
    CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
>(
    callstack: &mut CS,
    system: &mut System<S>,
    deployment_result: DeploymentResult<S>,
    mut resources_returned: S::Resources,
) -> Result<ControlFlow<S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    let deployment_frame = callstack
        .top()
        .ok_or(InternalError("Empty callstack on completed deployment"))?;
    let deploying_vm = deployment_frame.vm.ee_type();
    let (deployment_success, reverted, mut deployment_result) = match deployment_result {
        DeploymentResult::Successful {
            bytecode,
            bytecode_len,
            artifacts_len,
            return_values,
            deployed_at,
        } => {
            // it's responsibility of the system to finish deployment. We continue to use resources from deployment frame
            match system.deploy_bytecode(
                deploying_vm,
                &mut resources_returned,
                &deployed_at,
                bytecode,
                bytecode_len,
                artifacts_len,
            ) {
                Ok(bytecode) => {
                    let deployment_result = DeploymentResult::Successful {
                        bytecode,
                        bytecode_len,
                        artifacts_len,
                        return_values: ReturnValues::empty(system),
                        deployed_at,
                    };
                    // TODO: debug implementation for Bits uses global alloc, which panics in ZKsync OS
                    #[cfg(not(target_arch = "riscv32"))]
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Successfully deployed contract at {:?} \n",
                        deployed_at
                    ));
                    (true, false, deployment_result)
                }
                Err(SystemError::OutOfErgs) => {
                    let deployment_result = DeploymentResult::Failed {
                        return_values,
                        execution_reverted: false,
                    };
                    (false, false, deployment_result)
                }
                Err(SystemError::OutOfNativeResources) => {
                    return Err(FatalError::OutOfNativeResources)
                }
                Err(SystemError::Internal(e)) => return Err(e.into()),
            }
        }
        a @ DeploymentResult::Failed { .. } => (false, false, a),
        a @ DeploymentResult::DeploymentCallFailedToExecute => (false, true, a),
    };

    let deployment_frame = callstack
        .pop()
        .ok_or(InternalError("Empty callstack on completed deployment"))?;

    let deployment_rollback = deployment_frame
        .rollback_handle
        .as_ref()
        .map(|x| x.as_deploy())
        .transpose()?;

    // Now finish constructor frame
    if !reverted {
        system.finish_global_frame(
            (!deployment_success)
                .then_some(deployment_rollback)
                .flatten()
                .map(|x| &x.ctor),
        )?;
    }

    let _ = system.get_logger().write_fmt(format_args!(
        "Return from constructor call, success = {}, reverted = {}\n",
        deployment_success, reverted
    ));

    if let Some(caller_frame) = callstack.top() {
        system.finish_global_frame(
            reverted
                .then_some(deployment_rollback)
                .flatten()
                .map(|x| &x.prep),
        )?;

        if let Some(returndata_region) = deployment_result.returndata() {
            let returndata_iter = returndata_region.iter().copied();
            let _ = system.get_logger().write_fmt(format_args!("Returndata = "));
            let _ = system.get_logger().log_data(returndata_iter);
        }

        match &mut deployment_result {
            DeploymentResult::Successful { return_values, .. } => {
                let returndata = system
                    .memory
                    .copy_into_return_memory(&return_values.returndata)?;
                let returndata = returndata.take_slice(0..returndata.len());
                return_values.returndata = returndata;
            }
            DeploymentResult::Failed { return_values, .. } => {
                let returndata = system
                    .memory
                    .copy_into_return_memory(&return_values.returndata)?;
                let returndata = returndata.take_slice(0..returndata.len());
                return_values.returndata = returndata;
            }
            _ => {}
        }

        Ok(ControlFlow::Normal(
            caller_frame.vm.continue_after_deployment(
                system,
                resources_returned,
                deployment_result,
            )?,
        ))
    } else {
        // the final frame isn't finished because the caller will want to look at it
        Ok(ControlFlow::Break(
            TransactionEndPoint::CompletedDeployment(CompletedDeployment {
                deployment_result,
                resources_returned,
            }),
        ))
    }
}
