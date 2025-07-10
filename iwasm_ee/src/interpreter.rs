use alloc::vec::Vec;
use core::fmt::Write;
use errors::InternalError;
use errors::SystemError;
use iwasm_interpreter::routines::memory::ContinuousIndexAccessMut;
use iwasm_interpreter::routines::runtime::stack_value::StackValue;
use iwasm_interpreter::types::ValueTypeArray;
use iwasm_specification::sys::SliceRef;
use zk_ee::system::ExecutionEnvironmentType;
use zk_ee::utils::bump_nonce;
use zk_ee::utils::memzero;

use self::frame::*;
use self::host::ZkOSHost;

use super::*;
use crate::deployment_artifacts::*;
use crate::memory_manager::ZkOSIWasmMemoryManager;
use core::mem::MaybeUninit;
use iwasm_interpreter::parsers::runtime::main_parser::RuntimeParser;
use iwasm_interpreter::routines::runtime::instantiate::ExecutionResult;
use iwasm_interpreter::routines::runtime::module_instance::SourceRefs;
use zk_ee::system::kv_markers::*;
use zk_ee::system::queries::*;
use zk_ee::system::reference_implementations::BaseComputationalResources;

// TODO: move somewhere more appropriate.
pub const MAX_IMMUTABLES_SIZE: usize = 1 << 13;

impl<S: EthereumLikeSystem> IWasmInterpreter<S> {
    fn execute_till_yield_point(&mut self, system: &mut S) -> ExecutionEnvironmentExitState<S> {
        // assemble host
        let IWasmInterpreter {
            instantiated_module,
            resources,
            environment_params,
            context,
            iwasm_import_context,
            sidetable,
        } = self;

        let saved_pos = context.src_state;

        let mut host = ZkOSHost {
            context: &*context,
            import_context: iwasm_import_context,
            resources,
            system,
            returndata_region: MemoryRegion {
                region_type: MemoryRegionType::Shared,
                description: MemoryRegionDescription::empty(),
            },
        };

        // restore bytecode
        let bytecode_len = environment_params.bytecode_len;
        let parser = RuntimeParser::new(
            &environment_params.decommitted_bytecode.as_ref()[..bytecode_len as usize],
        );
        let mut source = SourceRefs::restore(parser, saved_pos);
        let result = instantiated_module.interpreter_loop(&mut source, &sidetable[..], &mut host);

        let _ = host
            .system
            .get_logger()
            .write_fmt(format_args!("IWasm result = {:?}\n", &result));

        let (returndata, reverted) = match result {
            Ok(result) => match result {
                ExecutionResult::Continue => {
                    panic!("Invalid final state");
                }
                r @ ExecutionResult::Return | r @ ExecutionResult::Reverted => {
                    let vref_addr = instantiated_module
                        .stack
                        .pop()
                        .expect("Expecting a return value on the stack.")
                        .as_i32();

                    assert!(
                        instantiated_module.stack.is_empty(),
                        "Only a single value is expected on the stack"
                    );

                    let vref = host
                        .system
                        // The returned value is a ptr...
                        .get_memory_region_range(
                            MemoryRegion
                            // ... to a SliceRef ...
                            ::shared_for_at::<SliceRef<u32>>(
                                vref_addr as usize
                            )
                            .expect("Interpreter must return a correctly aligned pointer."),
                        )
                        .as_ptr()
                        .cast::<SliceRef<u32>>();

                    let vref = unsafe { &*vref };

                    let returndata = MemoryRegion {
                        region_type: MemoryRegionType::Shared,
                        // ... that holds the pointer and the length of the actual return value.
                        description: MemoryRegionDescription {
                            offset: vref.ptr as usize,
                            len: vref.len as usize,
                        },
                    };

                    (returndata, r == ExecutionResult::Reverted)
                }
                ExecutionResult::DidNotComplete | ExecutionResult::Exception => {
                    (MemoryRegion::empty_shared(), true)
                }
                ExecutionResult::Preemption => {
                    todo!()
                }
            },
            Err(_) => (MemoryRegion::empty_shared(), true),
        };

        let exit_state = host.create_immediate_return_state(returndata, reverted);

        if self.context.is_constructor {
            if let ExecutionEnvironmentExitState::CompletedDeployment(CompletedDeployment {
                deployment_result:
                    DeploymentResult::Successful {
                        environment_params: returned_env_params,
                        ..
                    },
                ..
            } = environment_params;

            let imms = &system.get_memory_region_range(return_values.region());

            let decommitted_bytecode = match decommitted_bytecode {
                BytecodeSource::Owned(x) => x,
                _ => unreachable!(),
            };

            let new_art_len = unsafe {
                IWasmDeploymentArtifact::append_immutables(
                    bytecode_len.next_multiple_of(4) as usize,
                    imms,
                    decommitted_bytecode,
                )
            };

            let bytecode = AnySrc::Simple(decommitted_bytecode.as_ref());
            system
                .deploy_account_code_from_execution_environment::<Self>(
                    &deployed_at,
                    bytecode,
                    *bytecode_len,
                    new_art_len,
                    0u8,
                    0u8,
                )
                .expect("must deploy new contract to storage");
        }

        exit_state
    }
}

pub enum IWasmPreemptionRequest<S: System> {
    Call(IWasmCallRequest<S>),
    // Create(IWasmCreateRequest<S>),
}

// pub struct IWasmCreateRequest<S: System> {
//     pub(crate) passing_resources: S::Resources,
//     pub(crate) call_value: U256,
//     pub(crate) initcode: MemoryRegionDescription,
//     pub(crate) scheme: CreateScheme,
// }

#[allow(dead_code)]
pub struct IWasmCallRequest<S: System> {
    pub(crate) passing_resources: S::Resources,
    pub(crate) call_value: U256,
    pub(crate) destination_address: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub(crate) calldata: MemoryRegionDescription,
    pub(crate) modifier: CallModifier,
}

impl<S: EthereumLikeSystem> ExecutionEnvironment<S> for IWasmInterpreter<S> {
    const NEEDS_SCRATCH_SPACE: bool = false;

