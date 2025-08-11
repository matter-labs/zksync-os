use crate::bootloader::constants::SPECIAL_ADDRESS_SPACE_BOUND;
use crate::bootloader::supported_ees::SupportedEEVMState;
use crate::bootloader::DEBUG_OUTPUT;
use alloc::boxed::Box;
use core::fmt::Write;
use core::mem::MaybeUninit;
use errors::internal::InternalError;
use ruint::aliases::B160;
use ruint::aliases::U256;
use system_hooks::*;
use zk_ee::common_structs::CalleeAccountProperties;
use zk_ee::common_structs::TransferInfo;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::interface_error;
use zk_ee::memory::slice_vec::SliceVec;
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

    let mut run = Run {
        system,
        hooks,
        initial_ee_version,
        callstack_height: 0,
        return_memory: memories.return_data,
    };

    run.handle_requested_external_call::<true>(initial_ee_version, initial_request, heap, tracer)
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

struct Run<'a, 'm, S: EthereumLikeTypes> {
    system: &'a mut System<S>,
    hooks: &'a mut HooksStorage<S, S::Allocator>,
    initial_ee_version: ExecutionEnvironmentType,
    callstack_height: usize,

    return_memory: &'m mut [MaybeUninit<u8>],
}

const SPECIAL_ADDRESS_BOUND: B160 = B160::from_limbs([SPECIAL_ADDRESS_SPACE_BOUND, 0, 0]);

// TODO rename

/// Handles an external call `$spawn` originating from `$vm` with execution environment type `$ee_type`
/// and then proceeds to run the VM to the next preemption point.
///
/// Has to be a macro because the call request and VM overlap, so lifetimes don't work out otherwise.
/// Can't be split up because otherwise we need to check if call or deployment twice.
macro_rules! handle_spawn {
    ($run: ident, $vm:ident, $ee_type:ident, $spawn:ident, $heap:ident, $tracer:ident) => {{
        $run.callstack_height += 1;
        let CompletedExecution {
            resources_returned,
            result,
        } = $run.handle_requested_external_call::<false>($ee_type, $spawn, $heap, $tracer)?;

        let _ = $run.system.get_logger().write_fmt(format_args!(
            "Return from call or deployment, success = {:?}\n",
            !result.failed()
        ));
        $run.callstack_height -= 1;

        $vm.continue_after_preemption($run.system, resources_returned, result, $tracer)
            .map_err(wrap_error!())
    }};
}

