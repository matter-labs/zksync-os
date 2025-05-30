#![no_main]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use arbitrary::{Arbitrary, Unstructured};
use basic_bootloader::bootloader::supported_ees::SupportedEEVMState;
use evm_interpreter::Interpreter;
use libfuzzer_sys::fuzz_target;
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use rig::forward_system::system::system::ForwardRunningSystem;
use rig::ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::reference_implementations::FORMAL_INFINITE_BASE_RESOURCES;
use zk_ee::system::CallModifier;
use zk_ee::system::ExecutionEnvironmentLaunchParams;
use zk_ee::system::NopResultKeeper;
use zk_ee::system::{
    CallResult, DeploymentPreparationParameters, DeploymentResult, EnvironmentParameters,
    ExecutionEnvironment, ExternalCallRequest, MemorySubsystemExt, ReturnValues, System,
};
use zk_ee::utils::Bytes32;

extern crate alloc;

mod common;
use common::mock_oracle;

#[derive(Arbitrary, Debug)]
struct FuzzInput<'a> {
    // To run specific fuzz sub-test: #[arbitrary(value = 2)]
    // To exclude specific fuzz sub-tests: #[arbitrary(with = |u: &mut Unstructured| Ok(*u.choose(&[1,3]).unwrap()))]
    // To run all: #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=3))]
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=3))]
    selector: u8,

    #[arbitrary(value = 1)] // Only allow EVM
    ee_version: u8,

    raw_calldata: &'a [u8],

    raw_bytecode: &'a [u8],

    address1: [u8; 20],
    address2: [u8; 20],
    address3: [u8; 20],

    amount: [u8; 32],

    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=8))]
    modifier: u8,

    bool_1: bool,

    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=2))]
    call_deployment_result: u8,
}