    const EE_VERSION_BYTE: u8 = ExecutionEnvironmentType::IWASM_EE_BYTE;

    fn is_modifier_supported(modifier: &CallModifier) -> bool {
        matches!(
            modifier,
            CallModifier::NoModifier
                | CallModifier::Constructor
                | CallModifier::Static
                | CallModifier::Delegate
        )
    }

    fn self_address(&self) -> &<<S as System>::IOTypes as SystemIOTypesConfig>::Address {
        &self.context.address
    }

    fn resources_mut(&mut self) -> &mut <S as System>::Resources {
        &mut self.resources
    }

    fn is_static_context(&self) -> bool {
        self.context.is_static
    }

    fn new<'system>(system: &'system mut S) -> Result<Self, InternalError> {
        let empty_resources = S::Resources::empty();
        let empty_sidetable = &[];
        let empty_env = EnvironmentParameters::empty();
        let context = Context::<S>::empty();
        let iwasm_import_context = IWasmImportContext::empty(system);

        let empty_module = create_placeholder_module(system);

        Ok(Self {
            instantiated_module: empty_module,
            resources: empty_resources,
            environment_params: empty_env,
            context,
            iwasm_import_context,
            sidetable: empty_sidetable,
        })
    }

    fn start_executing_frame(
        &mut self,
        system: &mut S,
        frame_state: ExecutionEnvironmentLaunchParams<S>,
    ) -> ExecutionEnvironmentExitState<S> {
        let ExecutionEnvironmentLaunchParams {
            resources_passed,
            call_parameters,
            environment_parameters,
            call_values,
        } = frame_state;
        assert!(call_values.call_scratch_space.is_none());

        let CallParameters {
            callers_caller,
            caller,
            callee,
            modifier,
        } = call_parameters;

        let expected_len: usize = crate::deployment_artifacts::iwasm_full_preimage_len(
            environment_parameters.bytecode_len,
            environment_parameters.scratch_space_len,
        );
        if environment_parameters.decommitted_bytecode.len() != expected_len {
            panic!("invalid bytecode supplied");
        }

        let CallValues {
            calldata,
            call_scratch_space: _,
            nominal_token_value,
        } = call_values;

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
            CallModifier::Constructor => is_constructor = true,
            _ => {
                panic!("modifier is not expected");
            }
        }

        assert!(
            self.resources == S::Resources::empty(),
            "for a fresh call resources of initial frame must be empty",
        );

        // now we should decode sidetable

