use super::*;
use crate::errors::{EvmErrors, EvmInterfaceError, EvmSubsystemError};
use crate::gas::gas_utils;
use crate::interpreter::CreateScheme;
use alloc::boxed::Box;
use core::any::Any;
use core::fmt::Write;
use ruint::aliases::B160;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::root_cause::{GetRootCause, RootCause};
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::*;
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::cheap_clone::CheapCloneRiscV;
use zk_ee::utils::{b160_to_u256, Bytes32};
use zk_ee::{interface_error, internal_error, wrap_error};

impl<S: SystemTypes> EEDeploymentExtraParameters<S> for CreateScheme {}

impl<'ee, S: EthereumLikeTypes> ExecutionEnvironment<'ee, S, EvmErrors> for Interpreter<'ee, S> {
    const NEEDS_SCRATCH_SPACE: bool = false;

    const EE_VERSION_BYTE: u8 = ExecutionEnvironmentType::EVM_EE_BYTE;

    type UsageError = <EvmErrors as zk_ee::system::errors::subsystem::Subsystem>::Interface;
    type SubsystemError = EvmSubsystemError;

    fn is_modifier_supported(modifier: &CallModifier) -> bool {
        matches!(
            modifier,
            CallModifier::NoModifier
                | CallModifier::Constructor
                | CallModifier::Static
                | CallModifier::Delegate
                | CallModifier::DelegateStatic
                | CallModifier::EVMCallcode
        )
    }

    fn self_address(&self) -> &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address {
        &self.address
    }

    /// TODO unused
    fn resources_mut(&mut self) -> &mut <S as SystemTypes>::Resources {
        self.gas.resources_mut()
    }

    fn is_static_context(&self) -> bool {
        self.is_static
    }

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
        })
    }

    fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        frame_state: ExecutionEnvironmentLaunchParams<'i, S>,
        heap: SliceVec<'h, u8>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError> {
        let ExecutionEnvironmentLaunchParams {
            external_call:
                ExternalCallRequest {
                    ergs_to_pass: _,
                    mut available_resources,
                    caller,
                    callee,
                    callers_caller,
                    modifier,
                    calldata,
                    call_scratch_space,
                    nominal_token_value,
                },
            environment_parameters,
        } = frame_state;
        assert!(call_scratch_space.is_none());

        let EnvironmentParameters {
            bytecode,
            scratch_space_len: _,
        } = environment_parameters;

        let mut is_static = false;
        let mut is_constructor = false;

        let mut caller_address = caller;
        let mut this_address = callee;
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
                // check conventions
                debug_assert_eq!(
                    callers_caller,
                    <S::IOTypes as SystemIOTypesConfig>::Address::default()
                );

                is_constructor = true
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

        // we need to set bytecode, address of self and caller, static state
        // and calldata

        match bytecode {
            Bytecode::Constructor(constructor_code) => {
                let bytecode_preprocessing = BytecodePreprocessingData::create_artifacts(
                    system.get_allocator(),
                    constructor_code,
                    &mut available_resources,
                )?;
                self.bytecode = constructor_code;
                self.bytecode_preprocessing = bytecode_preprocessing;
            }
            Bytecode::Decommitted {
                bytecode,
                artifacts_len,
                unpadded_code_len,
                code_version,
            } => match code_version {
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
            },
        }

        *self.gas.resources_mut() = available_resources;
        self.address = this_address;
        self.caller = caller_address;
        self.is_static = is_static;
        self.is_constructor = is_constructor;
        self.calldata = calldata;
        self.heap = heap;
        self.call_value = nominal_token_value;

        self.execute_till_yield_point(system)
    }

    fn continue_after_external_call<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_result: CallResult<'res, S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError> {
        assert!(!call_result.has_scratch_space());
        assert!(self.gas.native() == 0);
        self.gas.reclaim_resources(returned_resources);
        match call_result {
            CallResult::CallFailedToExecute => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Call failed, out of gas\n"));
                // we fail because it's caller's failure
                return self.create_immediate_return_state(true, true, false);
            }
            CallResult::Failed { return_values } => {
                // NOTE: EE is ALLOWED to spend resources from caller's frame before
                // passing a desired part of them to the callee, If particular EE wants to
                // follow some not-true resource policy, it can make adjustments here before
                // continuing the execution
                self.copy_returndata_to_heap(return_values.returndata);
                self.stack.push_zero().expect("must have enough space");
            }
            CallResult::Successful { return_values } => {
                self.copy_returndata_to_heap(return_values.returndata);
                self.stack.push_one().expect("must have enough space");
            }
        }

        self.execute_till_yield_point(system)
    }

    fn continue_after_deployment<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        deployment_result: DeploymentResult<'res, S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError> {
        assert!(!deployment_result.has_scratch_space());
        assert!(self.gas.native() == 0);
        self.gas.reclaim_resources(returned_resources);
        match deployment_result {
            DeploymentResult::Failed {
                return_values,
                execution_reverted,
            } => {
                // NOTE: failed deployments may have non-empty returndata
                if execution_reverted {
                    assert!(self.returndata_location.is_empty());
                    assert!(return_values.return_scratch_space.is_none());
                }
                self.returndata = return_values.returndata;
                // we need to push 0 to stack
                self.stack.push_zero().expect("must have enough space");
            }
            DeploymentResult::Successful {
                return_values,
                deployed_at,
                ..
            } => {
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

        self.execute_till_yield_point(system)
    }

    type DeploymentExtraParameters = CreateScheme;

    fn default_ee_deployment_options(system: &mut System<S>) -> Option<Box<dyn Any, S::Allocator>> {
        let allocator = system.get_allocator();
        let scheme = Box::new_in(CreateScheme::Create, allocator);
        let scheme = scheme as Box<dyn Any, S::Allocator>;
        Some(scheme)
    }

    fn clarify_and_take_passed_resources(
        resources_available_in_caller_frame: &mut S::Resources,
        desired_ergs_to_pass: Ergs,
    ) -> Result<S::Resources, Self::SubsystemError> {
        // we just need to apply 63/64 rule, as System/IO is responsible for the rest

        let max_passable_ergs =
            gas_utils::apply_63_64_rule(resources_available_in_caller_frame.ergs());
        let ergs_to_pass = core::cmp::min(desired_ergs_to_pass, max_passable_ergs);

        // Charge caller frame
        let mut resources_to_pass = S::Resources::from_ergs(ergs_to_pass);

        // This never panics because max_passable_ergs <= resources_available_in_caller_frame
        resources_available_in_caller_frame
            .charge(&resources_to_pass)
            .unwrap();
        // Give native resource to the passed.
        resources_available_in_caller_frame.give_native_to(&mut resources_to_pass);

        Ok(resources_to_pass)
    }

    // derive address and check other preconditions to deploy the bytecode
    fn prepare_for_deployment<'a>(
        system: &mut System<S>,
        deployment_parameters: DeploymentPreparationParameters<'a, S>,
    ) -> Result<
        (
            S::Resources,
            Option<ExecutionEnvironmentLaunchParams<'a, S>>,
        ),
        Self::SubsystemError,
    >
    where
        S::IO: IOSubsystemExt,
    {
        // for EVM we just create a new frame and run it
        let DeploymentPreparationParameters {
            address_of_deployer,
            call_scratch_space,
            deployment_code,
            constructor_parameters,
            ee_specific_deployment_processing_data,
            nominal_token_value,
            mut deployer_full_resources,
            deployer_nonce,
        } = deployment_parameters;
        assert!(constructor_parameters.is_empty());
        assert!(call_scratch_space.is_none());
        let Some(ee_specific_deployment_processing_data) = ee_specific_deployment_processing_data
        else {
            return Err(interface_error!(EvmInterfaceError::NoDeploymentScheme));
        };
        let Ok(scheme) = <CreateScheme as EEDeploymentExtraParameters<S>>::from_box_dyn(
            ee_specific_deployment_processing_data,
        ) else {
            return Err(interface_error!(EvmInterfaceError::UnknownDeploymentData));
        };

        // Constructor gets 63/64 of available resources
        let ergs_for_constructor = gas_utils::apply_63_64_rule(deployer_full_resources.ergs());

        // We only charge after succeeding the following checks:
        // - Deployer has enough balance for token transfer
        // - Nonce overflow check

        // Native resource is still in deployer_full_resources, so we charge it from there.

        let allocator = system.get_allocator().clone();

        let deployer_balance = deployer_full_resources
            .with_infinite_ergs(|inf_resources| {
                system.io.read_account_properties(
                    THIS_EE_TYPE,
                    inf_resources,
                    &address_of_deployer,
                    AccountDataRequest::empty().with_nominal_token_balance(),
                )
            })?
            .nominal_token_balance
            .0;

        // Check deployer has enough balance for token transfer
        if !nominal_token_value.is_zero() && deployer_balance < nominal_token_value {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Not enough balance for deployment\n",));
            return Ok((deployer_full_resources, None));
        }

        // Nonce overflow check
        let old_deployer_nonce = match deployer_nonce {
            Some(old_nonce) => Ok::<u64, Self::SubsystemError>(old_nonce),
            None => {
                match deployer_full_resources.with_infinite_ergs(|inf_resources| {
                    system.io.increment_nonce(
                        THIS_EE_TYPE,
                        inf_resources,
                        &address_of_deployer,
                        1u64,
                    )
                }) {
                    Ok(nonce) => Ok(nonce),
                    Err(SubsystemError::LeafUsage(InterfaceError(
                        NonceError::NonceOverflow,
                        _,
                    ))) => return Ok((deployer_full_resources, None)),
                    Err(e) => return Err(wrap_error!(e)),
                }
            }
        }?;

        use crypto::sha3::{Digest, Keccak256};
        let deployed_address = match &scheme {
            CreateScheme::Create => {
                let mut buffer = [0u8; crate::utils::MAX_CREATE_RLP_ENCODING_LEN];
                let encoding_it =
                    crate::utils::create_quasi_rlp(&address_of_deployer, old_deployer_nonce);
                let encoding_len = ExactSizeIterator::len(&encoding_it);
                for (dst, src) in buffer.iter_mut().zip(encoding_it) {
                    *dst = src;
                }
                let new_address = Keccak256::digest(&buffer[..encoding_len]);
                let new_address = B160::try_from_be_slice(&new_address.as_slice()[12..])
                    .expect("must create address");
                new_address
            }
            CreateScheme::Create2 { salt } => {
                // we need to compute address based on the hash of the code and salt
                let mut initcode_hash = ArrayBuilder::default();
                deployer_full_resources
                    .with_infinite_ergs(|inf_resources| {
                        S::SystemFunctions::keccak256(
                            &deployment_code,
                            &mut initcode_hash,
                            inf_resources,
                            allocator,
                        )
                    })
                    .map_err(|e| -> EvmSubsystemError {
                        match e.root_cause() {
                            RootCause::Runtime(e @ RuntimeError::OutOfNativeResources(_)) => {
                                e.clone_or_copy().into()
                            }
                            _ => internal_error!("Keccak in create2 cannot fail").into(),
                        }
                    })?;
                let initcode_hash = Bytes32::from_array(initcode_hash.build());

                let mut create2_buffer = [0xffu8; 1 + 20 + 32 + 32];
                create2_buffer[1..(1 + 20)]
                    .copy_from_slice(&address_of_deployer.to_be_bytes::<{ B160::BYTES }>());
                create2_buffer[(1 + 20)..(1 + 20 + 32)]
                    .copy_from_slice(&salt.to_be_bytes::<{ U256::BYTES }>());
                create2_buffer[(1 + 20 + 32)..(1 + 20 + 32 + 32)]
                    .copy_from_slice(initcode_hash.as_u8_array_ref());

                let new_address = Keccak256::digest(&create2_buffer);
                let new_address = B160::try_from_be_slice(&new_address.as_slice()[12..])
                    .expect("must create address");
                new_address
            }
        };

        // For now, keep native in deployer resources.
        let mut deployer_remaining_resources = deployer_full_resources;

        let mut resources_for_constructor = S::Resources::from_ergs(ergs_for_constructor);
        // Charge ergs for constructor (take 63/64, cannot fail).
        deployer_remaining_resources.charge_unchecked(&resources_for_constructor);

        let AccountData {
            nonce: Just(deployee_nonce),
            unpadded_code_len: Just(deployee_code_len),
            ..
        } = deployer_remaining_resources.with_infinite_ergs(|inf_resources| {
            system.io.read_account_properties(
                THIS_EE_TYPE,
                inf_resources,
                &deployed_address,
                AccountDataRequest::empty()
                    .with_nonce()
                    .with_unpadded_code_len(),
            )
        })?;

        // Check there's no contract already deployed at this address.
        // NB: EVM also specifies that the address should have empty storage,
        // but we cannot perform such a check for now.
        // We need to check this here (not when we actually deploy the code)
        // because if this check fails the constructor shouldn't be executed.
        if deployee_code_len != 0 || deployee_nonce != 0 {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Deployment on existing account\n",));
            return Ok((deployer_remaining_resources, None));
        }

        // Now we know the constructor will be ran, so we can take the native
        // resources from deployer.
        deployer_remaining_resources.give_native_to(&mut resources_for_constructor);

        let environment_parameters = EnvironmentParameters {
            bytecode: Bytecode::Constructor(deployment_code),
            scratch_space_len: 0u32,
        };

        // TODO: eventually more resources OUT of the frame
        let next_frame_state = ExecutionEnvironmentLaunchParams {
            external_call: ExternalCallRequest {
                available_resources: resources_for_constructor,
                // Ergs to pass are only used for actual calls
                ergs_to_pass: Ergs(0),
                caller: address_of_deployer,
                callee: deployed_address,
                callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address::default(), // Fine to use placeholder
                modifier: CallModifier::Constructor,
                calldata: &[],
                call_scratch_space: None,
                nominal_token_value,
            },
            environment_parameters,
        };

        Ok((deployer_remaining_resources, Some(next_frame_state)))
    }
}
