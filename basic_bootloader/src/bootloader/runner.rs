use crate::bootloader::constants::SPECIAL_ADDRESS_SPACE_BOUND;
use crate::bootloader::supported_ees::SupportedEEVMState;
use crate::bootloader::DEBUG_OUTPUT;
use core::fmt::Write;
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
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::{
    errors::{InternalError, SystemError, UpdateQueryError},
    logger::Logger,
    *,
};

use super::StackFrame;

///
/// Helper to handle a deployment result, which might need to be passed
/// to the caller's frame.
/// If [callstack] is empty, then we're in the entry frame, so we break
/// out of the main loop with an immediate result.
///
fn halt_or_continue_after_deployment<'a, S: EthereumLikeTypes>(
    callstack: &mut SliceVec<StackFrame<'a, S, SystemFrameSnapshot<S>>>,
    system: &mut System<S>,
    resources_returned: S::Resources,
    deployment_result: DeploymentResult<S>,
) -> Result<ControlFlow<'a, S>, FatalError>
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
pub fn run_till_completion<S: EthereumLikeTypes>(
    callstack: &mut SliceVec<StackFrame<S, SystemFrameSnapshot<S>>>,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    initial_request: ExecutionEnvironmentSpawnRequest<S>,
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

    match initial_request {
        ExecutionEnvironmentSpawnRequest::RequestedExternalCall(external_call_request) => {
            let (resources_returned, call_result) = handle_requested_external_call(
                None,
                system,
                hooks,
                initial_ee_version,
                external_call_request,
            )?;
            let (return_values, reverted) = match call_result {
                CallResult::CallFailedToExecute => (ReturnValues::empty(system), true),
                CallResult::Failed { return_values } => (return_values, true),
                CallResult::Successful { return_values } => (return_values, false),
            };
            Ok(TransactionEndPoint::CompletedExecution(
                CompletedExecution {
                    resources_returned,
                    return_values,
                    reverted,
                },
            ))
        }
        ExecutionEnvironmentSpawnRequest::RequestedDeployment(
            deployment_preparation_parameters,
        ) => todo!(),
    }
}

#[inline(always)]
fn handle_spawn<'a, S: EthereumLikeTypes>(
    previous_vm: &mut SupportedEEVMState<'a, S>,
    spawn: ExecutionEnvironmentSpawnRequest<S>,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    match spawn {
        ExecutionEnvironmentSpawnRequest::RequestedExternalCall(external_call_request) => {
            let rollback_handle = system
                .start_global_frame()
                .map_err(|_| InternalError("must start a new frame for external call"))?;

            let (resources, mut call_result) = handle_requested_external_call(
                Some(previous_vm),
                system,
                hooks,
                initial_ee_version,
                external_call_request,
            )?;

            let success = matches!(call_result, CallResult::Successful { .. });

            let _ = system.get_logger().write_fmt(format_args!(
                "Return from external call, success = {}\n",
                success
            ));

            system
                .finish_global_frame(if success {
                    None
                } else {
                    Some(&rollback_handle)
                })
                .map_err(|_| InternalError("must finish execution frame"))?;

            match &mut call_result {
                CallResult::Successful { return_values } | CallResult::Failed { return_values } => {
                    let returndata = system
                        .memory
                        .copy_into_return_memory(&return_values.returndata)?;
                    let returndata = returndata.take_slice(0..returndata.len());
                    return_values.returndata = returndata;

                    let returndata_iter = return_values.returndata.iter().copied();
                    let _ = system.get_logger().write_fmt(format_args!("Returndata = "));
                    let _ = system.get_logger().log_data(returndata_iter);
                }
                _ => {}
            }

            previous_vm.continue_after_external_call(system, resources, call_result)
        }
        ExecutionEnvironmentSpawnRequest::RequestedDeployment(
            deployment_preparation_parameters,
        ) => todo!(),
    }
}

const SPECIAL_ADDRESS_BOUND: B160 = B160::from_limbs([SPECIAL_ADDRESS_SPACE_BOUND, 0, 0]);

