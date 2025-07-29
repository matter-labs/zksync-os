use crate::bootloader::constants::SPECIAL_ADDRESS_SPACE_BOUND;
use crate::bootloader::supported_ees::SupportedEEVMState;
use crate::bootloader::DEBUG_OUTPUT;
use alloc::boxed::Box;
use core::fmt::Write;
use core::mem::MaybeUninit;
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
    initial_request: ExecutionEnvironmentSpawnRequest<S>,
) -> Result<TransactionEndPoint<'a, S>, BootloaderSubsystemError>
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

    match initial_request {
        ExecutionEnvironmentSpawnRequest::RequestedExternalCall(external_call_request) => {
            let (resources_returned, call_result) = run.handle_requested_external_call::<true>(
                initial_ee_version,
                external_call_request,
                heap,
            )?;

            let (return_values, reverted) = match call_result {
                CallResult::CallFailedToExecute => (ReturnValues::empty(), true),
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
        ExecutionEnvironmentSpawnRequest::RequestedDeployment(deployment_parameters) => run
            .handle_requested_deployment::<true>(initial_ee_version, deployment_parameters, heap)
            .map(TransactionEndPoint::CompletedDeployment),
    }
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

/// Handles an external call `$spawn` originating from `$vm` with execution environment type `$ee_type`
/// and then proceeds to run the VM to the next preemption point.
///
/// Has to be a macro because the call request and VM overlap, so lifetimes don't work out otherwise.
/// Can't be split up because otherwise we need to check if call or deployment twice.
macro_rules! handle_spawn {
    ($run: ident, $vm:ident, $ee_type:ident, $spawn:ident, $heap:ident) => {
        match $spawn {
            ExecutionEnvironmentSpawnRequest::RequestedExternalCall(external_call_request) => {
                $run.callstack_height += 1;
                let (resources, call_result) = $run.handle_requested_external_call::<false>(
                    $ee_type,
                    external_call_request,
                    $heap,
                )?;
                $run.callstack_height -= 1;

                let success = matches!(call_result, CallResult::Successful { .. });

                let _ = $run.system.get_logger().write_fmt(format_args!(
                    "Return from external call, success = {success}\n"
                ));

                $vm.continue_after_external_call($run.system, resources, call_result)
                    .map_err(wrap_error!())
            }
            ExecutionEnvironmentSpawnRequest::RequestedDeployment(deployment_parameters) => {
                $run.callstack_height += 1;
                let CompletedDeployment {
                    resources_returned,
                    deployment_result,
                } = $run.handle_requested_deployment::<false>(
                    $ee_type,
                    deployment_parameters,
                    $heap,
                )?;
                $run.callstack_height -= 1;

                let returndata_region = deployment_result.returndata();
                let returndata_iter = returndata_region.iter().copied();
                let _ = $run
                    .system
                    .get_logger()
                    .write_fmt(format_args!("Returndata = "));
                let _ = $run.system.get_logger().log_data(returndata_iter);

                $vm.continue_after_deployment($run.system, resources_returned, deployment_result)
                    .map_err(wrap_error!())
            }
        }
    };
}

impl<'external, S: EthereumLikeTypes> Run<'_, 'external, S> {
    fn copy_into_return_memory<'a>(
        &mut self,
        return_values: ReturnValues<'a, S>,
    ) -> ReturnValues<'external, S> {
        let return_memory = core::mem::take(&mut self.return_memory);
        let (output, rest) = return_memory.split_at_mut(return_values.returndata.len());
        self.return_memory = rest;

        ReturnValues {
            returndata: output.write_copy_of_slice(return_values.returndata),
            ..return_values
        }
    }

    fn handle_requested_external_call<const IS_ENTRY_FRAME: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        call_request: ExternalCallRequest<S>,
        heap: SliceVec<u8>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
        #[cfg(not(target_arch = "riscv32"))]
        {
            let _ = self
                .system
                .get_logger()
                .write_fmt(format_args!("External call to {:?}\n", call_request.callee));

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
        match run_call_preparation::<S, IS_ENTRY_FRAME>(self.system, ee_type, call_request) {
            Ok(CallPreparationResult::Success {
                next_ee_version: next_ee_version_returned,
                transfer_to_perform: transfer_to_perform_returned,
                external_call_launch_params: external_call_launch_params_returned,
                resources_in_caller_frame: resources_in_caller_frame_returned,
            }) => {
                next_ee_version = next_ee_version_returned;
                transfer_to_perform = transfer_to_perform_returned;
                external_call_launch_params = external_call_launch_params_returned;
                resources_in_caller_frame = resources_in_caller_frame_returned;
            }

            Ok(CallPreparationResult::Failure {
                resources_in_caller_frame,
            }) => return Ok((resources_in_caller_frame, CallResult::CallFailedToExecute)),
            Err(e) => return Err(e),
        };

        // resources are checked and spent, so we continue with actual transition of control flow

        // We create a new frame for callee, should include transfer and
        // callee execution
        let rollback_handle = self.system.start_global_frame()?;

        // Note that actual transfer is executed in "check_if_external_call_returns_early" which may be confusing
        let callee_frame_execution_result = if let Some(call_result) = self
            .check_if_external_call_returns_early(
                &mut external_call_launch_params,
                &transfer_to_perform,
                ee_type,
                is_call_to_special_address,
            )? {
            // Call finished before VM started
            let failure = !matches!(call_result, CallResult::Successful { .. });
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
            )
        };

        let (resources_returned_from_callee, call_result) = callee_frame_execution_result?;
        resources_in_caller_frame.reclaim(resources_returned_from_callee);
        Ok((resources_in_caller_frame, call_result))
    }

    #[inline(always)]
    fn check_if_external_call_returns_early<'a>(
        &mut self,
        external_call_params: &mut ExecutionEnvironmentLaunchParams<S>,
        transfer_to_perform: &Option<TransferInfo>,
        ee_type: ExecutionEnvironmentType,
        is_call_to_special_address: bool,
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

        let is_eoa = match external_call_params.environment_parameters.bytecode {
            Bytecode::Decommitted {
                bytecode,
                unpadded_code_len: _,
                artifacts_len: _,
                code_version: _,
            } => bytecode.is_empty(),
            Bytecode::Constructor(_) => {
                return Err(SubsystemError::LeafDefect(internal_error!(
                    "Constructor bytecode used instead of bytecode"
                )))
            }
        };

        // Calls to EOAs succeed with empty return value
        if !is_call_to_special_address && is_eoa {
            return Ok(Some(CallResult::Successful {
                return_values: ReturnValues::empty(),
            }));
        }

        if self.callstack_height > 1024 {
            return Ok(Some(CallResult::Failed {
                return_values: ReturnValues::empty(),
            }));
        }

        Ok(None)
    }

    fn call_execute_callee_frame(
        &mut self,
        external_call_launch_params: ExecutionEnvironmentLaunchParams<S>,
        heap: SliceVec<u8>,
        next_ee_version: u8,
        rollback_handle: SystemFrameSnapshot<S>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // now grow callstack and prepare initial state
        let mut new_vm = create_ee(next_ee_version, self.system)?;
        let new_ee_type = new_vm.ee_type();

        let mut preemption = new_vm
            .start_executing_frame(self.system, external_call_launch_params, heap)
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
                    preemption = handle_spawn!(self, new_vm, new_ee_type, request, heap)?;
                }
                ExecutionEnvironmentPreemptionPoint::End(
                    TransactionEndPoint::CompletedExecution(CompletedExecution {
                        resources_returned,
                        return_values,
                        reverted,
                    }),
                ) => {
                    self.system
                        .finish_global_frame(reverted.then_some(&rollback_handle))
                        .map_err(|_| internal_error!("must finish execution frame"))?;

                    let returndata_iter = return_values.returndata.iter().copied();
                    let _ = self
                        .system
                        .get_logger()
                        .write_fmt(format_args!("Returndata = "));
                    let _ = self.system.get_logger().log_data(returndata_iter);

                    let return_values = self.copy_into_return_memory(return_values);

                    return Ok((
                        resources_returned,
                        if reverted {
                            CallResult::Failed { return_values }
                        } else {
                            CallResult::Successful { return_values }
                        },
                    ));
                }
                ExecutionEnvironmentPreemptionPoint::End(
                    TransactionEndPoint::CompletedDeployment(_),
                ) => {
                    //TODO should be misuse
                    return Err(BootloaderSubsystemError::LeafDefect(internal_error!(
                        "returned from external call as if it was a deployment",
                    )));
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
                return_values,
                resources_returned,
                reverted,
                ..
            } = system_hook_run_result;

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

            let return_values = self.copy_into_return_memory(return_values);

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

    fn handle_requested_deployment<const IS_ENTRY_FRAME: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        deployment_parameters: DeploymentPreparationParameters<S>,
        heap: SliceVec<u8>,
    ) -> Result<CompletedDeployment<'external, S>, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // Caller gave away all it's resources into deployment parameters, and in preparation function
        // we will charge for deployment, compute address and potentially increment nonce

        let (mut resources_for_deployer, mut launch_params) =
            match SupportedEEVMState::prepare_for_deployment(
                ee_type,
                self.system,
                deployment_parameters,
            ) {
                Ok((resources, Some(launch_params))) => (resources, launch_params),
                Ok((resources_for_deployer, None)) => {
                    return Ok(CompletedDeployment {
                        resources_returned: resources_for_deployer,
                        deployment_result: DeploymentResult::Failed {
                            return_values: ReturnValues::empty(),
                            execution_reverted: false,
                        },
                    })
                }
                Err(e) => {
                    return Err(wrap_error!(e));
                }
            };

        // resources returned back to caller
        if IS_ENTRY_FRAME {
            // resources returned back to caller do not make sense, so we join them back
            launch_params
                .external_call
                .available_resources
                .reclaim(resources_for_deployer);
            resources_for_deployer = S::Resources::empty();
        }

        if self.callstack_height > 1024 {
            resources_for_deployer.reclaim(launch_params.external_call.available_resources);
            return Ok(CompletedDeployment {
                resources_returned: resources_for_deployer,
                deployment_result: DeploymentResult::Failed {
                    return_values: ReturnValues::empty(),
                    execution_reverted: false,
                },
            });
        }

        let constructor_rollback_handle = self
            .system
            .start_global_frame()
            .map_err(|_| internal_error!("must start a new frame for init code"))?;

        let nominal_token_value = launch_params.external_call.nominal_token_value;

        // EIP-161: contracts should be initialized with nonce 1
        // Note: this has to be done before we actually deploy the bytecode,
        // as constructor execution should see the deployed_address as having
        // nonce = 1
        launch_params
            .external_call
            .available_resources
            .with_infinite_ergs(|inf_resources| {
                self.system.io.increment_nonce(
                    self.initial_ee_version,
                    inf_resources,
                    &launch_params.external_call.callee,
                    1,
                )
            })
            .map_err(|e| -> BootloaderSubsystemError {
                match e {
                    SubsystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_)) => {
                        wrap_error!(e)
                    }
                    _ => internal_error!("Failed to set deployed nonce to 1").into(),
                }
            })?;

        if nominal_token_value != U256::ZERO {
            launch_params
                .external_call
                .available_resources
                .with_infinite_ergs(|inf_resources| {
                    self.system.io.transfer_nominal_token_value(
                        self.initial_ee_version,
                        inf_resources,
                        &launch_params.external_call.caller,
                        &launch_params.external_call.callee,
                        &nominal_token_value,
                    )
                })
                .map_err(|e| -> BootloaderSubsystemError {
                    match e {
                        SubsystemError::LeafUsage(_interface_error) => {
                            // TODO must log the error, but logger is unavailable
                            internal_error!(
                                "Must transfer value on deployment after check in preparation"
                            )
                            .into()
                        }
                        e => wrap_error!(e),
                    }
                })?;
        }

        match self.deployment_execute_constructor_frame(ee_type, launch_params, heap) {
            Ok((deployment_success, mut resources_returned, deployment_result)) => {
                // Now finish constructor frame
                self.system.finish_global_frame(
                    (!deployment_success).then_some(&constructor_rollback_handle),
                )?;

                let _ = self.system.get_logger().write_fmt(format_args!(
                    "Return from constructor call, success = {deployment_success}\n",
                ));

                resources_returned.reclaim(resources_for_deployer);

                Ok(CompletedDeployment {
                    resources_returned,
                    deployment_result,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn deployment_execute_constructor_frame(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        launch_params: ExecutionEnvironmentLaunchParams<S>,
        heap: SliceVec<u8>,
    ) -> Result<
        (
            bool,
            <S as SystemTypes>::Resources,
            zk_ee::system::DeploymentResult<'external, S>,
        ),
        BootloaderSubsystemError,
    >
    where
        S::IO: IOSubsystemExt,
    {
        // EE made all the preparations and we are in callee's frame already
        let mut constructor = create_ee(ee_type as u8, self.system)?;
        let constructor_ee_type = constructor.ee_type();

        let mut preemption = constructor
            .start_executing_frame(self.system, launch_params, heap)
            .map_err(wrap_error!())?;

        let CompletedDeployment {
            mut resources_returned,
            deployment_result,
        } = loop {
            match preemption {
                ExecutionEnvironmentPreemptionPoint::Spawn {
                    ref mut request,
                    ref mut heap,
                } => {
                    let heap = core::mem::take(heap);
                    let request = core::mem::take(request);
                    drop(preemption);
                    preemption =
                        handle_spawn!(self, constructor, constructor_ee_type, request, heap)?;
                }
                ExecutionEnvironmentPreemptionPoint::End(end) => {
                    break match end {
                        TransactionEndPoint::CompletedExecution(_) => {
                            return Err(internal_error!(
                                "returned from deployment as if it was an external call",
                            )
                            .into())
                        }
                        TransactionEndPoint::CompletedDeployment(result) => result,
                    }
                }
            }
        };

        let (deployment_success, deployment_result) = match deployment_result {
            DeploymentResult::Successful {
                deployed_code,
                return_values,
                deployed_at,
            } => {
                // it's responsibility of the system to finish deployment. We continue to use resources from deployment frame
                match self.system.deploy_bytecode(
                    ee_type,
                    &mut resources_returned,
                    &deployed_at,
                    deployed_code,
                ) {
                    Ok(deployed_code) => {
                        let deployment_result = DeploymentResult::Successful {
                            deployed_code,
                            return_values: ReturnValues::empty(),
                            deployed_at,
                        };
                        // TODO: debug implementation for Bits uses global alloc, which panics in ZKsync OS
                        #[cfg(not(target_arch = "riscv32"))]
                        let _ = self.system.get_logger().write_fmt(format_args!(
                            "Successfully deployed contract at {deployed_at:?} \n"
                        ));
                        (true, deployment_result)
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                        let deployment_result = DeploymentResult::Failed {
                            return_values: self.copy_into_return_memory(return_values),
                            execution_reverted: false,
                        };
                        (false, deployment_result)
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc))) => {
                        return Err(RuntimeError::OutOfNativeResources(loc).into())
                    }
                    Err(SystemError::LeafDefect(e)) => return Err(e.into()),
                }
            }
            DeploymentResult::Failed {
                return_values,
                execution_reverted,
            } => (
                false,
                DeploymentResult::Failed {
                    return_values: self.copy_into_return_memory(return_values),
                    execution_reverted,
                },
            ),
        };

        Ok((deployment_success, resources_returned, deployment_result))
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
) -> Result<CallPreparationResult<'a, S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources_in_caller_frame = call_request.available_resources.take();

    let r = if IS_ENTRY_FRAME {
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

    let external_call_launch_params = ExecutionEnvironmentLaunchParams {
        external_call: ExternalCallRequest {
            available_resources: resources_for_callee_frame,
            ..call_request
        },
        environment_parameters: EnvironmentParameters {
            bytecode: Bytecode::Decommitted {
                bytecode: callee_account_properties.bytecode,
                unpadded_code_len: callee_account_properties.unpadded_code_len,
                artifacts_len: callee_account_properties.artifacts_len,
                code_version: callee_account_properties.code_version,
            },
            scratch_space_len: 0,
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
    let account_properties = match system.io.read_account_properties(
        ee_version,
        resources,
        &call_request.callee,
        AccountDataRequest::empty()
            .with_ee_version()
            .with_unpadded_code_len()
            .with_artifacts_len()
            .with_bytecode()
            .with_nonce()
            .with_nominal_token_balance()
            .with_code_version(),
    ) {
        Ok(account_properties) => account_properties,
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

    Ok(CalleeAccountProperties {
        next_ee_version: account_properties.ee_version.0,
        bytecode: account_properties.bytecode.0,
        nonce: account_properties.nonce.0,
        nominal_token_balance: account_properties.nominal_token_balance.0,
        code_version: account_properties.code_version.0,
        unpadded_code_len: account_properties.unpadded_code_len.0,
        artifacts_len: account_properties.artifacts_len.0,
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