impl<'external, S: EthereumLikeTypes> Run<'_, 'external, S> {
    fn copy_into_return_memory<'a>(
        &mut self,
        return_values: ReturnValues<'a, S>,
    ) -> Result<ReturnValues<'external, S>, InternalError> {
        let return_memory = core::mem::take(&mut self.return_memory);
        if return_values.returndata.len() > return_memory.len() {
            return Err(internal_error!("OOM on returndata buffer"));
        }
        let (output, rest) = return_memory.split_at_mut(return_values.returndata.len());
        self.return_memory = rest;

        Ok(ReturnValues {
            returndata: output.write_copy_of_slice(return_values.returndata),
            ..return_values
        })
    }

    fn handle_requested_external_call<const IS_ENTRY_FRAME: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
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
            && self
                .hooks
                .has_hook_for(call_request.callee.as_limbs()[0] as u16);

        // NOTE: on external call request caller doesn't spend resources,
        // but indicates how much he would want to pass at most. Here we can decide the rest

        // we should create next EE and push to callstack
        // only system knows next EE version

        // NOTE: we should move to the frame of the CALLEE now, even though we still use resources of
        // CALLER to perform some reads. If we bail, then we will roll back the frame and all
        // potential writes below, otherwise we will pass what's needed to caller

        // declaring these here rather than returning them reduces stack usage.
        let (
            next_ee_version,
            transfer_to_perform,
            mut external_call_launch_params,
            mut resources_in_caller_frame,
        );
        let is_constructor = call_request.modifier == CallModifier::Constructor;
        match run_call_preparation::<S, IS_ENTRY_FRAME>(
            self.system,
            ee_type,
            call_request,
            self.callstack_height,
        ) {
            Ok(CallPreparationResult::Success {
                next_ee_version: next_ee_version_returned,
                transfer_to_perform: transfer_to_perform_returned,
                external_call_launch_params: external_call_launch_params_returned,
                resources_in_caller_frame: resources_in_caller_frame_returned,
            }) => {
                next_ee_version = if is_constructor {
                    ee_type as u8
                } else {
                    next_ee_version_returned
                };
                transfer_to_perform = transfer_to_perform_returned;
                external_call_launch_params = external_call_launch_params_returned;
                resources_in_caller_frame = resources_in_caller_frame_returned;
            }

            Ok(CallPreparationResult::Failure {
                resources_in_caller_frame,
            }) => {
                return Ok(CompletedExecution {
                    resources_returned: resources_in_caller_frame,
                    result: CallResult::CallFailedToExecute,
                })
            }
            Err(e) => return Err(e),
        };

        // resources are checked and spent, so we continue with actual transition of control flow

        // Note that for tracing we treat failure on preparation step as failure before external call started
        tracer.on_new_execution_frame(&external_call_launch_params);

        let mut next_ee_type = match next_ee_version {
            0 => ExecutionEnvironmentType::NoEE,
            1 => ExecutionEnvironmentType::EVM,
            _ => unreachable!(), // TODO
        };

        if next_ee_type == ExecutionEnvironmentType::NoEE {
            next_ee_type = ExecutionEnvironmentType::EVM;
        }

        match SupportedEEVMState::before_executing_frame(
            next_ee_type, // TODO ee type
            self.system,
            &mut external_call_launch_params,
            tracer,
        ) {
            Ok(success) => {
                if !success {
                    tracer.after_execution_frame_completed(None); // TODO pass returned resources anyway

                    resources_in_caller_frame.reclaim(
                        external_call_launch_params
                            .external_call
                            .available_resources,
                    );
                    return Ok(CompletedExecution {
                        resources_returned: resources_in_caller_frame,
                        result: CallResult::Failed {
                            return_values: ReturnValues::empty(),
                        },
                    });
                }
            }
            Err(e) => return Err(wrap_error!(e)),
        }

        // We create a new frame for callee, should include transfer and
        // callee execution
        let rollback_handle = self.system.start_global_frame()?;

        let callee_frame_execution_result = if let Some(call_result) = self
            .perform_requested_transfer(
                &mut external_call_launch_params,
                &transfer_to_perform,
                ee_type,
            )? {
            let failure = matches!(call_result, CallResult::Failed { .. });
            self.system
                .finish_global_frame(failure.then_some(&rollback_handle))?;

            let resources_to_return = external_call_launch_params
                .external_call
                .available_resources;

            Ok((resources_to_return, call_result))
        } else if is_call_to_special_address {
            // The call is targeting the "system contract" space.
            self.call_to_special_address_execute_callee_frame(
                external_call_launch_params,
                ee_type,
                rollback_handle,
            )
        } else {
            self.call_execute_callee_frame(
                external_call_launch_params,
                heap,
                next_ee_version,
                rollback_handle,
                tracer,
            )
        };

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

    #[inline(always)]
    fn perform_requested_transfer<'a>(
        &mut self,
        external_call_params: &mut ExecutionEnvironmentLaunchParams<S>,
        transfer_to_perform: &Option<TransferInfo>,
        ee_type: ExecutionEnvironmentType,
    ) -> Result<Option<CallResult<'a, S>>, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // Now, perform transfer with infinite ergs
        if let Some(TransferInfo { value, target }) = transfer_to_perform {
            match external_call_params
                .external_call
                .available_resources
                .with_infinite_ergs(|inf_resources| {
                    self.system.io.transfer_nominal_token_value(
                        ExecutionEnvironmentType::NoEE,
                        inf_resources,
                        &external_call_params.external_call.caller,
                        &target,
                        &value,
                    )
                }) {
                Ok(()) => (),
                Err(e) => {
                    match e {
                        SubsystemError::LeafUsage(_interface_error) => {
                            // TODO log this error, but logger is unavailable
                            // Insufficient balance
                            match ee_type {
                                ExecutionEnvironmentType::NoEE => {
                                    return Err(interface_error!(
                                        BootloaderInterfaceError::TopLevelInsufficientBalance
                                    ))
                                }
                                ExecutionEnvironmentType::EVM => {
                                    // Following EVM, a call with insufficient balance is not a revert,
                                    // but rather a normal failing call.
                                    return Ok(Some(CallResult::Failed {
                                        return_values: ReturnValues::empty(),
                                    }));
                                }
                            }
                        }
                        SubsystemError::LeafDefect(_) => return Err(wrap_error!(e)),
                        SubsystemError::LeafRuntime(ref runtime_error) => match runtime_error {
                            RuntimeError::OutOfNativeResources(_) => return Err(wrap_error!(e)),
                            RuntimeError::OutOfErgs(_) => {
                                return Err(internal_error!("Out of ergs on infinite ergs").into())
                            }
                        },
                        SubsystemError::Cascaded(cascaded_error) => match cascaded_error {},
                    }
                }
            }
        }

        Ok(None)
    }

    fn call_execute_callee_frame(
        &mut self,
        external_call_launch_params: ExecutionEnvironmentLaunchParams<S>,
        heap: SliceVec<u8>,
        next_ee_version: u8,
        rollback_handle: SystemFrameSnapshot<S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // By convention, calls to empty accounts succeed without any return data
        if next_ee_version == ExecutionEnvironmentType::NO_EE_BYTE {
            if let Bytecode::Decommitted {
                bytecode,
                unpadded_code_len: _,
                artifacts_len: _,
                code_version: _,
            } = external_call_launch_params.environment_parameters.bytecode
            {
                if bytecode.len() != 0 {
                    return Err(internal_error!("Unexpected non-empty bytecode").into());
                }
            } else {
                return Err(internal_error!("Invalid No_EE invocation").into());
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

        // now grow callstack and prepare initial state
        let mut new_vm = create_ee(next_ee_version, self.system)?;
        let new_ee_type = new_vm.ee_type();

        let mut preemption = new_vm
            .start_executing_frame(self.system, external_call_launch_params, heap, tracer)
            .map_err(wrap_error!())?;

        loop {
            match preemption {
                ExecutionEnvironmentPreemptionPoint::Spawn {
                    ref mut request,
                    ref mut heap,
                } => {
                    let heap = core::mem::take(heap);
                    let request = core::mem::take(request);
                    drop(preemption);
                    preemption = handle_spawn!(self, new_vm, new_ee_type, request, heap, tracer)?;
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

    fn call_to_special_address_execute_callee_frame(
        &mut self,
        external_call_launch_params: ExecutionEnvironmentLaunchParams<S>,
        caller_ee_type: ExecutionEnvironmentType,
        rollback_handle: SystemFrameSnapshot<S>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
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
        next_ee_version: u8,
        transfer_to_perform: Option<TransferInfo>,
        external_call_launch_params: ExecutionEnvironmentLaunchParams<'a, S>,
        resources_in_caller_frame: S::Resources,
    },
    Failure {
        resources_in_caller_frame: S::Resources,
    },
}

/// Read callee properties, execute additional checks, charge resources and perform additional EE-specific logic
fn run_call_preparation<'a, S: EthereumLikeTypes, const IS_ENTRY_FRAME: bool>(
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
    mut call_request: ExternalCallRequest<'a, S>,
    callstack_depth: usize,
) -> Result<CallPreparationResult<'a, S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources_in_caller_frame = call_request.available_resources.take();

    // TODO ugly
    let r = if IS_ENTRY_FRAME || call_request.modifier == CallModifier::Constructor {
        // For entry frame we don't charge ergs for call preparation,
        // as this is included in the intrinsic cost.
        resources_in_caller_frame.with_infinite_ergs(|inf_resources| {
            read_callee_account_properties(system, ee_version, inf_resources, &call_request)
        })
    } else {
        read_callee_account_properties(
            system,
            ee_version,
            &mut resources_in_caller_frame,
            &call_request,
        )
    };

    let callee_account_properties = match r {
        Ok(x) => x,
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            return Ok(CallPreparationResult::Failure {
                resources_in_caller_frame,
            });
        }
        Err(SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc))) => {
            return Err(RuntimeError::OutOfNativeResources(loc).into())
        }
        Err(SystemError::LeafDefect(e)) => return Err(e.into()),
    };

    // Check transfer is allowed and determine transfer target
    let transfer_to_perform =
        if call_request.nominal_token_value != U256::ZERO && !call_request.is_delegate() {
            if !call_request.is_transfer_allowed() {
                let _ = system.get_logger().write_fmt(format_args!(
                    "Call failed: positive value with modifier {:?}\n",
                    call_request.modifier
                ));
                return Ok(CallPreparationResult::Failure {
                    resources_in_caller_frame,
                });
            }
            // Adjust transfer target due to CALLCODE
            let target = match call_request.modifier {
                CallModifier::EVMCallcode | CallModifier::EVMCallcodeStatic => call_request.caller,
                _ => call_request.callee,
            };
            Some(TransferInfo {
                value: call_request.nominal_token_value,
                target,
            })
        } else {
            None
        };

    // If we're in the entry frame, i.e. not the execution of a CALL opcode,
    // we don't apply the CALL-specific gas charging, but instead set
    // resources_for_callee_frame equal to the available resources
    let resources_for_callee_frame = if !IS_ENTRY_FRAME {
        // now we should ask current EE to calculate resources for the callee frame
        let mut callee_resources =
            match SupportedEEVMState::<S>::calculate_resources_passed_in_external_call(
                ee_version,
                &mut resources_in_caller_frame,
                &call_request,
                &callee_account_properties,
            ) {
                Ok(x) => x,
                Err(x) => {
                    if let RootCause::Runtime(RuntimeError::OutOfErgs(_)) = x.root_cause() {
                        return Ok(CallPreparationResult::Failure {
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

    // TODO ugly
    let bytecode = if call_request.modifier == CallModifier::Constructor {
        Bytecode::Constructor(&call_request.input)
    } else {
        Bytecode::Decommitted {
            bytecode: callee_account_properties.bytecode,
            unpadded_code_len: callee_account_properties.unpadded_code_len,
            artifacts_len: callee_account_properties.artifacts_len,
            code_version: callee_account_properties.code_version,
        }
    };

    if call_request.modifier == CallModifier::Constructor {
        // TODO ugly
        call_request.input = &[];
    }

    let external_call_launch_params = ExecutionEnvironmentLaunchParams {
        external_call: ExternalCallRequest {
            available_resources: resources_for_callee_frame,
            ..call_request
        },
        environment_parameters: EnvironmentParameters {
            bytecode,
            scratch_space_len: 0,
            callstack_depth,
        },
    };

    Ok(CallPreparationResult::Success {
        next_ee_version: callee_account_properties.next_ee_version,
        transfer_to_perform,
        external_call_launch_params,
        resources_in_caller_frame,
    })
}

/// Charge for reading account properties and perform actual read
fn read_callee_account_properties<'a, S: EthereumLikeTypes>(
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
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
            ee_version,
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
            let properties = if cfg!(feature = "pectra") && account_properties.is_delegated.0 {
                use crate::bootloader::transaction::parse_delegation;
                // Resolve delegation following EIP-7702 (only one level
                // of delegation is allowed).
                let delegation = &account_properties.bytecode.0
                    [..account_properties.unpadded_code_len.0 as usize];
                let address = parse_delegation(delegation)?;
                let delegate_properties = system.io.read_account_properties(
                    ee_version,
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
        Err(SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc))) => {
            return Err(SystemError::LeafRuntime(
                RuntimeError::OutOfNativeResources(loc),
            ))
        }
        Err(SystemError::LeafDefect(e)) => return Err(e.into()),
    };

    // Read required data to perform a call
    let (
        next_ee_version,
        bytecode,
        code_version,
        unpadded_code_len,
        artifacts_len,
        nonce,
        nominal_token_balance,
    ) = if let Some(delegate_properties) = delegate_properties {
        let ee_version = delegate_properties.ee_version.0;
        let unpadded_code_len = delegate_properties.unpadded_code_len.0;
        let artifacts_len = delegate_properties.artifacts_len.0;
        let bytecode = delegate_properties.bytecode.0;
        let code_version = delegate_properties.code_version.0;
        let nonce = delegate_properties.nonce.0;
        let nominal_token_balance = delegate_properties.nominal_token_balance.0;

        (
            ee_version,
            bytecode,
            code_version,
            unpadded_code_len,
            artifacts_len,
            nonce,
            nominal_token_balance,
        )
    } else {
        let ee_version = account_properties.ee_version.0;
        let unpadded_code_len = account_properties.unpadded_code_len.0;
        let artifacts_len = account_properties.artifacts_len.0;
        let bytecode = account_properties.bytecode.0;
        let code_version = account_properties.code_version.0;
        let nonce = account_properties.nonce.0;
        let nominal_token_balance = account_properties.nominal_token_balance.0;
        (
            ee_version,
            bytecode,
            code_version,
            unpadded_code_len,
            artifacts_len,
            nonce,
            nominal_token_balance,
        )
    };

    Ok(CalleeAccountProperties {
        next_ee_version,
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
    ee_type: u8,
    system: &mut System<S>,
) -> Result<Box<SupportedEEVMState<'a, S>, S::Allocator>, BootloaderSubsystemError> {
    Ok(Box::new_in(
        SupportedEEVMState::create_initial(ee_type, system).map_err(wrap_error!())?,
        system.get_allocator(),
    ))
}