fn handle_requested_external_call<S: EthereumLikeTypes>(
    caller_vm: Option<&mut SupportedEEVMState<S>>,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    call_request: ExternalCallRequest<S>,
) -> Result<(S::Resources, CallResult<S>), FatalError>
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
    // (< SPECIAL_ADDRESS_BOUND). These calls will either be handled by
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
        call_request.callee.as_uint() < SPECIAL_ADDRESS_BOUND.as_uint();

    #[cfg(feature = "code_in_kernel_space")]
    let is_call_to_special_address = call_request.callee.as_uint()
        < SPECIAL_ADDRESS_BOUND.as_uint()
        && hooks.has_hook_for(call_request.callee.as_limbs()[0] as u16);

    // The call is targeting the "system contract" space.
    if is_call_to_special_address {
        /*return handle_requested_external_call_to_special_address_space(
            callstack,
            system,
            hooks,
            initial_ee_version,
            call_request,
        );*/
    }

    let ee_type = match &caller_vm {
        Some(vm) => vm.ee_type(),
        None => initial_ee_version,
    };

    // NOTE: on external call request caller doesn't spend resources,
    // but indicates how much he would want to pass at most. Here we can decide the rest

    // we should create next EE and push to callstack
    // only system knows next EE version

    // NOTE: we should move to the frame of the CALLEE now, even though we still use resources of
    // CALLER to perform some reads. If we bail, then we will roll back the frame and all
    // potential writes below, otherwise we will pass what's needed to caller

    match run_call_preparation(caller_vm, system, ee_type, &call_request) {
        Ok(CallPreparationResult::Success {
            next_ee_version,
            bytecode,
            bytecode_len,
            artifacts_len,
            actual_resources_to_pass,
        }) => {
            let calldata_slice = &call_request.calldata;
            let _ = system.get_logger().write_fmt(format_args!("Calldata = "));
            let _ = system.get_logger().log_data(calldata_slice.iter().copied());

            // Calls to EOAs succeed with empty return value
            if bytecode.len() == 0 {
                Ok((
                    actual_resources_to_pass,
                    CallResult::Successful {
                        return_values: ReturnValues::empty(system),
                    },
                ))
            } else {
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

                // now grow callstack and prepare initial state
                let mut new_vm =
                    Box::new(SupportedEEVMState::create_initial(next_ee_version, system)?);

                let mut preemption = new_vm.start_executing_frame(
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
                )?;

                loop {
                    match preemption {
                        ExecutionEnvironmentPreemptionPoint::Spawn(spawn) => {
                            preemption =
                                handle_spawn(&mut new_vm, spawn, system, hooks, initial_ee_version)?
                        }
                        ExecutionEnvironmentPreemptionPoint::End(
                            TransactionEndPoint::CompletedExecution(CompletedExecution {
                                resources_returned,
                                return_values,
                                reverted,
                            }),
                        ) => {
                            break Ok((
                                resources_returned,
                                if reverted {
                                    CallResult::Failed { return_values }
                                } else {
                                    CallResult::Successful { return_values }
                                },
                            ))
                        }
                        ExecutionEnvironmentPreemptionPoint::End(
                            TransactionEndPoint::CompletedDeployment(_),
                        ) => {
                            return Err(FatalError::Internal(InternalError(
                                "returned from external call as if it was a deployment",
                            )))
                        }
                    }
                }
            }
        }
        Ok(CallPreparationResult::Failure {
            resources_returned,
            call_result,
        }) => Ok((resources_returned, call_result)),
        Err(e) => Err(e),
    }
}

#[inline(always)]
fn handle_requested_external_call_to_special_address_space<'a, S: EthereumLikeTypes>(
    callstack: &mut SliceVec<StackFrame<'a, S, SystemFrameSnapshot<S>>>,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    mut call_request: ExternalCallRequest<S>,
) -> Result<ControlFlow<'a, S>, FatalError>
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

        let call_result = CallResult::Successful {
            return_values: ReturnValues::empty(system),
        };

        Ok(halt_or_continue_after_external_call(
            callstack,
            system,
            resources_returned,
            call_result,
        )?)
    }
}

pub enum CallPreparationResult<'a, S: SystemTypes> {
    Success {
        next_ee_version: u8,
        bytecode: &'a [u8],
        bytecode_len: u32,
        artifacts_len: u32,
        actual_resources_to_pass: S::Resources,
    },
    Failure {
        resources_returned: S::Resources,
        call_result: CallResult<S>,
    },
}