        let artifacts_start = environment_parameters.bytecode_len.next_multiple_of(4);

        let bcref = environment_parameters.decommitted_bytecode.as_ref();
        #[allow(clippy::missing_transmute_annotations)]
        let bcref = unsafe { core::mem::transmute::<_, &'static [u8]>(bcref) };
        let aligned_view = unsafe {
            IWasmDeploymentArtifactAligned::from_slice(&bcref[artifacts_start as usize..])
        };
        // construct a state

        // fill the context
        let ctx = &mut self.context;
        ctx.caller = caller_address;
        ctx.address = this_address;
        ctx.call_value = nominal_token_value;
        ctx.is_static = is_static;
        ctx.is_constructor = is_constructor;
        ctx.calldata = calldata;
        ctx.bytecode_len = expected_len as u32;
        ctx.immutables = aligned_view.immutables;

        // now we can parse a module
        use iwasm_interpreter::routines::runtime::Interpreter;
        let source = &environment_parameters.decommitted_bytecode.as_ref()
            [..(environment_parameters.bytecode_len as usize)];
        let full_bytecode = RuntimeParser::new(source);
        let mut memory_manager = ZkOSIWasmMemoryManager::new(system);

        let mapping_fn = |idx: u16| unsafe {
            aligned_view
                .function_to_sidetable_compact_mapping
                .get_unchecked(idx as usize) as u32
        };

        let parsed_module = Interpreter::<_, _, ValueTypeArray, _, _>::new_from_validated_code(
            full_bytecode,
            &mapping_fn,
            memory_manager.clone(),
            |_x| {
                #[cfg(feature = "testing")]
                {
                    println!("{}", _x)
                }
            },
        )
        .expect("must prepare pre-validated module");

        let function_name = if self.context.is_constructor {
            "constructor"
        } else {
            "runtime"
        };
        let function_idx = parsed_module
            .find_function_idx_by_name(function_name)
            .expect("must get a function to run");

        let mut host = ZkOSHost {
            context: &self.context,
            import_context: &mut self.iwasm_import_context,
            // TODO: Verify how resources should be handled for this host instance.
            resources: &mut BaseResources {
                spendable: BaseComputationalResources { ergs: u64::MAX },
            },
            system,
            returndata_region: MemoryRegion {
                region_type: MemoryRegionType::Shared,
                description: MemoryRegionDescription::empty(),
            },
        };
        let mut instantiated_module = parsed_module
            .instantiate_module_owned(&mut host, &mut memory_manager)
            .expect("must instantiate pre-validated module");

        // now we should rewind the source, so we can run the interpreter
        let src_refs = instantiated_module
            .prepare_to_run_function_by_index(
                function_idx,
                &[], // there is no formal input
                full_bytecode,
                |args| {
                    let _ = host.system.get_logger().write_fmt(args);
                },
            )
            .expect("must prepare to run function by index");
        let src_pos = src_refs.save_pos();

        self.instantiated_module = instantiated_module;
        self.sidetable = aligned_view.raw_sidetable_entries;
        self.resources = resources_passed;
        self.environment_params = environment_parameters;
        self.context.src_state = src_pos;

        self.execute_till_yield_point(system)
    }

    fn continue_after_external_call(
        &mut self,
        system: &mut S,
        returned_resources: S::Resources,
        call_result: CallResult<S>,
    ) -> ExecutionEnvironmentExitState<S> {
        assert!(!call_result.has_scratch_space());
        self.resources
            .spendable_part_mut()
            .add(returned_resources.spendable_part());
        match call_result {
            CallResult::CallFailedToExecute => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Call failed, out of gas\n"));
                // we fail because it's caller's failure
                let host = ZkOSHost {
                    context: &self.context,
                    import_context: &mut self.iwasm_import_context,
                    resources: &mut self.resources,
                    system,
                    returndata_region: MemoryRegion {
                        region_type: MemoryRegionType::Shared,
                        description: MemoryRegionDescription::empty(),
                    },
                };
                return host.create_immediate_return_state(MemoryRegion::empty_shared(), true);
            }
            CallResult::Failed { return_values } => {
                assert!(return_values.return_scratch_space.is_none());
                // top two elements of the stack are the output place of our host fn
                let stack_len = self.instantiated_module.stack.len();
                let dst = unsafe {
                    self.instantiated_module
                        .stack
                        .get_slice_unchecked_mut((stack_len - 2)..stack_len)
                };
                dst[0] = StackValue::new_bool(false);
                dst[1] = StackValue::new_i64(return_values.returndata.len() as u64 as i64);
                self.context.last_returndata = return_values.returndata;
            }
            CallResult::Successful { return_values } => {
                assert!(return_values.return_scratch_space.is_none());
                // top two elements of the stack are the output place of our host fn
                let stack_len = self.instantiated_module.stack.len();
                let dst = unsafe {
                    self.instantiated_module
                        .stack
                        .get_slice_unchecked_mut((stack_len - 2)..stack_len)
                };
                dst[0] = StackValue::new_bool(true);
                dst[1] = StackValue::new_i64(return_values.returndata.len() as u64 as i64);
                self.context.last_returndata = return_values.returndata;
            }
        }

