use super::*;
use crate::errors::{EvmErrors, EvmInterfaceError, EvmSubsystemError};
use crate::gas::gas_utils;
use crate::gas_constants::{CALLVALUE, CALL_STIPEND, NEWACCOUNT};
use crate::interpreter::CreateScheme;
use core::fmt::Write;
use core::mem;
use zk_ee::common_structs::CalleeAccountProperties;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::tracer::evm_tracer::EvmTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::*;
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::b160_to_u256;
use zk_ee::{interface_error, internal_error, wrap_error};

impl<S: SystemTypes> EEDeploymentExtraParameters<S> for CreateScheme {}

impl<'ee, S: EthereumLikeTypes> ExecutionEnvironment<'ee, S, EvmErrors> for Interpreter<'ee, S> {
    const NEEDS_SCRATCH_SPACE: bool = false;

    const EE_VERSION_BYTE: u8 = ExecutionEnvironmentType::EVM_EE_BYTE;

    type UsageError = <EvmErrors as zk_ee::system::errors::subsystem::Subsystem>::Interface;
    type SubsystemError = EvmSubsystemError;

    fn new(system: &mut System<S>) -> Result<Self, Self::SubsystemError> {
        let gas = Gas::new();
        let stack_space = EvmStack::new_in(system.get_allocator());
        let empty_address = <S::IOTypes as SystemIOTypesConfig>::Address::default();
        let empty_preprocessing = BytecodePreprocessingData::empty();

        Ok(Self {
            instruction_pointer: 0,
            gas,
            stack: stack_space,
            returndata: &[],
            is_static: false,
            caller: empty_address,
            address: empty_address,
            calldata: &[],
            heap: SliceVec::new(&mut []),
            returndata_location: 0..0,
            bytecode: &[],
            bytecode_preprocessing: empty_preprocessing,
            call_value: U256::ZERO,
            is_constructor: false,
            pending_os_request: None,
        })
    }

    fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
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

        // Set bytecode
        if modifier == CallModifier::Constructor {
            // Code to execute is in calldata
            let bytecode_preprocessing = BytecodePreprocessingData::create_artifacts(
                system.get_allocator(),
                calldata,
                &mut available_resources,
            )?;
            self.bytecode = calldata;
            self.bytecode_preprocessing = bytecode_preprocessing;
        } else {
            // Execute actual decommited bytecode provided by OS
            let bytecode = callee_account_properties.bytecode;
            let unpadded_code_len = callee_account_properties.unpadded_code_len;
            let artifacts_len = callee_account_properties.artifacts_len;
            let code_version = callee_account_properties.code_version;

            match code_version {
                DEFAULT_CODE_VERSION_BYTE => {
                    assert_eq!(artifacts_len, 0);
                    let bytecode_preprocessing = BytecodePreprocessingData::create_artifacts(
                        system.get_allocator(),
                        bytecode,
                        &mut available_resources,
                    )?;
                    self.bytecode = bytecode;
                    self.bytecode_preprocessing = bytecode_preprocessing;
                }
                ARTIFACTS_CACHING_CODE_VERSION_BYTE => {
                    let (code, bytecode_preprocessing) = BytecodePreprocessingData::parse_bytecode(
                        bytecode,
                        unpadded_code_len as usize,
                        artifacts_len as usize,
                    )?;
                    self.bytecode = code;
                    self.bytecode_preprocessing = bytecode_preprocessing;
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
                // EIP-161: contracts should be initialized with nonce 1
                // Note: this has to be done before we actually deploy the bytecode,
                // as constructor execution should see the deployed_address as having
                // nonce = 1
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
                // This strange modifier doesn't preserve caller and value,
                // but we still need to substitute "this" to the caller
                this_address = caller;
            }
            CallModifier::EVMCallcodeStatic => {
                // This strange modifier doesn't preserve caller and value,
                // but we still need to substitute "this" to the caller
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
            *self.gas.resources_mut() == S::Resources::empty(),
            "for a fresh call resources of initial frame must be empty",
        );

        // We need to set address of self and caller, static state
        // and calldata

        *self.gas.resources_mut() = available_resources;
        self.address = this_address;
        self.caller = caller_address;
        self.is_static = is_static;
        self.is_constructor = is_constructor;
        self.calldata = calldata;
        self.heap = heap;
        self.call_value = nominal_token_value;

        self.execute_till_yield_point(system, tracer)
    }

    /// Note: panics if `pending_os_request` is None
    fn continue_after_preemption<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_request_result: CallResult<'res, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let preemption_reason = match mem::take(&mut self.pending_os_request) {
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
        if self.gas.native() != 0 {
            return Err(internal_error!("Invalid initial native resources").into());
        }

        self.gas.reclaim_resources(returned_resources);

        match call_request_result {
            CallResult::PreparationStepFailed => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Call failed, out of gas\n"));
                // we fail because it's caller's failure
                let exit_code = EvmError::OutOfGas.into();
                return self.create_immediate_return_state(system, exit_code, tracer);
            }
            CallResult::Failed { return_values } => {
                match preemption_reason {
                    PendingOsRequest::Call => {
                        // NOTE: EE is ALLOWED to spend resources from caller's frame before
                        // passing a desired part of them to the callee, If particular EE wants to
                        // follow some not-true resource policy, it can make adjustments here before
                        // continuing the execution
                        self.copy_returndata_to_heap(return_values.returndata);
                    }
                    PendingOsRequest::Create(_) => {
                        // NOTE: failed deployments may have non-empty returndata
                        assert!(self.returndata_location.is_empty());
                        assert!(return_values.return_scratch_space.is_none());

                        self.returndata = return_values.returndata;
                    }
                }

                self.stack.push_zero().expect("must have enough space");
            }
            CallResult::Successful { return_values } => {
                match preemption_reason {
                    PendingOsRequest::Call => {
                        self.copy_returndata_to_heap(return_values.returndata);
                        self.stack.push_one().expect("must have enough space");
                    }
                    PendingOsRequest::Create(deployed_at) => {
                        assert!(return_values.return_scratch_space.is_none());
                        // NOTE: successful deployments have empty returndata
                        assert!(return_values.returndata.is_empty());
                        self.returndata = return_values.returndata;
                        // we need to push address to stack
                        self.stack
                            .push(&b160_to_u256(deployed_at))
                            .expect("must have enough space");
                    }
                }
            }
        }

        self.execute_till_yield_point(system, tracer)
    }

    fn calculate_resources_passed_in_external_call(
        resources_available_in_caller_frame: &mut S::Resources,
        call_request: &ExternalCallRequest<S>,
        callee_parameters: &CalleeAccountProperties,
    ) -> Result<S::Resources, Self::SubsystemError> {
        let mut stipend = None;

        // Additional cost for non-zero value for general calls
        if call_request.modifier != CallModifier::Constructor {
            // Gas stipend calculation
            let is_delegate = call_request.is_delegate();
            let is_callcode = call_request.is_callcode();
            let is_callcode_or_delegate = is_callcode || is_delegate;

            // Positive value cost and stipend
            stipend = if !is_delegate && !call_request.nominal_token_value.is_zero() {
                let positive_value_cost = S::Resources::from_ergs(Ergs(CALLVALUE * ERGS_PER_GAS));
                resources_available_in_caller_frame.charge(&positive_value_cost)?;
                Some(Ergs(CALL_STIPEND * ERGS_PER_GAS))
            } else {
                None
            };

            // Account creation cost
            let callee_is_empty = callee_parameters.nonce == 0
                && callee_parameters.unpadded_code_len == 0
                && callee_parameters.nominal_token_balance.is_zero();
            if !is_callcode_or_delegate
                && !call_request.nominal_token_value.is_zero()
                && callee_is_empty
            {
                let callee_creation_cost = S::Resources::from_ergs(Ergs(NEWACCOUNT * ERGS_PER_GAS));
                resources_available_in_caller_frame.charge(&callee_creation_cost)?
            }
        }

        // we just need to apply 63/64 rule, as System/IO is responsible for the rest

        let max_passable_ergs =
            gas_utils::apply_63_64_rule(resources_available_in_caller_frame.ergs());
        let ergs_to_pass = core::cmp::min(call_request.ergs_to_pass, max_passable_ergs);

        // Charge caller frame
        let mut resources_to_pass = S::Resources::from_ergs(ergs_to_pass);

        // This never panics because max_passable_ergs <= resources_available_in_caller_frame
        resources_available_in_caller_frame
            .charge(&resources_to_pass)
            .unwrap();

        // Add stipend
        if let Some(stipend) = stipend {
            resources_to_pass.add_ergs(stipend);
        }

        Ok(resources_to_pass)
    }

    fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        system: &mut System<S>,
        frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<bool, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        if frame_state.environment_parameters.callstack_depth > 1024 {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Callstack is too deep\n",));

            tracer.evm_tracer().on_call_error(&EvmError::CallTooDeep);
            return Ok(false);
        }

        // Check caller has enough balance for token transfer
        if !frame_state.external_call.nominal_token_value.is_zero()
            && !frame_state.external_call.is_delegate()
        {
            let caller_balance = frame_state
                .external_call
                .available_resources
                .with_infinite_ergs(|inf_resources| {
                    system.io.read_account_properties(
                        THIS_EE_TYPE,
                        inf_resources,
                        &frame_state.external_call.caller,
                        AccountDataRequest::empty().with_nominal_token_balance(),
                    )
                })?
                .nominal_token_balance
                .0;

            if caller_balance < frame_state.external_call.nominal_token_value {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Not enough balance for transfer\n",));
                tracer
                    .evm_tracer()
                    .on_call_error(&EvmError::InsufficientBalance);
                return Ok(false);
            }
        }

        if frame_state.external_call.modifier == CallModifier::Constructor {
            // Increase nonce. Ignore, if we are in the root frame - caller's nonce already incremented before.
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

            let deployee_code_len = frame_state
                .environment_parameters
                .callee_account_properties
                .unpadded_code_len;
            let deployee_nonce = frame_state
                .environment_parameters
                .callee_account_properties
                .nonce;

            // Check there's no contract already deployed at this address.
            // NB: EVM also specifies that the address should have empty storage,
            // but we cannot perform such a check for now.
            // We need to check this here (not when we actually deploy the code)
            // because if this check fails the constructor shouldn't be executed.
            if deployee_code_len != 0 || deployee_nonce != 0 {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Deployment on existing account\n",));
                frame_state
                    .external_call
                    .available_resources
                    .charge(&S::Resources::from_ergs(
                        frame_state.external_call.available_resources.ergs(),
                    ))
                    .expect("Should succeed"); // Burn all gas

                tracer
                    .evm_tracer()
                    .on_call_error(&EvmError::CreateCollision);
                return Ok(false);
            }
        }

        Ok(true)
    }
}
