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

use super::gas::{code_deposit_costs, CALL_STIPEND, CALL_VALUE, MAX_CODE_SIZE, NEW_ACCOUNT};
use super::ops_cold::copy_returndata_to_heap;
use super::GlamsterdamTypes;
use super::{ColdFrameParts, Env, HotFrameParts, Interpreter};
use crate::ee_trait_impl::{
    check_depth_and_balance, constructor_pre_checks, emit_pre_frame_call_error,
};
use crate::errors::{EvmErrors, EvmInterfaceError, EvmSubsystemError};
use crate::evm_stack::EvmStack;
use crate::gas::gas_utils;
use crate::{
    BytecodePreprocessingData, ExitCode, PendingOsRequest, ARTIFACTS_CACHING_CODE_VERSION_BYTE,
    ARTIFACTS_FROM_CODE_CACHE_CODE_VERSION_BYTE, DEFAULT_CODE_VERSION_BYTE, THIS_EE_TYPE,
};

/// Whether a frame's caller pays the state gas of creating the frame's account (EIP-8037):
/// a deployment, or a value transfer by a plain call, to an account that is not alive (it
/// does not exist, or is empty)
fn account_creation_charged(
    callee: &CalleeAccountProperties,
    modifier: CallModifier,
    value: &ruint::aliases::U256,
) -> bool {
    let alive = callee.nonce != 0
        || !callee.nominal_token_balance.is_zero()
        || callee.bytecode.has_code() != Some(false);
    !alive
        && match modifier {
            CallModifier::Constructor => true,
            CallModifier::NoModifier => !value.is_zero(),
            _ => false,
        }
}

#[cfg(not(target_arch = "riscv32"))]
use super::FrameView;

