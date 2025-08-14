use crate::bootloader::constants::SPECIAL_ADDRESS_SPACE_BOUND;
use crate::bootloader::supported_ees::SupportedEEVMState;
use crate::bootloader::DEBUG_OUTPUT;
use alloc::boxed::Box;
use core::fmt::Write;
use core::mem::MaybeUninit;
use ruint::aliases::B160;
use system_hooks::*;
use zk_ee::common_structs::CalleeAccountProperties;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::interface_error;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::out_of_return_memory;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{errors::system::SystemError, logger::Logger, *};
use zk_ee::wrap_error;
use zk_ee::{internal_error, out_of_ergs_error};

use super::errors::BootloaderInterfaceError;
use super::errors::BootloaderSubsystemError;

/// Main execution loop.
/// Expects the caller to start and close the entry frame.
pub fn run_till_completion<'a, S: EthereumLikeTypes>(
    memories: RunnerMemoryBuffers<'a>,
    system: &mut System<S>,
    hooks: &mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    initial_request: ExternalCallRequest<S>,
    tracer: &mut impl Tracer<S>,
) -> Result<CompletedExecution<'a, S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
{
    let heap = SliceVec::new(memories.heaps);

    // NOTE: we do not need to make a new frame as we are in the root already

    let _ = system
        .get_logger()
        .write_fmt(format_args!("Begin execution\n"));

    let mut execution = ExecutionContext {
        system,
        hooks,
        callstack_height: 0,
        return_memory: memories.return_data,
    };

    execution.handle_requested_external_call::<true>(
        initial_ee_version,
        initial_request,
        heap,
        tracer,
    )
}

pub struct RunnerMemoryBuffers<'a> {
    pub heaps: &'a mut [MaybeUninit<u8>],
    pub return_data: &'a mut [MaybeUninit<u8>],
}

impl RunnerMemoryBuffers<'_> {
    /// This struct can't implement [Clone] because it contains mutable references.
    /// This analogue of cloning holds onto self until the returned struct is dropped.
    pub fn reborrow<'a>(&'a mut self) -> RunnerMemoryBuffers<'a> {
        let RunnerMemoryBuffers { heaps, return_data } = self;
        RunnerMemoryBuffers { heaps, return_data }
    }
}

struct ExecutionContext<'a, 'm, S: EthereumLikeTypes> {
    system: &'a mut System<S>,
    hooks: &'a mut HooksStorage<S, S::Allocator>,
    callstack_height: usize,

    return_memory: &'m mut [MaybeUninit<u8>],
}

const SPECIAL_ADDRESS_BOUND: B160 = B160::from_limbs([SPECIAL_ADDRESS_SPACE_BOUND, 0, 0]);

