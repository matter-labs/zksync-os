#![no_main]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use arbitrary::Arbitrary;
use basic_bootloader::bootloader::supported_ees::SupportedEEVMState;
use libfuzzer_sys::fuzz_target;
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use rig::forward_system::system::system::ForwardRunningSystem;
use rig::ruint::aliases::{B160, U256};
use zk_ee::reference_implementations::FORMAL_INFINITE_BASE_RESOURCES;
use zk_ee::system::CallModifier;
use zk_ee::system::ExecutionEnvironmentLaunchParams;
use zk_ee::system::NopResultKeeper;
use zk_ee::system::{EnvironmentParameters, ExternalCallRequest, MemorySubsystemExt, System};
use zk_ee::utils::Bytes32;

extern crate alloc;

mod common;
use common::mock_oracle;

#[derive(Arbitrary, Debug)]
struct FuzzInput<'a> {
    #[arbitrary(value = 1)] // Only allow EVM
    ee_version: u8,

    raw_calldata: &'a [u8],

    args: [u8; 160],

    opcode: u8,

    address1: [u8; 20],
    address2: [u8; 20],
    address3: [u8; 20],

    amount: [u8; 32],

    bool_1: bool,
}

fn fuzz(input: FuzzInput) {
    let mut system = System::<ForwardRunningSystem<_, _, _>>::init_from_oracle(mock_oracle())
        .expect("Failed to initialize the mock system");

    let Ok(mut vm_state) = SupportedEEVMState::create_initial(input.ee_version, &mut system) else {
        return;
    };

    // wrap calldata
    let calldata = unsafe {
        system
            .memory
            .construct_immutable_slice_from_static_slice(core::mem::transmute::<&[u8], &[u8]>(
                input.raw_calldata,
            ))
    };

    let mut bytecode = Vec::<u8>::new();
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&input.args[..32]);
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&input.args[32..64]);
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&input.args[64..96]);
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&input.args[96..128]);
    bytecode.push(0x7f); // PUSH32
    bytecode.extend_from_slice(&input.args[128..160]);
    bytecode.push(input.opcode); // Random opcode

    // wrap bytecode
    let decommitted_bytecode = unsafe {
        system
            .memory
            .construct_immutable_slice_from_static_slice(core::mem::transmute::<&[u8], &[u8]>(
                bytecode.as_ref(),
            ))
    };

    let bytecode_len = decommitted_bytecode.len() as u32;

    let Ok(_) = system.start_global_frame() else {
        return;
    };

    let callers_caller = B160::from_be_bytes(input.address1);
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
            modifier: CallModifier::NoModifier,
            calldata,
            call_scratch_space: None,
            nominal_token_value,
        },
    };

    let _ = vm_state.start_executing_frame(&mut system, ee_launch_params);

    let Ok(_) = system.finish_global_frame(None) else {
        return;
    };

    system.finish(Bytes32::default(), Bytes32::default(), &mut NopResultKeeper);
}

fuzz_target!(|input: FuzzInput| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(input);
});
