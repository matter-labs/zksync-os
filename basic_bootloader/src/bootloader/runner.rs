use crate::bootloader::constants::SPECIAL_ADDRESS_SPACE_BOUND;
use crate::bootloader::supported_ees::SupportedEEVMState;
use crate::bootloader::DEBUG_OUTPUT;
use alloc::boxed::Box;
use core::fmt::Write;
use core::mem::MaybeUninit;
use evm_interpreter::gas_constants::CALLVALUE;
use evm_interpreter::gas_constants::CALL_STIPEND;
use evm_interpreter::gas_constants::NEWACCOUNT;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::B160;
use ruint::aliases::U256;
use system_hooks::*;
use zk_ee::common_structs::CalleeParameters;
use zk_ee::common_structs::TransferInfo;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::{
    errors::{system::SystemError, UpdateQueryError},
    logger::Logger,
    *,
};
use zk_ee::wrap_error;
use zk_ee::{internal_error, out_of_ergs_error};

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
            let (resources_returned, call_result) = run.handle_requested_external_call(
                initial_ee_version,
                true,
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
            .handle_requested_deployment(initial_ee_version, true, deployment_parameters, heap)
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

enum CallOrDeployResult<'a, S: EthereumLikeTypes> {
    CallResult(CallResult<'a, S>),
    DeploymentResult(DeploymentResult<'a, S>),
}

const SPECIAL_ADDRESS_BOUND: B160 = B160::from_limbs([SPECIAL_ADDRESS_SPACE_BOUND, 0, 0]);

impl<'external, S: EthereumLikeTypes> Run<'_, 'external, S> {
    #[inline(always)]
    fn handle_spawn<'s, 'a>(
        &'s mut self,
        ee_type: ExecutionEnvironmentType,
        spawn: ExecutionEnvironmentSpawnRequest<'a, S>,
        heap: SliceVec<'a, u8>,
    ) -> Result<(S::Resources, CallOrDeployResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        self.callstack_height += 1;
        let result = self.handle_spawn_inner(ee_type, spawn, heap);
        self.callstack_height -= 1;
        result
    }

    #[inline(always)]
    fn handle_spawn_inner<'a>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        spawn: ExecutionEnvironmentSpawnRequest<'a, S>,
        heap: SliceVec<'a, u8>,
    ) -> Result<(S::Resources, CallOrDeployResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        match spawn {
            ExecutionEnvironmentSpawnRequest::RequestedExternalCall(external_call_request) => {
                let (resources, call_result) = self.handle_requested_external_call(
                    ee_type,
                    false,
                    external_call_request,
                    heap,
                )?;

                let success = matches!(call_result, CallResult::Successful { .. });

                let _ = self.system.get_logger().write_fmt(format_args!(
                    "Return from external call, success = {}\n",
                    success
                ));

                Ok((resources, CallOrDeployResult::CallResult(call_result)))
            }
            ExecutionEnvironmentSpawnRequest::RequestedDeployment(deployment_parameters) => {
                let CompletedDeployment {
                    resources_returned,
                    deployment_result,
                } =
                    self.handle_requested_deployment(ee_type, false, deployment_parameters, heap)?;

                let returndata_region = deployment_result.returndata();
                let returndata_iter = returndata_region.iter().copied();
                let _ = self
                    .system
                    .get_logger()
                    .write_fmt(format_args!("Returndata = "));
                let _ = self.system.get_logger().log_data(returndata_iter);

                Ok((
                    resources_returned,
                    CallOrDeployResult::DeploymentResult(deployment_result),
                ))
            }
        }
    }

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

    fn handle_requested_external_call(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_entry_frame: bool,
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

        // The call is targeting the "system contract" space.
        if is_call_to_special_address {
            return self.handle_requested_external_call_to_special_address_space(
                ee_type,
                is_entry_frame,
                call_request,
            );
        }

        // NOTE: on external call request caller doesn't spend resources,
        // but indicates how much he would want to pass at most. Here we can decide the rest

        // we should create next EE and push to callstack
        // only system knows next EE version

        // NOTE: we should move to the frame of the CALLEE now, even though we still use resources of
        // CALLER to perform some reads. If we bail, then we will roll back the frame and all
        // potential writes below, otherwise we will pass what's needed to caller

        // declaring these here rather than returning them reduces stack usage.
        let (
            mut new_vm,
            new_ee_type,
            resources_returned_from_preparation,
            mut preemption,
            rollback_handle,
        );
        match run_call_preparation(is_entry_frame, self.system, ee_type, &call_request) {
            Ok(CallPreparationResult::Success {
                next_ee_version,
                bytecode,
                code_version,
                unpadded_code_len,
                artifacts_len,
                mut actual_resources_to_pass,
                transfer_to_perform,
                resources_returned,
            }) => {
                // We create a new frame for callee, should include transfer and
                // callee execution
                rollback_handle = self.system.start_global_frame()?;

                if let Some(call_result) = self.external_call_before_vm(
                    &mut actual_resources_to_pass,
                    &call_request,
                    bytecode.len() == 0,
                    &transfer_to_perform,
                    ee_type,
                )? {
                    let failure = !matches!(call_result, CallResult::Successful { .. });
                    self.system
                        .finish_global_frame(failure.then_some(&rollback_handle))?;
                    actual_resources_to_pass.reclaim(resources_returned);
                    return Ok((actual_resources_to_pass, call_result));
                }

                resources_returned_from_preparation = resources_returned;

                if DEBUG_OUTPUT {
                    let _ = self.system.get_logger().write_fmt(format_args!(
                        "Bytecode len for `callee` = {}\n",
                        bytecode.len(),
                    ));
                    let _ = self
                        .system
                        .get_logger()
                        .write_fmt(format_args!("Bytecode for `callee` = "));
                    let _ = self
                        .system
                        .get_logger()
                        .log_data(bytecode.as_ref().iter().copied());
                }

                // resources are checked and spent, so we continue with actual transition of control flow

                // now grow callstack and prepare initial state
                new_vm = create_ee(next_ee_version, self.system)?;
                new_ee_type = new_vm.ee_type();

                preemption = new_vm
                    .start_executing_frame(
                        self.system,
                        ExecutionEnvironmentLaunchParams {
                            external_call: ExternalCallRequest {
                                available_resources: actual_resources_to_pass,
                                ..call_request
                            },
                            environment_parameters: EnvironmentParameters {
                                bytecode: Bytecode::Decommitted {
                                    bytecode,
                                    unpadded_code_len,
                                    artifacts_len,
                                    code_version,
                                },
                                scratch_space_len: 0,
                            },
                        },
                        heap,
                    )
                    .map_err(wrap_error!())?;
            }

            Ok(CallPreparationResult::Failure { resources_returned }) => {
                return Ok((resources_returned, CallResult::CallFailedToExecute))
            }
            Err(e) => return Err(e),
        };

        loop {
            match preemption {
                ExecutionEnvironmentPreemptionPoint::Spawn {
                    ref mut request,
                    ref mut heap,
                } => {
                    let heap = core::mem::take(heap);
                    let request = core::mem::take(request);
                    let (resources, result) = self.handle_spawn(new_ee_type, request, heap)?;
                    drop(preemption);
                    preemption = match result {
                        CallOrDeployResult::CallResult(call_result) => new_vm
                            .continue_after_external_call(self.system, resources, call_result)
                            .map_err(wrap_error!())?,
                        CallOrDeployResult::DeploymentResult(deployment_result) => new_vm
                            .continue_after_deployment(self.system, resources, deployment_result)
                            .map_err(wrap_error!())?,
                    };
                }
                ExecutionEnvironmentPreemptionPoint::End(
                    TransactionEndPoint::CompletedExecution(CompletedExecution {
                        mut resources_returned,
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

                    resources_returned.reclaim(resources_returned_from_preparation);
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

    #[inline(always)]
    fn external_call_before_vm<'a>(
        &mut self,
        actual_resources_to_pass: &mut S::Resources,
        call_request: &ExternalCallRequest<S>,
        is_eoa: bool,
        transfer_to_perform: &Option<TransferInfo>,
        ee_type: ExecutionEnvironmentType,
    ) -> Result<Option<CallResult<'a, S>>, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // Now, perform transfer with infinite ergs
        if let Some(TransferInfo { value, target }) = transfer_to_perform {
            match actual_resources_to_pass.with_infinite_ergs(|inf_resources| {
                self.system.io.transfer_nominal_token_value(
                    ExecutionEnvironmentType::NoEE,
                    inf_resources,
                    &call_request.caller,
                    &target,
                    &value,
                )
            }) {
                Ok(()) => (),
                Err(UpdateQueryError::System(SystemError::LeafRuntime(
                    RuntimeError::OutOfErgs(_),
                ))) => {
                    return Err(internal_error!("Our of ergs on infinite").into());
                }
                Err(UpdateQueryError::System(SystemError::LeafDefect(e))) => return Err(e.into()),
                Err(UpdateQueryError::System(SystemError::LeafRuntime(
                    RuntimeError::OutOfNativeResources(loc),
                ))) => {
                    return Err(RuntimeError::OutOfNativeResources(loc).into());
                }
                Err(UpdateQueryError::NumericBoundsError) => {
                    // Insufficient balance
                    match ee_type {
                        ExecutionEnvironmentType::NoEE => {
                            unreachable!("Cannot be in NoEE deep in the callstack")
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
            }
        }

        // Calls to EOAs succeed with empty return value
        if is_eoa {
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

    #[inline(always)]
    fn handle_requested_external_call_to_special_address_space(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_entry_frame: bool,
        call_request: ExternalCallRequest<S>,
    ) -> Result<(S::Resources, CallResult<'external, S>), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let callee = call_request.callee;
        let address_low = callee.as_limbs()[0] as u16;

        let _ = self.system.get_logger().write_fmt(format_args!(
            "Call to special address 0x{:04x}\n",
            address_low
        ));
        let calldata_slice = &call_request.calldata;
        let calldata_iter = calldata_slice.iter().copied();
        let _ = self
            .system
            .get_logger()
            .write_fmt(format_args!("Calldata = "));
        let _ = self.system.get_logger().log_data(calldata_iter);

        let rollback_handle;
        let (mut actual_resources_to_pass, resources_to_return_from_preparation) =
            match run_call_preparation(is_entry_frame, self.system, ee_type, &call_request) {
                Ok(CallPreparationResult::Success {
                    mut actual_resources_to_pass,
                    transfer_to_perform,
                    resources_returned,
                    ..
                }) => {
                    // We create a new frame for callee, should include transfer and
                    // callee execution
                    rollback_handle = self.system.start_global_frame()?;

                    if let Some(call_result) = self.external_call_before_vm(
                        &mut actual_resources_to_pass,
                        &call_request,
                        false,
                        &transfer_to_perform,
                        ee_type,
                    )? {
                        let failure = !matches!(call_result, CallResult::Successful { .. });
                        self.system
                            .finish_global_frame(failure.then_some(&rollback_handle))?;

                        actual_resources_to_pass.reclaim(resources_returned);
                        return Ok((actual_resources_to_pass, call_result));
                    }

                    (actual_resources_to_pass, resources_returned)
                }
                Ok(CallPreparationResult::Failure { resources_returned }) => {
                    return Ok((resources_returned, CallResult::CallFailedToExecute))
                }
                Err(e) => return Err(e),
            };

        let return_memory = core::mem::take(&mut self.return_memory);
        let (res, remaining_memory) = self.hooks.try_intercept(
            address_low,
            ExternalCallRequest {
                available_resources: actual_resources_to_pass.clone(),
                ..call_request
            },
            ee_type as u8,
            self.system,
            return_memory,
        )?;
        // Reclaim unused return memory
        self.return_memory = remaining_memory;

        if let Some(system_hook_run_result) = res {
            let CompletedExecution {
                return_values,
                mut resources_returned,
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

            resources_returned.reclaim(resources_to_return_from_preparation);
            Ok((
                resources_returned,
                if reverted {
                    CallResult::Failed { return_values }
                } else {
                    CallResult::Successful { return_values }
                },
            ))
        } else {
            // it's an empty account for all the purposes, or default AA
            let _ = self.system.get_logger().write_fmt(format_args!(
                "Call to special address was not intercepted\n",
            ));
            self.system
                .finish_global_frame(None)
                .map_err(|_| internal_error!("must finish execution frame"))?;

            actual_resources_to_pass.reclaim(resources_to_return_from_preparation);
            Ok((
                actual_resources_to_pass,
                CallResult::Successful {
                    return_values: ReturnValues::empty(),
                },
            ))
        }
    }

    fn handle_requested_deployment(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_entry_frame: bool,
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
                    return Err(e.wrap(zk_ee::location!()));
                }
            };

        // resources returned back to caller
        if is_entry_frame {
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

        // EE made all the preparations and we are in callee's frame already
        let mut constructor = create_ee(ee_type as u8, self.system)?;
        let constructor_ee_type = constructor.ee_type();

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
                    UpdateQueryError::System(SystemError::LeafRuntime(
                        RuntimeError::OutOfNativeResources(loc),
                    )) => RuntimeError::OutOfNativeResources(loc).into(),
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
                        UpdateQueryError::System(SystemError::LeafRuntime(
                            RuntimeError::OutOfNativeResources(loc),
                        )) => RuntimeError::OutOfNativeResources(loc).into(),
                        _ => internal_error!(
                            "Must transfer value on deployment after check in preparation",
                        )
                        .into(),
                    }
                })?;
        }

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
                    let (resources, result) =
                        self.handle_spawn(constructor_ee_type, request, heap)?;
                    drop(preemption);
                    preemption = match result {
                        CallOrDeployResult::CallResult(call_result) => constructor
                            .continue_after_external_call(self.system, resources, call_result)
                            .map_err(wrap_error!())?,
                        CallOrDeployResult::DeploymentResult(deployment_result) => constructor
                            .continue_after_deployment(self.system, resources, deployment_result)
                            .map_err(wrap_error!())?,
                    };
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
                            "Successfully deployed contract at {:?} \n",
                            deployed_at
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

        // Now finish constructor frame
        self.system
            .finish_global_frame((!deployment_success).then_some(&constructor_rollback_handle))?;

        let _ = self.system.get_logger().write_fmt(format_args!(
            "Return from constructor call, success = {}\n",
            deployment_success
        ));

        resources_returned.reclaim(resources_for_deployer);
        Ok(CompletedDeployment {
            resources_returned,
            deployment_result,
        })
    }
}

pub enum CallPreparationResult<'a, S: SystemTypes> {
    Success {
        next_ee_version: u8,
        bytecode: &'a [u8],
        code_version: u8,
        unpadded_code_len: u32,
        artifacts_len: u32,
        actual_resources_to_pass: S::Resources,
        transfer_to_perform: Option<TransferInfo>,
        resources_returned: S::Resources,
    },
    Failure {
        resources_returned: S::Resources,
    },
}

/// Reads callee account and runs call preparation function
/// from the system. Additionally, does token transfer if needed.
fn run_call_preparation<'a, S: EthereumLikeTypes>(
    is_entry_frame: bool,
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
    call_request: &ExternalCallRequest<S>,
) -> Result<CallPreparationResult<'a, S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
{
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
        code_version,
        unpadded_code_len,
        artifacts_len,
        stipend,
        transfer_to_perform,
    } = match r {
        Ok(x) => x,
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            return Ok(CallPreparationResult::Failure {
                resources_returned: resources_available,
            });
        }
        Err(SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc))) => {
            return Err(RuntimeError::OutOfNativeResources(loc).into())
        }
        Err(SystemError::LeafDefect(e)) => return Err(e.into()),
    };

    // If we're in the entry frame, i.e. not the execution of a CALL opcode,
    // we don't apply the CALL-specific gas charging, but instead set
    // actual_resources_to_pass equal to the available resources
    let mut actual_resources_to_pass = if !is_entry_frame {
        // now we should ask current EE for observable resource behavior if needed
        {
            SupportedEEVMState::<S>::clarify_and_take_passed_resources(
                ee_version,
                &mut resources_available,
                call_request.ergs_to_pass,
            )
            .map_err(wrap_error!())?
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
        code_version,
        unpadded_code_len,
        artifacts_len,
        actual_resources_to_pass,
        transfer_to_perform,
        resources_returned: resources_available,
    })
}