impl<'external, S: EthereumLikeTypes> ExecutionContext<'_, 'external, S> {
    fn copy_into_return_memory<'a>(
        &mut self,
        return_values: ReturnValues<'a, S>,
    ) -> Result<ReturnValues<'external, S>, BootloaderSubsystemError> {
        let return_memory = core::mem::take(&mut self.return_memory);
        if return_values.returndata.len() > return_memory.len() {
            return Err(out_of_return_memory!().into());
        }
        let (output, rest) = return_memory.split_at_mut(return_values.returndata.len());
        self.return_memory = rest;

        Ok(ReturnValues {
            returndata: output.write_copy_of_slice(return_values.returndata),
            ..return_values
        })
    }

    /// High-level function used to process call requests
    fn handle_requested_external_call<const IS_ENTRY_FRAME: bool>(
        &mut self,
        caller_ee_type: ExecutionEnvironmentType,
        call_request: ExternalCallRequest<S>,
        heap: SliceVec<u8>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<CompletedExecution<'external, S>, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
        #[cfg(not(target_arch = "riscv32"))]
        {
            let _ = self.system.get_logger().write_fmt(format_args!(
                "External call or deploy to {:?}\n",
                call_request.callee
            ));

            let _ = self.system.get_logger().write_fmt(format_args!(
                "External call with parameters:\n{:?}\n",
                &call_request,
            ));
        }

        // We begin execution of the requested call in the caller's context. This is necessary
        // because the execution environment does not charge the caller's frame for reading
        // the callee's account properties — this is currently handled within the storage implementation.
        // Therefore, we read the callee's data, charge the caller accordingly, calculate the actual amount
        // of ergs passed to the callee, and then execute the callee's frame.

        // Note: in future charging for reading account properties should be done in EE, so this logic could be simplified

        // declaring these here rather than returning them reduces stack usage.
        let (next_ee_type, external_call_launch_params, mut resources_in_caller_frame);
        match read_callee_and_prepare_frame_state::<S, IS_ENTRY_FRAME>(
            self.system,
            caller_ee_type,
            call_request,
            self.callstack_height,
        ) {
            Ok(CallPreparationResult::Success {
                next_ee_type: next_ee_type_returned,
                external_call_launch_params: external_call_launch_params_returned,
                resources_in_caller_frame: resources_in_caller_frame_returned,
            }) => {
                next_ee_type = next_ee_type_returned;
                external_call_launch_params = external_call_launch_params_returned;
                resources_in_caller_frame = resources_in_caller_frame_returned;
            }

            Ok(CallPreparationResult::OutOfErgs {
                resources_in_caller_frame,
            }) => {
                // Failure in the **caller** frame context
                // Should not happen in entry frame (callstack depth 0)
                return Ok(CompletedExecution {
                    resources_returned: resources_in_caller_frame,
                    result: CallResult::PreparationStepFailed,
                });
            }
            Err(e) => return Err(e),
        };

        // Resources are checked and spent, so we continue with actual transition of control flow to the callee

        tracer.on_new_execution_frame(&external_call_launch_params);

        let callee_frame_execution_result = self.execute_call(
            next_ee_type,
            caller_ee_type,
            external_call_launch_params,
            heap,
            tracer,
        );

        tracer.after_execution_frame_completed(
            callee_frame_execution_result
                .as_ref()
                .map(|(resources_returned, call_result)| Some((resources_returned, call_result)))
                .unwrap_or_default(),
        );

        let (resources_returned_from_callee, call_result) = callee_frame_execution_result?;
        resources_in_caller_frame.reclaim(resources_returned_from_callee);

        Ok(CompletedExecution {
            resources_returned: resources_in_caller_frame,
            result: call_result,
        })
    }

    /// Internal implementation of call execution. Requires prepared external_call_launch_params which include all required data for EE launch.
    fn execute_call(
        &mut self,
        next_ee_type: ExecutionEnvironmentType,
        caller_ee_type: ExecutionEnvironmentType,
        mut external_call_launch_params: ExecutionEnvironmentLaunchParams<S>,
        heap: SliceVec<u8>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // We want to execute some prechecks even if EE is not specified (e.g. call/transfer to empty account or precompile)
        let interpret_as_ee_type = if next_ee_type == ExecutionEnvironmentType::NoEE {
            if caller_ee_type == ExecutionEnvironmentType::NoEE {
                // "Default" EE type
                ExecutionEnvironmentType::EVM
            } else {
                caller_ee_type
            }
        } else {
            next_ee_type
        };

        // Pre-checks and operations that should not be rolled back if call fails
        match SupportedEEVMState::before_executing_frame(
            interpret_as_ee_type,
            self.system,
            &mut external_call_launch_params,
            tracer,
        ) {
            Ok(success) => {
                if !success {
                    return Ok((
                        external_call_launch_params
                            .external_call
                            .available_resources,
                        CallResult::Failed {
                            return_values: ReturnValues::empty(),
                        },
                    ));
                }
            }
            Err(e) => return Err(wrap_error!(e)),
        }

        // Create snapshot for rollbacks
        let rollback_handle = self.system.start_global_frame()?;

        // Try to execute transfer if requested
        if !self.perform_transfer_if_required(
            &mut external_call_launch_params.external_call,
            caller_ee_type,
        )? {
            self.system.finish_global_frame(Some(&rollback_handle))?;

            return Ok((
                external_call_launch_params
                    .external_call
                    .available_resources,
                CallResult::Failed {
                    return_values: ReturnValues::empty(),
                },
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
        let is_call_to_special_address = external_call_launch_params.external_call.callee.as_uint()
            < SPECIAL_ADDRESS_BOUND.as_uint();

        #[cfg(feature = "code_in_kernel_space")]
        let is_call_to_special_address = external_call_launch_params.external_call.callee.as_uint()
            < SPECIAL_ADDRESS_BOUND.as_uint()
            && self.hooks.has_hook_for(
                external_call_launch_params.external_call.callee.as_limbs()[0] as u16,
            );

        if is_call_to_special_address {
            // The call is targeting the "system contract" space.
            self.call_to_special_address_execute_callee_frame(
                external_call_launch_params,
                caller_ee_type,
                rollback_handle,
            )
        } else {
            self.call_execute_callee_frame(
                external_call_launch_params,
                heap,
                next_ee_type,
                rollback_handle,
                tracer,
            )
        }
    }

    /// Check if transfer is requested and try to perform it
    fn perform_transfer_if_required(
        &mut self,
        call_request: &mut ExternalCallRequest<S>,
        caller_ee_type: ExecutionEnvironmentType,
    ) -> Result<bool, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        if call_request.nominal_token_value.is_zero() || call_request.is_delegate() {
            return Ok(true);
        }

        // Check transfer is allowed and determine transfer target
        if !call_request.is_transfer_allowed() {
            let _ = self.system.get_logger().write_fmt(format_args!(
                "Call failed: positive value with modifier {:?}\n",
                call_request.modifier
            ));
            return Err(internal_error!("Positive value with incorrect modifier").into());
        }
        // Adjust transfer target due to CALLCODE
        // TODO: in future should be moved to EE
        let target = match call_request.modifier {
            CallModifier::EVMCallcode | CallModifier::EVMCallcodeStatic => call_request.caller,
            _ => call_request.callee,
        };

        match call_request
            .available_resources
            .with_infinite_ergs(|inf_resources| {
                self.system.io.transfer_nominal_token_value(
                    ExecutionEnvironmentType::NoEE,
                    inf_resources,
                    &call_request.caller,
                    &target,
                    &call_request.nominal_token_value,
                )
            }) {
            Ok(()) => Ok(true),
            Err(e) => {
                match e {
                    SubsystemError::LeafUsage(_interface_error) => {
                        let _ = self
                            .system
                            .get_logger()
                            .write_fmt(format_args!("Insufficient balance for transfer\n"));

                        // Insufficient balance
                        match caller_ee_type {
                            ExecutionEnvironmentType::NoEE => Err(interface_error!(
                                BootloaderInterfaceError::TopLevelInsufficientBalance
                            )),
                            ExecutionEnvironmentType::EVM => {
                                // Following EVM, a call with insufficient balance is not a revert,
                                // but rather a normal failing call.
                                Ok(false)
                            }
                        }
                    }
                    SubsystemError::LeafDefect(_) => Err(wrap_error!(e)),
                    SubsystemError::LeafRuntime(ref runtime_error) => match runtime_error {
                        RuntimeError::FatalRuntimeError(_) => Err(wrap_error!(e)),
                        RuntimeError::OutOfErgs(_) => {
                            Err(internal_error!("Out of ergs on infinite ergs").into())
                        }
                    },
                    SubsystemError::Cascaded(cascaded_error) => match cascaded_error {},
                }
            }
        }
    }

    /// Actual passing of control flow to the callee
    fn call_execute_callee_frame(
        &mut self,
        external_call_launch_params: ExecutionEnvironmentLaunchParams<S>,
        heap: SliceVec<u8>,
        next_ee_type: ExecutionEnvironmentType,
        rollback_handle: SystemFrameSnapshot<S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // By convention, calls to empty accounts succeed without any return data
        if next_ee_type == ExecutionEnvironmentType::NoEE {
            if external_call_launch_params.external_call.modifier == CallModifier::Constructor {
                return Err(internal_error!("Invalid No_EE invocation").into());
            } else if external_call_launch_params
                .environment_parameters
                .callee_account_properties
                .unpadded_code_len
                != 0
            {
                return Err(internal_error!("Unexpected non-empty bytecode").into());
            }

            return Ok((
                external_call_launch_params
                    .external_call
                    .available_resources,
                CallResult::Successful {
                    return_values: ReturnValues::empty(),
                },
            ));
        }

        // Create new EE execution instance (frame)
        let mut new_vm = create_ee(next_ee_type, self.system)?;
        let new_ee_type = new_vm.ee_type();

        let mut preemption = new_vm
            .start_executing_frame(self.system, external_call_launch_params, heap, tracer)
            .map_err(wrap_error!())?;

        // Execute until we get `End` preemption point
        loop {
            match preemption {
                ExecutionEnvironmentPreemptionPoint::CallRequest {
                    ref mut request,
                    ref mut heap,
                } => {
                    let heap = core::mem::take(heap);
                    let request = core::mem::take(request);
                    drop(preemption);

                    self.callstack_height += 1;
                    let CompletedExecution {
                        resources_returned,
                        result,
                    } = self.handle_requested_external_call::<false>(
                        new_ee_type,
                        request,
                        heap,
                        tracer,
                    )?;

                    let _ = self.system.get_logger().write_fmt(format_args!(
                        "Return from call or deployment, success = {:?}\n",
                        !result.failed()
                    ));
                    self.callstack_height -= 1;

                    preemption = new_vm
                        .continue_after_preemption(self.system, resources_returned, result, tracer)
                        .map_err(wrap_error!())?;
                }
                ExecutionEnvironmentPreemptionPoint::End(CompletedExecution {
                    resources_returned,
                    result,
                }) => {
                    let reverted = result.failed();
                    let return_values = result.return_values();

                    self.system
                        .finish_global_frame(reverted.then_some(&rollback_handle))
                        .map_err(|_| internal_error!("must finish execution frame"))?;

                    let returndata_iter = return_values.returndata.iter().copied();
                    let _ = self
                        .system
                        .get_logger()
                        .write_fmt(format_args!("Returndata = "));
                    let _ = self.system.get_logger().log_data(returndata_iter);

                    let return_values = self.copy_into_return_memory(return_values)?;

                    return Ok((
                        resources_returned,
                        if reverted {
                            CallResult::Failed { return_values }
                        } else {
                            CallResult::Successful { return_values }
                        },
                    ));
                }
            }
        }
    }

    /// Actual passing of control flow to a special address (system hook)
    fn call_to_special_address_execute_callee_frame(
        &mut self,
        external_call_launch_params: ExecutionEnvironmentLaunchParams<S>,
        caller_ee_type: ExecutionEnvironmentType,
        rollback_handle: SystemFrameSnapshot<S>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // Deploying attempt should be reverted
        if external_call_launch_params.external_call.modifier == CallModifier::Constructor {
            let _ = self.system.get_logger().write_fmt(format_args!(
                "Attempt to deploy something on special address\n"
            ));
            self.system
                .finish_global_frame(Some(&rollback_handle))
                .map_err(|_| internal_error!("must finish execution frame"))?;

            return Ok((
                external_call_launch_params
                    .external_call
                    .available_resources,
                CallResult::Failed {
                    return_values: ReturnValues::empty(),
                },
            ));
        }

        let return_memory = core::mem::take(&mut self.return_memory);
        let resources_passed = external_call_launch_params
            .external_call
            .available_resources
            .clone();
        let (res, remaining_memory) = self.hooks.try_intercept(
            external_call_launch_params.external_call.callee.as_limbs()[0] as u16,
            external_call_launch_params.external_call,
            caller_ee_type as u8,
            self.system,
            return_memory,
        )?;
        // Reclaim unused return memory
        self.return_memory = remaining_memory;

        if let Some(system_hook_run_result) = res {
            let CompletedExecution {
                resources_returned,
                result,
            } = system_hook_run_result;

            let reverted = result.failed();
            let return_values = result.return_values();

            let _ = self.system.get_logger().write_fmt(format_args!(
                "Call to special address returned, success = {}\n",
                !reverted
            ));

            let returndata_slice = return_values.returndata;
            let returndata_iter = returndata_slice.iter().copied();
            let _ = self
                .system
                .get_logger()
                .write_fmt(format_args!("Returndata = "));
            let _ = self.system.get_logger().log_data(returndata_iter);

            self.system
                .finish_global_frame(if reverted {
                    Some(&rollback_handle)
                } else {
                    None
                })
                .map_err(|_| internal_error!("must finish execution frame"))?;

            Ok((
                resources_returned,
                if reverted {
                    CallResult::Failed { return_values }
                } else {
                    CallResult::Successful { return_values }
                },
            ))
        } else {
            let resources_returned = resources_passed;
            // it's an empty account for all the purposes, or default AA
            let _ = self.system.get_logger().write_fmt(format_args!(
                "Call to special address was not intercepted\n",
            ));
            self.system
                .finish_global_frame(None)
                .map_err(|_| internal_error!("must finish execution frame"))?;

            Ok((
                resources_returned,
                CallResult::Successful {
                    return_values: ReturnValues::empty(),
                },
            ))
        }
    }
}

pub enum CallPreparationResult<'a, S: SystemTypes> {
    Success {
        next_ee_type: ExecutionEnvironmentType,
        external_call_launch_params: ExecutionEnvironmentLaunchParams<'a, S>,
        resources_in_caller_frame: S::Resources,
    },
    /// Out of ergs during preparation. For EE it looks like failure happened in the caller frame
    OutOfErgs {
        resources_in_caller_frame: S::Resources,
    },
}

