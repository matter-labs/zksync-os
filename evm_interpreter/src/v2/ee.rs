//! The frame API of the interpreter: `ExecutionEnvironment` and the run around the loop.

use core::fmt::Write;
use core::mem;

use u256::U256;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::common_structs::{BytecodeData, CalleeAccountProperties};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::evm::EvmError;
use zk_ee::system::tracer::evm_tracer::EvmTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::*;
use zk_ee::system_log;
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::{interface_error, internal_error, wrap_error};

use super::ops_cold::copy_returndata_to_heap;
use super::{ColdFrameParts, Env, HotFrameParts, Interpreter};
use crate::ee_trait_impl::{
    check_depth_and_balance, constructor_pre_checks, emit_pre_frame_call_error,
};
use crate::errors::{EvmErrors, EvmInterfaceError, EvmSubsystemError};
use crate::evm_stack::EvmStack;
use crate::gas::gas_utils;
use crate::gas_constants::{CALLVALUE, CALL_STIPEND, NEWACCOUNT};
use crate::{
    BytecodePreprocessingData, ExitCode, PendingOsRequest, ARTIFACTS_CACHING_CODE_VERSION_BYTE,
    ARTIFACTS_FROM_CODE_CACHE_CODE_VERSION_BYTE, DEFAULT_CODE_VERSION_BYTE, MAX_CODE_SIZE,
    THIS_EE_TYPE,
};