fn fuzz(input: FuzzInput) {
    let selector = input.selector;

    let mut system = System::<
        ForwardRunningSystem<InMemoryTree, InMemoryPreimageSource, TxListSource>,
    >::init_from_oracle(mock_oracle())
    .expect("Failed to initialize the mock system");

    let Ok(mut vm_state) = SupportedEEVMState::create_initial(input.ee_version, &mut system) else {
        return;
    };

    // choose a CallModifier
    let modifier = match input.modifier {
        0 => CallModifier::NoModifier,
        1 => CallModifier::Constructor,
        2 => CallModifier::Delegate,
        3 => CallModifier::Static,
        4 => CallModifier::DelegateStatic,
        5 => CallModifier::EVMCallcodeStatic,
        6 => CallModifier::EVMCallcode,
        7 => CallModifier::ZKVMSystem,
        _ => CallModifier::ZKVMSystemStatic,
    };

    // modifier should be supported by EE
    let is_supported_modifier = matches!(
        modifier,
        CallModifier::NoModifier
            | CallModifier::Constructor
            | CallModifier::Static
            | CallModifier::Delegate
            | CallModifier::DelegateStatic
            | CallModifier::EVMCallcode
    );

    if !is_supported_modifier {
        return;
    }

    // wrap calldata
    let calldata = unsafe {
        system
            .memory
            .construct_immutable_slice_from_static_slice(core::mem::transmute::<&[u8], &[u8]>(
                input.raw_calldata,
            ))
    };

    let mut bytecode = input.raw_bytecode.to_vec();
    if bytecode.len() > 0 && bytecode[0] == 91 {
        bytecode[0] = 95 as u8; // swap jumpdest to push0
    }

    // wrap bytecode
    let decommitted_bytecode = unsafe {
        system
            .memory
            .construct_immutable_slice_from_static_slice(core::mem::transmute::<&[u8], &[u8]>(
                &bytecode,
            ))
    };

    let bytecode_len = decommitted_bytecode.len() as u32;

    let empty = unsafe {
        system
            .memory
            .construct_immutable_slice_from_static_slice(core::mem::transmute::<&[u8], &[u8]>(&[]))
    };

    let Ok(_) = system.start_global_frame() else {
        return;
    };

    match selector {
        0 => {
            // Fuzz-test SupportedEEVMState::start_executing_frame
            let callers_caller = match modifier {
                CallModifier::Constructor => B160::default(),
                _ => B160::from_be_bytes(input.address1),
            };
            let caller = B160::from_be_bytes(input.address2);
            let callee = B160::from_be_bytes(input.address3);
            let nominal_token_value = U256::from_be_bytes(input.amount);

            // Pack everything into ExecutionEnvironmentLaunchParams
            let ee_launch_params: ExecutionEnvironmentLaunchParams<
                ForwardRunningSystem<InMemoryTree, InMemoryPreimageSource, TxListSource>,
            > = ExecutionEnvironmentLaunchParams {
                environment_parameters: EnvironmentParameters {
                    decommitted_bytecode,
                    bytecode_len,
                    scratch_space_len: 0,
                },
                external_call: ExternalCallRequest {
                    resources_to_pass: FORMAL_INFINITE_BASE_RESOURCES,
                    callers_caller,
                    caller,
                    callee,
                    modifier,
                    calldata,
                    call_scratch_space: None,
                    nominal_token_value,
                },
            };

            let _ = vm_state.start_executing_frame(&mut system, ee_launch_params);
        }
        1 => {
            // Fuzz-test SupportedEEVMState::continue_after_external_call
            let return_values = ReturnValues::from_immutable_slice(calldata);

            let call_result = match input.call_deployment_result {
                0 => CallResult::CallFailedToExecute,
                1 => CallResult::Failed { return_values },
                _ => CallResult::Successful { return_values },
            };

            // set bytecode
            #[allow(clippy::single_match)]
            match input.ee_version {
                0 => {
                    let SupportedEEVMState::EVM(evm_frame) = &mut vm_state;
                    evm_frame.bytecode = decommitted_bytecode;
                }
                _ => (),
            }

            let _ = vm_state.continue_after_external_call(
                &mut system,
                FORMAL_INFINITE_BASE_RESOURCES,
                call_result,
            );
        }
        2 => {
            // Fuzz-test SupportedEEVMState::continue_after_deployment
            let deployed_at = B160::from_be_bytes(input.address1);
            let execution_reverted = input.bool_1;

            let return_values = ReturnValues::from_immutable_slice(calldata);

            let return_values_successful = ReturnValues::from_immutable_slice(empty);

            let deployment_result = match input.call_deployment_result {
                0 => DeploymentResult::Failed {
                    return_values,
                    execution_reverted,
                },
                _ => DeploymentResult::Successful {
                    bytecode: decommitted_bytecode,
                    bytecode_len,
                    artifacts_len: 0,
                    return_values: return_values_successful,
                    deployed_at,
                },
            };

            let _ = vm_state.continue_after_deployment(
                &mut system,
                FORMAL_INFINITE_BASE_RESOURCES,
                deployment_result,
            );
        }
        3 => {
            // Fuzz-test SupportedEEVMState::prepare_for_deployment
            let address_of_deployer = B160::from_be_bytes(input.address1);
            let nominal_token_value = U256::from_be_bytes(input.amount);

            #[allow(clippy::match_single_binding)]
            let ee_specific_deployment_processing_data = match input.ee_version {
                _ => Interpreter::default_ee_deployment_options(&mut system),
            };

            #[allow(clippy::match_single_binding)]
            let ee_type = match input.ee_version {
                _ => ExecutionEnvironmentType::EVM,
            };

            let _ = SupportedEEVMState::prepare_for_deployment(
                ee_type,
                &mut system,
                DeploymentPreparationParameters {
                    address_of_deployer,
                    call_scratch_space: None,
                    constructor_parameters: empty,
                    deployment_code: calldata,
                    ee_specific_deployment_processing_data,
                    deployer_full_resources: FORMAL_INFINITE_BASE_RESOURCES,
                    deployer_nonce: None,
                    nominal_token_value,
                },
            );
        }
        _ => (),
    }

    let Ok(_) = system.finish_global_frame(None) else {
        return;
    };

    system.finish(Bytes32::default(), Bytes32::default(), &mut NopResultKeeper);
}

fuzz_target!(|input: FuzzInput| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(input);
});