/// Reads callee account and runs call preparation function
/// from the system. Additionally, does token transfer if needed.
fn run_call_preparation<'a, S: EthereumLikeTypes>(
    vm: Option<&mut SupportedEEVMState<S>>,
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
    call_request: &ExternalCallRequest<S>,
) -> Result<CallPreparationResult<'a, S>, FatalError>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    let is_entry_frame = vm.is_none();
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
            return Ok(CallPreparationResult::Failure {
                resources_returned: resources_available,
                call_result: CallResult::CallFailedToExecute,
            })
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
                                return Ok(CallPreparationResult::Failure {
                                    resources_returned: resources_available,
                                    call_result: CallResult::CallFailedToExecute,
                                })
                            }
                            Err(SystemError::OutOfNativeResources) => {
                                return Err(FatalError::OutOfNativeResources)
                            }
                            Err(SystemError::Internal(error)) => {
                                return Err(error.into());
                            }
                        };
                    // Give remaining ergs back to caller
                    vm.unwrap().give_back_ergs(resources_available);
                    // Add stipend
                    if let Some(stipend) = stipend {
                        resources_to_pass.add_ergs(stipend)
                    }

                    return Ok(CallPreparationResult::Failure {
                        resources_returned: resources_to_pass,
                        call_result: CallResult::Failed {
                            return_values: ReturnValues::empty(system),
                        },
                    });
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
                    return Ok(CallPreparationResult::Failure {
                        resources_returned: resources_available,
                        call_result: CallResult::CallFailedToExecute,
                    })
                }
                Err(SystemError::OutOfNativeResources) => {
                    return Err(FatalError::OutOfNativeResources)
                }
                Err(SystemError::Internal(error)) => {
                    return Err(error.into());
                }
            };
            // Give remaining ergs back to caller
            vm.unwrap().give_back_ergs(resources_available);
            to_pass
        }
    } else {
        resources_available.take()
    };

    // Add stipend
    if let Some(stipend) = stipend {
        actual_resources_to_pass.add_ergs(stipend)
    }
    Ok(CallPreparationResult::Success {
        next_ee_version,
        bytecode,
        bytecode_len,
        artifacts_len,
        actual_resources_to_pass,
    })
}

// TODO: all the gas computation in this function seems very EVM-specific.
// It should be split into EVM and generic part.
/// Run call preparation, which includes reading the callee parameters,
/// performing a token transfer and charging for resources.
fn prepare_for_call<'a, S: EthereumLikeTypes>(
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
    resources: &mut S::Resources,
    call_request: &ExternalCallRequest<S>,
    is_entry_frame: bool,
) -> Result<CalleeParameters<'a>, CallPreparationError>
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
fn handle_requested_deployment<'a, S: EthereumLikeTypes>(
    callstack: &mut SliceVec<StackFrame<'a, S, SystemFrameSnapshot<S>>>,
    system: &mut System<S>,
    deployment_parameters: DeploymentPreparationParameters<'a, S>,
    initial_ee_version: ExecutionEnvironmentType,
) -> Result<ControlFlow<'a, S>, FatalError>
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
            let mut constructor =
                Box::new(SupportedEEVMState::create_initial(ee_type as u8, system)?);

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
            halt_or_continue_after_deployment(
                callstack,
                system,
                resources_for_deployer,
                deployment_result,
            )
        }
        Err(FatalError::OutOfNativeResources) => Err(FatalError::OutOfNativeResources),
        Err(FatalError::Internal(e)) => Err(e.into()),
    }
}

#[inline(always)]
fn handle_completed_execution<'a, S: EthereumLikeTypes>(
    callstack: &mut SliceVec<StackFrame<'a, S, SystemFrameSnapshot<S>>>,
    system: &mut System<S>,
    return_values: ReturnValues<S>,
    resources_returned: S::Resources,
    reverted: bool,
) -> Result<ControlFlow<'a, S>, FatalError>
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
fn handle_completed_deployment<'a, S: EthereumLikeTypes>(
    callstack: &mut SliceVec<StackFrame<'a, S, SystemFrameSnapshot<S>>>,
    system: &mut System<S>,
    deployment_result: DeploymentResult<S>,
    mut resources_returned: S::Resources,
) -> Result<ControlFlow<'a, S>, FatalError>
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