        self.execute_till_yield_point(system)
    }

    fn continue_after_deployment(
        &mut self,
        system: &mut S,
        returned_resources: S::Resources,
        deployment_result: DeploymentResult<S>,
    ) -> ExecutionEnvironmentExitState<S> {
        assert!(!deployment_result.has_scratch_space());
        self.resources
            .spendable_part_mut()
            .add(returned_resources.spendable_part());
        match deployment_result {
            DeploymentResult::Failed { return_values, .. } => {
                assert!(return_values.return_scratch_space.is_none());
                // top two elements of the stack are the output place of our host fn
                let stack_len = self.instantiated_module.stack.len();
                let dst = unsafe {
                    self.instantiated_module
                        .stack
                        .get_slice_unchecked_mut((stack_len - 2)..stack_len)
                };
                dst[0] = StackValue::new_bool(false);
                dst[1] = StackValue::new_i64(return_values.returndata.len() as u64 as i64);
                self.context.last_returndata = return_values.returndata;
                self.context.last_deployed_address =
                    <S::IOTypes as SystemIOTypesConfig>::Address::default();
            }
            DeploymentResult::Successful {
                return_values,
                deployed_at,
                ..
            } => {
                assert!(return_values.return_scratch_space.is_none());
                // top two elements of the stack are the output place of our host fn
                let stack_len = self.instantiated_module.stack.len();
                let dst = unsafe {
                    self.instantiated_module
                        .stack
                        .get_slice_unchecked_mut((stack_len - 2)..stack_len)
                };
                dst[0] = StackValue::new_bool(true);
                dst[1] = StackValue::new_i64(return_values.returndata.len() as u64 as i64);
                self.context.last_returndata = return_values.returndata;
                self.context.last_deployed_address = deployed_at;
            }
        }

        self.execute_till_yield_point(system)
    }

    type DeploymentExtraParameters = CreateScheme;

    fn charge_to_continue_call_execution(
        _call_preparation_mask: CallPreparationWorkBitmask,
        _resources_before_start_of_call: S::Resources, // What EE had right BEFORE the call
        resources_available: &mut S::Resources, // What EE has left right now, after SYSTEM did the preparations
        desired_resources_to_pass: &mut S::Resources, // What EE originally intended to pass
        _positive_value: bool,
        _callee_is_empty: bool,
        _modifier: CallModifier,
    ) -> Result<(), SystemError> {
        // we only need 63/64 rule and trust system to charge for IO

        // follow 63/64 rule
        let passable = core::cmp::min(
            resources_available.spendable_part().ergs
                - resources_available.spendable_part().ergs / 64,
            desired_resources_to_pass.spendable_part().ergs,
        );

        // now spend and res-assign
        desired_resources_to_pass.spendable_part_mut().ergs = passable;

        resources_available
            .spendable_part_mut()
            .try_spend_or_floor_self(desired_resources_to_pass.spendable_part())
    }

    fn start_deployment<'system>(
        system: &'system mut S,
        deployment_parameters: DeploymentParameters<S>,
        resources_available: &mut S::Resources,
    ) -> Result<Option<ExecutionEnvironmentLaunchParams<S>>, SystemError> {
        // We need to perform bytecode validation
        let DeploymentParameters {
            address_of_deployer,
            call_scratch_space,
            deployment_code,
            constructor_parameters,
            ee_specific_deployment_processing_data,
            nominal_token_value,
            nonce_already_updated,
        } = deployment_parameters;

        // // for EVM we just create a new frame and run it

        assert!(call_scratch_space.is_none());
        let Some(ee_specific_deployment_processing_data) = ee_specific_deployment_processing_data
        else {
            panic!("We need deployment scheme!");
        };
        let Ok(scheme) = <CreateScheme as EEDeploymentExtraParameters<S>>::from_box_dyn(
            ee_specific_deployment_processing_data,
        ) else {
            panic!("Unknown EE specific deployment data");
        };

        // Constructor gets 63/64 of available resources
        let mut resources_for_constructor = BaseResources {
            spendable: BaseComputationalResources {
                ergs: resources_available.spendable_part().ergs
                    - resources_available.spendable_part().ergs / 64,
            },
        };
        // We charge the caller all those forwarded resources
        resources_available
            .spendable_part_mut()
            .try_spend_or_floor_self(&resources_for_constructor.spendable_part())?;

        let initcode_len = deployment_code.len() as u64;
        // charge for initcode
        let initcode_cost = initcode_len;
        let ergs_to_spend = initcode_cost;
        // pay for initcode
        let deployment_cost = BaseComputationalResources {
            ergs: ergs_to_spend,
        };

        resources_for_constructor
            .spendable_part_mut()
            .try_spend_or_floor_self(&deployment_cost)?;

        use crypto::blake2s::Blake2s256;
        use crypto::MiniDigest;
        let mut formal_infinite_resources =
            zk_ee::system::reference_implementations::FORMAL_INFINITE_BASE_RESOURCES;
        let old_deployer_nonce = nonce_already_updated
            .ok_or(internal_error!("Deployer nonce should have been updated"))?;

        let (deployed_address, ergs_cost) = match &scheme {
            CreateScheme::Create => {
                let mut buffer = [0u8; 1 + 32 + 32];
                buffer[0] = 0xff;
                buffer[(1 + 12)..33]
                    .copy_from_slice(&address_of_deployer.to_be_bytes::<{ B160::BYTES }>());
                buffer[(33 + 24)..65].copy_from_slice(&old_deployer_nonce.to_be_bytes());
                let new_address = Blake2s256::digest(&buffer[..]);
                let new_address = B160::try_from_be_slice(&new_address.as_slice()[12..])
                    .expect("must create address");

                const CREATE_GAS: u64 = 32000;

                (new_address, CREATE_GAS) // TODO
            }
            CreateScheme::Create2 { salt } => {
                // we need to compute address based on the hash of the code and salt
                let deployment_code_slice = system.get_memory_region_range(deployment_code);
                let initcode_hash = Blake2s256::digest(&deployment_code_slice);
                let num_words = deployment_code_slice.len().next_multiple_of(64) / 64;
                let hashing_cost = num_words as u64; // TODO

                let mut create2_buffer = [0xffu8; 1 + 20 + 32 + 32];
                create2_buffer[1..(1 + 20)]
                    .copy_from_slice(&address_of_deployer.to_be_bytes::<{ B160::BYTES }>());
                create2_buffer[(1 + 20)..(1 + 20 + 32)]
                    .copy_from_slice(&salt.to_be_bytes::<{ U256::BYTES }>());
                create2_buffer[(1 + 20 + 32)..(1 + 20 + 32 + 32)]
                    .copy_from_slice(initcode_hash.as_slice());

                let new_address = Blake2s256::digest(&create2_buffer);
                let new_address = B160::try_from_be_slice(&new_address.as_slice()[12..])
                    .expect("must create address");

                (new_address, hashing_cost + 1000) // TODO
            }
        };

        // EIP-161: contracts should be initialized with nonce 1
        bump_nonce(system, &deployed_address, &mut formal_infinite_resources)
            .expect("nonce for deployed contract should be set to 1");

        let ergs_to_spend = ergs_cost;
        // pay for deployment
        let deployment_cost = BaseComputationalResources {
            ergs: ergs_to_spend,
        };

        resources_for_constructor
            .spendable_part_mut()
            .try_spend_or_floor_self(&deployment_cost)?;

        if nominal_token_value != U256::ZERO {
            let mut formal_infinite_resources =
                zk_ee::system::reference_implementations::FORMAL_INFINITE_BASE_RESOURCES;
            let mut old_value = MaybeUninit::uninit();
            let mut access_mask = AccountPropertyAccessBitmask::empty();
            let mut query = UpdateQuery::Balance(UpdateQueryRef {
                key: &address_of_deployer,
                old_value_dst: &mut old_value,
                access_mask: &mut access_mask,
                update_fn: BalanceUpdateFn {
                    diff: nominal_token_value,
                    is_sub: true,
                },
            });
            system
                .perform_update_query(&mut query, &mut formal_infinite_resources)
                .expect("must update balance of deployer");
            let mut query = UpdateQuery::Balance(UpdateQueryRef {
                key: &deployed_address,
                old_value_dst: &mut old_value,
                access_mask: &mut access_mask,
                update_fn: BalanceUpdateFn {
                    diff: nominal_token_value,
                    is_sub: false,
                },
            });
            system
                .perform_update_query(&mut query, &mut formal_infinite_resources)
                .expect("must update balance of newly deployed");
        }

        // and now we can just create a new special frame and let system run from there

        let bytecode_len = deployment_code.len();

        let bytecode_to_check = system.get_memory_region_range(deployment_code);
        let source =
            iwasm_interpreter::parsers::verification_time::main_parser::VerificationTimeParser::new(
                bytecode_to_check,
            );
        let mut memory_manager = ZkOSIWasmMemoryManager::new(system);
        let (raw_sidetable_entries, function_to_sidetable_mapping) =
            iwasm_interpreter::routines::verification_time::Validator::parse(
                source,
                &mut memory_manager,
                |_x| {
                    #[cfg(feature = "testing")]
                    {
                        std::println!("{}", _x)
                    };
                },
            )
            .map_err(|_| internal_error!("iwasm: validator parsing failed"))?;
        for el in function_to_sidetable_mapping[..].iter() {
            if *el > u8::MAX as u32 {
                return Err(internal_error!("iwasm: element out of range in mapping").into());
            }
        }
        let deployment_artifacts = IWasmDeploymentArtifact {
            function_to_sidetable_mapping,
            raw_sidetable_entries,
            // Immutables are not yet set.
            immutables: Vec::new_in(system.get_allocator()),
        };

        let padded_bytecode_len = bytecode_len.next_multiple_of(4);
        let deployment_artifacts_len = deployment_artifacts.serialization_len();
        let total_len = padded_bytecode_len + deployment_artifacts_len + MAX_IMMUTABLES_SIZE;
        let mut serialized_bytecode = Vec::with_capacity_in(total_len, system.get_allocator());
        serialized_bytecode.extend_from_slice(system.get_memory_region_range(deployment_code));
        // unsafe, but resize doesn't always produce sane bytecode
        unsafe {
            let start = serialized_bytecode.as_mut_ptr_range().end;
            serialized_bytecode.set_len(padded_bytecode_len);
            let end = serialized_bytecode.as_mut_ptr_range().end;
            memzero(start, end);
        }
        deployment_artifacts.serialize_extend(&mut serialized_bytecode);

        let call_parameters = CallParameters::<S> {
            callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address::default(), // Fine to use placeholder
            caller: address_of_deployer,
            callee: deployed_address,
            modifier: CallModifier::Constructor,
        };

        let environment_parameters = EnvironmentParameters {
            decommitted_bytecode: BytecodeSource::Owned(serialized_bytecode),
            bytecode_len: bytecode_len as u32,
            scratch_space_len: deployment_artifacts_len as u32,
        };

        // remap calldata
        let mut calldata = constructor_parameters;
        system.remap_region_to_absolute(&mut calldata);

        let call_values = CallValues {
            calldata,
            call_scratch_space: None,
            nominal_token_value,
        };

        let next_frame_state = ExecutionEnvironmentLaunchParams {
            resources_passed: resources_for_constructor,
            call_parameters,
            environment_parameters,
            call_values,
        };

        Ok(Some(next_frame_state))
    }
}

/// Create scheme.
#[repr(usize)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum CreateScheme {
    /// Legacy create scheme of `CREATE`.
    Create = 1,
    /// Create scheme of `CREATE2`.
    Create2 {
        /// Salt.
        salt: U256,
    },
}

impl<S: System> EEDeploymentExtraParameters<S> for CreateScheme {}