#[cfg(not(target_arch = "riscv32"))]
use super::FrameView;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    /// Runs the loop and returns why it stopped
    pub fn run(
        &mut self,
        system: &mut System<S>,
        hooks: &mut HooksStorage<S, S::Allocator>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExitCode, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let mut env = Env::new(system, hooks, tracer);
        self.exit_code = None;
        self.run_loop(&mut env);
        let result = self
            .exit_code
            .take()
            .ok_or(internal_error!("interpreter stopped without an exit code"))?;
        #[cfg(not(target_arch = "riscv32"))]
        system_log!(
            env.system,
            "Instructions executed = {}\nFinal instruction result = {:?}\n",
            env.cycles,
            &result
        );
        Ok(result)
    }

    /// Keeps executing instructions until a yield point: an error, a return, or a request
    /// to call or create a contract.
    pub fn execute_till_yield_point<'a>(
        &'a mut self,
        system: &mut System<S>,
        hooks: &mut HooksStorage<S, S::Allocator>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let exit_code = self.run(system, hooks, tracer)?;

        match exit_code {
            ExitCode::FatalError(e) => return Err(e),
            ExitCode::FatalRuntime(f) => return Err(RuntimeError::FatalRuntimeError(f).into()),
            _ => {}
        }

        if let Some(call) = self.cold.pending_call.take() {
            assert!(exit_code == ExitCode::ExternalCall);
            let (current_heap, next_heap) = self.cold.heap.freeze();
            let request = ExternalCallRequest {
                available_resources: call.full_caller_resources,
                ergs_to_pass: call.ergs_to_pass,
                caller: self.cold.address,
                callee: call.destination_address,
                callers_caller: self.cold.caller,
                modifier: call.modifier,
                input: &current_heap[call.input_data],
                nominal_token_value: call.call_value,
                call_scratch_space: None,
            };
            return Ok(ExecutionEnvironmentPreemptionPoint::CallRequest {
                heap: next_heap,
                request,
            });
        }

        self.create_immediate_return_state(system, exit_code, tracer)
    }

    pub(crate) fn create_immediate_return_state<'a>(
        &'a mut self,
        system: &mut System<S>,
        exit_code: ExitCode,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        #[cfg(target_arch = "riscv32")]
        let _ = &tracer;
        let mut return_values = ReturnValues::empty();
        match exit_code {
            ExitCode::Return | ExitCode::EvmError(EvmError::Revert) => {
                return_values.returndata = &self.cold.heap[self.cold.returndata_location.clone()];
            }
            ExitCode::Stop | ExitCode::SelfDestruct | ExitCode::EvmError(_) => (),
            ExitCode::ExternalCall | ExitCode::FatalError(_) | ExitCode::FatalRuntime(_) => {
                return Err(internal_error!("Invalid exit code passed").into())
            }
        };

        if let ExitCode::EvmError(evm_error) = exit_code {
            if evm_error != EvmError::Revert {
                // an EVM error consumes all remaining gas and returns nothing
                self.hot.resources.exhaust_ergs();
                return_values.returndata = &[];
            }
            #[cfg(not(target_arch = "riscv32"))]
            tracer.evm_tracer().on_opcode_error(
                &evm_error,
                &FrameView::from_parts(&self.hot, &self.cold, &*system),
            );
            return Ok(ExecutionEnvironmentPreemptionPoint::End(
                CompletedExecution {
                    resources_returned: self.hot.resources.take(),
                    result: CallResult::Failed { return_values },
                },
            ));
        };

        let result = if self.cold.is_constructor {
            let deployed_code = return_values.returndata;
            let mut error_after_constructor = None;
            if deployed_code.len() > MAX_CODE_SIZE {
                // EIP-170
                error_after_constructor = Some(EvmError::CreateContractSizeLimit)
            } else if !deployed_code.is_empty() && deployed_code[0] == 0xEF {
                // EIP-3541
                error_after_constructor = Some(EvmError::CreateContractStartingWithEF);
            } else {
                match system.deploy_bytecode(
                    THIS_EE_TYPE,
                    &mut self.hot.resources,
                    &self.cold.address,
                    deployed_code,
                ) {
                    Ok((
                        actual_deployed_bytecode,
                        internal_bytecode_hash,
                        observable_bytecode_len,
                    )) => {
                        system_log!(
                            system,
                            "Successfully deployed contract at {:?} \n",
                            self.cold.address
                        );
                        #[cfg(not(target_arch = "riscv32"))]
                        tracer.on_bytecode_change(
                            THIS_EE_TYPE,
                            self.cold.address,
                            Some(actual_deployed_bytecode),
                            internal_bytecode_hash,
                            observable_bytecode_len,
                        );
                        #[cfg(target_arch = "riscv32")]
                        let _ = (
                            actual_deployed_bytecode,
                            internal_bytecode_hash,
                            observable_bytecode_len,
                        );
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                        error_after_constructor = Some(EvmError::CodeStoreOutOfGas);
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(e))) => {
                        return Err(RuntimeError::FatalRuntimeError(e).into())
                    }
                    Err(SystemError::LeafDefect(e)) => return Err(e.into()),
                }
            }

            if let Some(error) = error_after_constructor {
                self.hot.resources.exhaust_ergs();
                #[cfg(not(target_arch = "riscv32"))]
                tracer.evm_tracer().on_opcode_error(
                    &error,
                    &FrameView::from_parts(&self.hot, &self.cold, &*system),
                );
                #[cfg(target_arch = "riscv32")]
                let _ = error;
                CallResult::Failed {
                    return_values: ReturnValues::empty(),
                }
            } else {
                CallResult::Successful {
                    return_values: ReturnValues::empty(),
                }
            }
        } else {
            CallResult::Successful { return_values }
        };

        Ok(ExecutionEnvironmentPreemptionPoint::End(
            CompletedExecution {
                resources_returned: self.hot.resources.take(),
                result,
            },
        ))
    }
}