/// Read callee properties, charge for it, calculate resources for callee
fn read_callee_and_prepare_frame_state<'a, S: EthereumLikeTypes, const IS_ENTRY_FRAME: bool>(
    system: &mut System<S>,
    caller_ee_version: ExecutionEnvironmentType,
    mut call_request: ExternalCallRequest<'a, S>,
    callstack_depth: usize,
) -> Result<CallPreparationResult<'a, S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources_in_caller_frame = call_request.available_resources.take();

    let r = if IS_ENTRY_FRAME || call_request.modifier == CallModifier::Constructor {
        // For entry frame we don't charge ergs for call preparation,
        // as this is included in the intrinsic cost. For constructor frame this is also included in creation gas cost.
        // Note: in future charging should be done by EE, so this logic can be unified
        resources_in_caller_frame.with_infinite_ergs(|inf_resources| {
            read_callee_account_properties(system, caller_ee_version, inf_resources, &call_request)
        })
    } else {
        read_callee_account_properties(
            system,
            caller_ee_version,
            &mut resources_in_caller_frame,
            &call_request,
        )
    };

    let callee_account_properties = match r {
        Ok(x) => x,
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            return Ok(CallPreparationResult::OutOfErgs {
                resources_in_caller_frame,
            });
        }
        Err(SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(e))) => {
            return Err(RuntimeError::FatalRuntimeError(e).into())
        }
        Err(SystemError::LeafDefect(e)) => return Err(e.into()),
    };

    let resources_for_callee_frame = if !IS_ENTRY_FRAME {
        // now we should ask current EE to calculate resources for the callee frame
        let mut callee_resources =
            match SupportedEEVMState::<S>::calculate_resources_passed_in_external_call(
                caller_ee_version,
                &mut resources_in_caller_frame,
                &call_request,
                &callee_account_properties,
            ) {
                Ok(x) => x,
                Err(x) => {
                    if let RootCause::Runtime(RuntimeError::OutOfErgs(_)) = x.root_cause() {
                        return Ok(CallPreparationResult::OutOfErgs {
                            resources_in_caller_frame,
                        });
                    } else {
                        return Err(wrap_error!(x));
                    }
                }
            };

        // Give native resource to the callee.
        resources_in_caller_frame.give_native_to(&mut callee_resources);
        callee_resources
    } else {
        // If we're in the entry frame, i.e. not the execution of a CALL opcode,
        // we just pass all available resources
        resources_in_caller_frame.take()
    };

    if DEBUG_OUTPUT {
        let _ = system.get_logger().write_fmt(format_args!(
            "Bytecode len for `callee` = {}\n",
            callee_account_properties.bytecode.len(),
        ));
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Bytecode for `callee` = "));
        let _ = system
            .get_logger()
            .log_data(callee_account_properties.bytecode.as_ref().iter().copied());
    }

    let next_ee_version = if call_request.modifier == CallModifier::Constructor {
        // Note: only correct for EVM. For EraVM integration logic should be modified (it calls "constructor" branch of already deployed account)
        caller_ee_version as u8
    } else {
        callee_account_properties.ee_type
    };

    let external_call_launch_params = ExecutionEnvironmentLaunchParams {
        external_call: ExternalCallRequest {
            available_resources: resources_for_callee_frame,
            ..call_request
        },
        environment_parameters: EnvironmentParameters {
            scratch_space_len: 0,
            callstack_depth,
            callee_account_properties,
        },
    };

    Ok(CallPreparationResult::Success {
        next_ee_type: ExecutionEnvironmentType::parse_ee_version_byte(next_ee_version)?,
        external_call_launch_params,
        resources_in_caller_frame,
    })
}