// It should be split into EVM and generic part.
/// Run call preparation, which includes reading the callee parameters
/// and charging for resources.
fn prepare_for_call<'a, S: EthereumLikeTypes>(
    system: &mut System<S>,
    ee_version: ExecutionEnvironmentType,
    resources: &mut S::Resources,
    call_request: &ExternalCallRequest<S>,
    is_entry_frame: bool,
) -> Result<CalleeParameters<'a>, SystemError>
where
    S::IO: IOSubsystemExt,
{
    // IO will follow the rules of the CALLER (`initial_ee_version`) here to charge for execution
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

    // Now we charge for the rest of the CALL related costs
    let stipend = if !is_entry_frame {
        match ee_version {
            ExecutionEnvironmentType::NoEE => {
                return Err(internal_error!("Cannot be NoEE deep in the callstack").into())
            }
            ExecutionEnvironmentType::EVM => {
                let is_delegate = call_request.is_delegate();
                let is_callcode = call_request.is_callcode();
                let is_callcode_or_delegate = is_callcode || is_delegate;

                // Positive value cost and stipend
                let stipend = if !is_delegate && !call_request.nominal_token_value.is_zero() {
                    let positive_value_cost =
                        S::Resources::from_ergs(Ergs(CALLVALUE * ERGS_PER_GAS));
                    resources.charge(&positive_value_cost)?;
                    Some(Ergs(CALL_STIPEND * ERGS_PER_GAS))
                } else {
                    None
                };

                // Account creation cost
                let callee_is_empty = account_properties.nonce.0 == 0
                    && account_properties.unpadded_code_len.0 == 0
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
        }
    } else {
        None
    };

    // Check transfer is allowed an determine transfer target
    let transfer_to_perform =
        if call_request.nominal_token_value != U256::ZERO && !call_request.is_delegate() {
            if !call_request.is_transfer_allowed() {
                let _ = system.get_logger().write_fmt(format_args!(
                    "Call failed: positive value with modifier {:?}\n",
                    call_request.modifier
                ));
                return Err(out_of_ergs_error!());
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

    // Read required data to perform a call
    let (next_ee_version, bytecode, code_version, unpadded_code_len, artifacts_len) = {
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

    Ok(CalleeParameters {
        next_ee_version,
        bytecode,
        code_version,
        unpadded_code_len,
        artifacts_len,
        stipend,
        transfer_to_perform,
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