impl<'ee, S: EthereumLikeTypes> ExecutionEnvironment<'ee, S, EvmErrors> for Interpreter<'ee, S> {
    const NEEDS_SCRATCH_SPACE: bool = false;

    const EE_VERSION_BYTE: u8 = ExecutionEnvironmentType::EVM_EE_BYTE;

    type UsageError = <EvmErrors as zk_ee::system::errors::subsystem::Subsystem>::Interface;
    type SubsystemError = EvmSubsystemError;

    fn new(system: &mut System<S>) -> Result<Self, Self::SubsystemError> {
        let empty_address = <S::IOTypes as SystemIOTypesConfig>::Address::default();
        let mut cold = ColdFrameParts {
            returndata: &[],
            is_static: false,
            caller: empty_address,
            address: empty_address,
            calldata: &[],
            heap: SliceVec::new(&mut []),
            returndata_location: 0..0,
            bytecode: &[],
            bytecode_preprocessing: BytecodePreprocessingData::empty(),
            jumpdest_words: core::ptr::null(),
            call_value: U256::zero(),
            is_constructor: false,
            gas_paid_for_heap_growth: 0,
            pending_os_request: None,
            pending_call: None,
        };
        cold.set_code(&[], BytecodePreprocessingData::empty());
        Ok(Self {
            hot: HotFrameParts {
                instruction_pointer: 0,
                stack: EvmStack::new_in(system.get_allocator()),
                resources: S::Resources::empty(),
            },
            cold,
            exit_code: None,
        })
    }

    fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        hooks: &mut HooksStorage<S, S::Allocator>,
        frame_state: ExecutionEnvironmentLaunchParams<'i, S>,
        heap: SliceVec<'h, u8>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let ExecutionEnvironmentLaunchParams {
            external_call:
                ExternalCallRequest {
                    ergs_to_pass: _,
                    mut available_resources,
                    caller,
                    callee,
                    callers_caller,
                    modifier,
                    input: mut calldata,
                    call_scratch_space,
                    nominal_token_value,
                },
            environment_parameters,
        } = frame_state;
        assert!(call_scratch_space.is_none());

        let EnvironmentParameters {
            scratch_space_len: _,
            callstack_depth: _,
            callee_account_properties,
        } = environment_parameters;

        let mut is_static = false;
        let mut is_constructor = false;
        let mut caller_address = caller;
        let mut this_address = callee;

        if modifier == CallModifier::Constructor {
            // the code to execute is in the calldata
            let preprocessing = BytecodePreprocessingData::create_artifacts(
                system.get_allocator(),
                calldata,
                &mut available_resources,
            )?;
            self.cold.set_code(calldata, preprocessing);
        } else {
            // a call always runs on loaded code; only a deployment target's code is left unloaded
            let BytecodeData::Available {
                bytecode,
                unpadded_code_len,
                artifacts_len,
            } = callee_account_properties.bytecode
            else {
                return Err(internal_error!("callee code is not loaded for a call").into());
            };
            match callee_account_properties.code_version {
                DEFAULT_CODE_VERSION_BYTE => {
                    assert_eq!(artifacts_len, 0);
                    let preprocessing = BytecodePreprocessingData::create_artifacts(
                        system.get_allocator(),
                        bytecode,
                        &mut available_resources,
                    )?;
                    self.cold.set_code(bytecode, preprocessing);
                }
                ARTIFACTS_CACHING_CODE_VERSION_BYTE => {
                    let (code, preprocessing) = BytecodePreprocessingData::parse_bytecode(
                        bytecode,
                        unpadded_code_len as usize,
                        artifacts_len as usize,
                    )?;
                    self.cold.set_code(code, preprocessing);
                }
                ARTIFACTS_FROM_CODE_CACHE_CODE_VERSION_BYTE => {
                    // charged as `DEFAULT_CODE_VERSION_BYTE` is
                    BytecodePreprocessingData::<S::Allocator>::charge_for_artifacts(
                        unpadded_code_len as usize,
                        &mut available_resources,
                    )?;
                    let (code, preprocessing) = BytecodePreprocessingData::parse_bytecode(
                        bytecode,
                        unpadded_code_len as usize,
                        artifacts_len as usize,
                    )?;
                    self.cold.set_code(code, preprocessing);
                }
                _ => return Err(internal_error!("Unknown code version").into()),
            }
        };

        match modifier {
            CallModifier::NoModifier => {}
            CallModifier::Delegate => {
                caller_address = callers_caller;
                this_address = caller;
            }
            CallModifier::Static => is_static = true,
            CallModifier::DelegateStatic => {
                caller_address = callers_caller;
                this_address = caller;
                is_static = true;
            }
            CallModifier::Constructor => {
                // EIP-161: the constructor sees the deployed address with nonce 1
                available_resources
                    .with_infinite_ergs(|inf_resources| {
                        system
                            .io
                            .increment_nonce(THIS_EE_TYPE, inf_resources, &this_address, 1)
                    })
                    .map_err(|e| -> EvmSubsystemError {
                        match e {
                            SubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_)) => {
                                wrap_error!(e)
                            }
                            _ => internal_error!("Failed to set deployed nonce to 1").into(),
                        }
                    })?;
                is_constructor = true;
                calldata = &[];
            }
            CallModifier::EVMCallcode => {
                this_address = caller;
            }
            CallModifier::EVMCallcodeStatic => {
                this_address = caller;
                is_static = true;
            }
            a => {
                return Err(interface_error!(EvmInterfaceError::UnexpectedModifier {
                    modifier: a
                }))
            }
        }

        assert!(
            self.hot.resources == S::Resources::empty(),
            "for a fresh call resources of initial frame must be empty",
        );

        self.hot.resources = available_resources;
        self.hot.instruction_pointer = 0;
        self.cold.address = this_address;
        self.cold.caller = caller_address;
        self.cold.is_static = is_static;
        self.cold.is_constructor = is_constructor;
        self.cold.calldata = calldata;
        self.cold.heap = heap;
        self.cold.call_value = U256::from(nominal_token_value);

        self.execute_till_yield_point(system, hooks, tracer)
    }

    /// Note: panics if `pending_os_request` is None
    fn continue_after_preemption<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        hooks: &mut HooksStorage<S, S::Allocator>,
        returned_resources: S::Resources,
        call_request_result: CallResult<'res, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let preemption_reason = match mem::take(&mut self.cold.pending_os_request) {
            Some(x) => x,
            None => {
                return Err(interface_error!(
                    EvmInterfaceError::InvalidReenterAfterPreemtion
                ))
            }
        };

        if call_request_result.has_scratch_space() {
            return Err(internal_error!("Unexpected scratch space").into());
        }
        if self.hot.resources.native().as_u64() != 0 {
            return Err(internal_error!("Invalid initial native resources").into());
        }

        self.hot.resources.reclaim(returned_resources);

        match call_request_result {
            CallResult::PreparationStepFailed => {
                system_log!(system, "Call failed, out of gas\n");
                // the caller's failure
                let exit_code = EvmError::OutOfGas.into();
                return self.create_immediate_return_state(system, exit_code, tracer);
            }
            CallResult::Failed { return_values } => {
                match preemption_reason {
                    PendingOsRequest::Call => {
                        if let Err(exit_code) = copy_returndata_to_heap(
                            &mut self.cold,
                            &mut self.hot.resources,
                            return_values.returndata,
                        ) {
                            return self.create_immediate_return_state(system, exit_code, tracer);
                        }
                    }
                    PendingOsRequest::Create(_) => {
                        // failed deployments may have non-empty returndata
                        assert!(self.cold.returndata_location.is_empty());
                        assert!(return_values.return_scratch_space.is_none());
                        self.cold.returndata = return_values.returndata;
                    }
                }
                self.hot.stack.push_zero().expect("must have enough space");
            }
            CallResult::Successful { return_values } => match preemption_reason {
                PendingOsRequest::Call => {
                    if let Err(exit_code) = copy_returndata_to_heap(
                        &mut self.cold,
                        &mut self.hot.resources,
                        return_values.returndata,
                    ) {
                        return self.create_immediate_return_state(system, exit_code, tracer);
                    }
                    self.hot.stack.push_one().expect("must have enough space");
                }
                PendingOsRequest::Create(deployed_at) => {
                    assert!(return_values.return_scratch_space.is_none());
                    // successful deployments have empty returndata
                    assert!(return_values.returndata.is_empty());
                    self.cold.returndata = return_values.returndata;
                    self.hot
                        .stack
                        .push(&U256::from_b160(deployed_at))
                        .expect("must have enough space");
                }
            },
        }

        self.execute_till_yield_point(system, hooks, tracer)
    }

    fn calculate_resources_passed_in_external_call(
        resources_available_in_caller_frame: &mut S::Resources,
        call_request: &ExternalCallRequest<S>,
        callee_parameters: &CalleeAccountProperties,
    ) -> Result<S::Resources, Self::SubsystemError> {
        let mut stipend = None;

        if call_request.modifier != CallModifier::Constructor {
            let is_delegate = call_request.is_delegate();
            let is_callcode = call_request.is_callcode();
            let is_callcode_or_delegate = is_callcode || is_delegate;

            // positive value cost and stipend
            stipend = if !is_delegate && !call_request.nominal_token_value.is_zero() {
                resources_available_in_caller_frame.charge_legacy_gas(CALLVALUE)?;
                Some(<S::Resources as Resources>::Ergs::from_legacy_gas_saturating(CALL_STIPEND))
            } else {
                None
            };

            // account creation cost
            let Some(callee_has_code) = callee_parameters.bytecode.has_code() else {
                return Err(internal_error!("callee code state is unknown").into());
            };
            let callee_is_empty = callee_parameters.nonce == 0
                && !callee_has_code
                && callee_parameters.nominal_token_balance.is_zero();
            if !is_callcode_or_delegate
                && !call_request.nominal_token_value.is_zero()
                && callee_is_empty
            {
                resources_available_in_caller_frame.charge_legacy_gas(NEWACCOUNT)?
            }
        }

        // 63/64 rule; the system is responsible for the rest
        let max_passable_ergs =
            gas_utils::apply_63_64_rule(resources_available_in_caller_frame.ergs());
        let ergs_to_pass = core::cmp::min(call_request.ergs_to_pass, max_passable_ergs);
        let mut resources_to_pass = S::Resources::from_ergs(ergs_to_pass);
        // never fails: max_passable_ergs <= resources_available_in_caller_frame
        resources_available_in_caller_frame
            .charge(&resources_to_pass)
            .unwrap();
        if let Some(stipend) = stipend {
            resources_to_pass.add_ergs(stipend);
        }
        Ok(resources_to_pass)
    }

    fn before_reading_callee<'a, 'i: 'ee, 'h: 'ee>(
        system: &mut System<S>,
        call_request: &mut ExternalCallRequest<S>,
        callstack_depth: usize,
        tracer: &mut impl Tracer<S>,
    ) -> Result<bool, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // CREATE(2) fails before warming up the callee on depth, balance or nonce overflow;
        // a CALL still warms it up in the first two cases.
        if call_request.modifier == CallModifier::Constructor {
            if let Some(error) = constructor_pre_checks(system, call_request, callstack_depth)? {
                emit_pre_frame_call_error(call_request, callstack_depth, tracer, &error);
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        system: &mut System<S>,
        frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<bool, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        if let Some(error) = check_depth_and_balance(
            system,
            &mut frame_state.external_call,
            frame_state.environment_parameters.callstack_depth,
        )? {
            tracer.evm_tracer().on_call_error(&error);
            return Ok(false);
        }

        if frame_state.external_call.modifier == CallModifier::Constructor {
            // the root frame's nonce was incremented before
            if frame_state.environment_parameters.callstack_depth > 0 {
                match frame_state
                    .external_call
                    .available_resources
                    .with_infinite_ergs(|inf_resources| {
                        system.io.increment_nonce(
                            THIS_EE_TYPE,
                            inf_resources,
                            &frame_state.external_call.caller,
                            1u64,
                        )
                    }) {
                    Ok(_) => {}
                    Err(SubsystemError::LeafUsage(InterfaceError(
                        NonceError::NonceOverflow,
                        _,
                    ))) => {
                        tracer.evm_tracer().on_call_error(&EvmError::NonceOverflow);
                        return Ok(false);
                    }
                    Err(e) => return Err(wrap_error!(e)),
                };
            };

            let Some(deployee_has_code) = frame_state
                .environment_parameters
                .callee_account_properties
                .bytecode
                .has_code()
            else {
                return Err(internal_error!("deployment target code state is unknown").into());
            };
            let deployee_nonce = frame_state
                .environment_parameters
                .callee_account_properties
                .nonce;
            // No contract may already be deployed at the address; checked here because the
            // constructor must not run then.
            if deployee_has_code || deployee_nonce != 0 {
                system_log!(system, "Deployment on existing account\n",);
                frame_state
                    .external_call
                    .available_resources
                    .charge(&S::Resources::from_ergs(
                        frame_state.external_call.available_resources.ergs(),
                    ))
                    .expect("Should succeed"); // burn all gas
                tracer
                    .evm_tracer()
                    .on_call_error(&EvmError::CreateCollision);
                return Ok(false);
            }
        }
        Ok(true)
    }
}