impl<'ee, S: GlamsterdamTypes> Interpreter<'ee, S> {
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
            // a failed frame refills its state charges, and the creation of its account
            settle_failed_frame::<S>(&mut self.hot.resources, &self.cold);
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
            } else if let Err(error) =
                charge_code_deposit::<S>(&mut self.hot.resources, deployed_code.len())
            {
                error_after_constructor = Some(error);
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
                settle_failed_frame::<S>(&mut self.hot.resources, &self.cold);
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

/// The state gas settlement of a frame that fails (EIP-8037): the state charges since its
/// entry are refilled, and so is the creation of its account, which the caller charged and
/// which does not happen
fn settle_failed_frame<S: GlamsterdamTypes>(
    resources: &mut S::Resources,
    cold: &ColdFrameParts<'_, S>,
) {
    resources.restore_state_gas(cold.state_gas_baseline);
    if cold.account_creation_charged {
        resources.credit_state_gas(NEW_ACCOUNT);
    }
}

/// Charges the deposit of deployed code: its hashing in execution gas and its storage in
/// state gas (EIP-8037)
fn charge_code_deposit<S: GlamsterdamTypes>(
    resources: &mut S::Resources,
    code_len: usize,
) -> Result<(), EvmError> {
    let (hashing_gas, storage_state_gas) = code_deposit_costs(code_len);
    resources
        .charge_legacy_gas(hashing_gas)
        .map_err(|_| EvmError::CodeStoreOutOfGas)?;
    resources
        .charge_state_gas(storage_state_gas)
        .map_err(|_| EvmError::CodeStoreOutOfGas)
}

impl<'ee, S: GlamsterdamTypes> ExecutionEnvironment<'ee, S, EvmErrors> for Interpreter<'ee, S> {
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
            state_gas_baseline: 0,
            account_creation_charged: false,
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
            callstack_depth,
            callee_account_properties,
        } = environment_parameters;

        let mut is_static = false;
        let mut is_constructor = false;
        let mut caller_address = caller;
        let mut this_address = callee;

        self.cold.state_gas_baseline = available_resources.state_gas();
        // The creation of the root frame's account is charged by the transaction flow
        // before dispatch, as a spill the frame's restore to its baseline repays on a
        // failure: crediting it here as well would refill it twice. Only a frame entered
        // from a call or a creation refills what its caller charged.
        self.cold.account_creation_charged = callstack_depth > 0
            && account_creation_charged(&callee_account_properties, modifier, &nominal_token_value);

        if modifier == CallModifier::Constructor {
            // the code to execute is in the calldata
            let mut preprocessing = BytecodePreprocessingData::create_artifacts(
                system.get_allocator(),
                calldata,
                &mut available_resources,
            )?;
            let code = if S::CODE_IS_PADDED {
                // the interpreter reads past the end of padded code; the calldata is not padded
                preprocessing.take_padded_copy(system.get_allocator(), calldata)
            } else {
                calldata
            };
            self.cold.set_code(code, preprocessing);
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
                    let mut preprocessing = BytecodePreprocessingData::create_artifacts(
                        system.get_allocator(),
                        bytecode,
                        &mut available_resources,
                    )?;
                    let code = if S::CODE_IS_PADDED {
                        // code without cached artifacts comes from an unpadded buffer
                        preprocessing.take_padded_copy(system.get_allocator(), bytecode)
                    } else {
                        bytecode
                    };
                    self.cold.set_code(code, preprocessing);
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
        // the heap is zeroed and copied by whole 32-byte slots (see `resize_heap`), which
        // needs the backing memory 32 bytes aligned: the bootloader aligns the root buffer
        // and every frame's heap starts at a multiple of 32 in it
        assert!(
            heap.memory_ptr().addr().is_multiple_of(32),
            "frame heap must be 32 bytes aligned"
        );
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
            // stipend for a positive value; its cost was charged in `before_reading_callee`
            stipend = if !call_request.is_delegate() && !call_request.nominal_token_value.is_zero()
            {
                Some(<S::Resources as Resources>::Ergs::from_legacy_gas_saturating(CALL_STIPEND))
            } else {
                None
            };
        }
        if callee_parameters.bytecode.has_code().is_none() {
            return Err(internal_error!("callee code state is unknown").into());
        }

        // The creation of the callee's account is state gas (EIP-8037), charged before the
        // child's share is computed (it may spill into the execution gas); the callee
        // refills it if it fails (`settle_failed_frame`)
        if account_creation_charged(
            callee_parameters,
            call_request.modifier,
            &call_request.nominal_token_value,
        ) {
            resources_available_in_caller_frame.charge_state_gas(NEW_ACCOUNT)?;
        }

        // 63/64 rule on the execution gas; the system is responsible for the rest
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
        // the whole state gas reservoir rides along (no 63/64 rule for it)
        resources_to_pass.add_state_gas(resources_available_in_caller_frame.drain_state_gas());
        Ok(resources_to_pass)
    }

    fn before_reading_callee<'a, 'i: 'ee, 'h: 'ee>(
        system: &mut System<S>,
        call_request: &mut ExternalCallRequest<S>,
        callstack_depth: usize,
        tracer: &mut impl Tracer<S>,
    ) -> Result<zk_ee::system::CalleePreCheck, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // CREATE(2) fails before warming up the callee on depth, balance or nonce overflow;
        // a CALL still warms it up in the first two cases.
        if call_request.modifier == CallModifier::Constructor {
            if let Some(error) = constructor_pre_checks(system, call_request, callstack_depth)? {
                emit_pre_frame_call_error(call_request, callstack_depth, tracer, &error);
                return Ok(zk_ee::system::CalleePreCheck::Failed);
            }
        } else if callstack_depth > 0
            && !call_request.is_delegate()
            && !call_request.nominal_token_value.is_zero()
        {
            // The value transfer cost is paid before the callee is loaded (as revm does,
            // and unlike the cold/new-account costs, which need the callee): a call that
            // can not afford it fails without warming the callee up, and the callee is
            // then absent from the block's witness. The entry frame (depth 0) pays it as
            // part of the intrinsic gas instead.
            match call_request
                .available_resources
                .charge_legacy_gas(CALL_VALUE)
            {
                Ok(()) => {}
                Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                    return Ok(zk_ee::system::CalleePreCheck::OutOfErgs);
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(zk_ee::system::CalleePreCheck::Proceed)
    }

    fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        system: &mut System<S>,
        frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<bool, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // A call that never enters its frame refills the creation of its account, which
        // its caller charged: credited to the callee's resources, which go back to the
        // caller (the same as crediting the caller after the merge). The root frame's
        // charge is the transaction flow's, repaid by the frame's restore: never here.
        let creation_charged = frame_state.environment_parameters.callstack_depth > 0
            && account_creation_charged(
                &frame_state.environment_parameters.callee_account_properties,
                frame_state.external_call.modifier,
                &frame_state.external_call.nominal_token_value,
            );
        if let Some(error) = check_depth_and_balance(
            system,
            &mut frame_state.external_call,
            frame_state.environment_parameters.callstack_depth,
        )? {
            if creation_charged {
                frame_state
                    .external_call
                    .available_resources
                    .credit_state_gas(NEW_ACCOUNT);
            }
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
                        if creation_charged {
                            frame_state
                                .external_call
                                .available_resources
                                .credit_state_gas(NEW_ACCOUNT);
                        }
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