/// Charge for reading account properties and perform actual read
fn read_callee_account_properties<'a, S: EthereumLikeTypes>(
    system: &mut System<S>,
    caller_ee_type: ExecutionEnvironmentType,
    resources: &mut S::Resources,
    call_request: &ExternalCallRequest<S>,
) -> Result<CalleeAccountProperties<'a>, SystemError>
where
    S::IO: IOSubsystemExt,
{
    // IO will follow the rules of the CALLER here to charge for execution
    let (account_properties, delegate_properties) = match system
        .io
        .read_account_properties(
            caller_ee_type,
            resources,
            &call_request.callee,
            AccountDataRequest::empty()
                .with_ee_version()
                .with_unpadded_code_len()
                .with_artifacts_len()
                // If the account is delegated, the bytecode will
                // contain the address of the delegate.
                .with_bytecode()
                .with_nonce()
                .with_nominal_token_balance()
                .with_code_version()
                .with_is_delegated(),
        )
        .and_then(|account_properties| {
            // Note: we ignore delegation in case if this is a constructor call. EE should revert due to collision.
            let properties = if cfg!(feature = "pectra")
                && account_properties.is_delegated.0
                && call_request.modifier != CallModifier::Constructor
            {
                // TODO: in future should be moved to EE
                use crate::bootloader::transaction::parse_delegation;
                // Resolve delegation following EIP-7702 (only one level
                // of delegation is allowed).
                let delegation = &account_properties.bytecode.0
                    [..account_properties.unpadded_code_len.0 as usize];
                let address = parse_delegation(delegation)?;
                let delegate_properties = system.io.read_account_properties(
                    caller_ee_type,
                    resources,
                    &address,
                    AccountDataRequest::empty()
                        .with_ee_version()
                        .with_unpadded_code_len()
                        .with_artifacts_len()
                        .with_bytecode()
                        .with_code_version()
                        .with_nonce()
                        .with_nominal_token_balance(),
                )?;
                (account_properties, Some(delegate_properties))
            } else {
                (account_properties, None)
            };

            Ok(properties)
        }) {
        Ok((account_properties, delegate)) => (account_properties, delegate),
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            let _ = system.get_logger().write_fmt(format_args!(
                "Call failed: insufficient resources to read callee account data\n",
            ));
            return Err(out_of_ergs_error!());
        }
        Err(SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(e))) => {
            return Err(SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(e)))
        }
        Err(SystemError::LeafDefect(e)) => return Err(e.into()),
    };

    // Read required data to perform a call
    let (next_ee_version, bytecode, code_version, unpadded_code_len, artifacts_len) =
        if let Some(delegate_properties) = delegate_properties {
            let ee_version = delegate_properties.ee_version.0;
            let unpadded_code_len = delegate_properties.unpadded_code_len.0;
            let artifacts_len = delegate_properties.artifacts_len.0;
            let bytecode = delegate_properties.bytecode.0;
            let code_version = delegate_properties.code_version.0;

            (
                ee_version,
                bytecode,
                code_version,
                unpadded_code_len,
                artifacts_len,
            )
        } else {
            let ee_version = account_properties.ee_version.0;
            let unpadded_code_len = account_properties.unpadded_code_len.0;
            let artifacts_len = account_properties.artifacts_len.0;
            let bytecode = account_properties.bytecode.0;
            let code_version = account_properties.code_version.0;
            (
                ee_version,
                bytecode,
                code_version,
                unpadded_code_len,
                artifacts_len,
            )
        };

    let nonce = account_properties.nonce.0;
    let nominal_token_balance = account_properties.nominal_token_balance.0;

    Ok(CalleeAccountProperties {
        ee_type: next_ee_version,
        bytecode,
        code_version,
        unpadded_code_len,
        artifacts_len,
        nonce,
        nominal_token_balance,
    })
}

/// This needs to be a separate function so the stack memory
/// that this (unfortunately) allocates gets cleaned up.
#[inline(never)]
fn create_ee<'a, S: EthereumLikeTypes>(
    ee_type: ExecutionEnvironmentType,
    system: &mut System<S>,
) -> Result<Box<SupportedEEVMState<'a, S>, S::Allocator>, BootloaderSubsystemError> {
    Ok(Box::new_in(
        SupportedEEVMState::create_initial(ee_type, system).map_err(wrap_error!())?,
        system.get_allocator(),
    ))
}
