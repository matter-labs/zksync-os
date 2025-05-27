#![no_main]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use basic_bootloader::bootloader::constants::{MAX_CALLSTACK_DEPTH, TX_OFFSET};
use basic_bootloader::bootloader::transaction::ZkSyncTransaction;
use basic_bootloader::bootloader::StackFrame;
use basic_bootloader::bootloader::BasicBootloader;
use basic_bootloader::bootloader::config::BasicBootloaderForwardSimulationConfig;
use common::{mock_oracle_balance, mutate_transaction};
use libfuzzer_sys::{fuzz_mutator, fuzz_target};
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use rig::forward_system::system::system::ForwardRunningSystem;
use rig::ruint::aliases::U256;
use system_hooks::HooksStorage;
use zk_ee::system::{System, SystemFrameSnapshot};

mod common;

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    mutate_transaction(data, size, max_size, seed)
});

fn fuzz(data: &[u8]) {
    let mut data = data.to_vec();
    if data.len() < TX_OFFSET + 1 {
        data.resize(TX_OFFSET + 1, 0);
    }

    let Ok(decoded_tx) = ZkSyncTransaction::try_from_slice(&mut data) else {
        return;
    };
    let amount = U256::from_be_bytes([255 as u8; 32]);
    let address = decoded_tx.from.read();
    let oracle = mock_oracle_balance(address, amount);

    let mut system =
        System::init_from_oracle(oracle).expect("Failed to initialize the mock system");

    let mut system_functions: HooksStorage<
        ForwardRunningSystem<InMemoryTree, InMemoryPreimageSource, TxListSource>,
        _,
    > = HooksStorage::new_in(system.get_allocator());
    let mut callstack: Vec<
        StackFrame<
            ForwardRunningSystem<InMemoryTree, InMemoryPreimageSource, TxListSource>,
            SystemFrameSnapshot<_>,
        >,
    > = Vec::with_capacity_in(MAX_CALLSTACK_DEPTH, system.get_allocator());

    system_functions.add_precompiles();

    let data_mut_ref: &'static mut [u8] = unsafe { core::mem::transmute(data.as_mut_slice()) };

    let _ = BasicBootloader::process_transaction::<_, BasicBootloaderForwardSimulationConfig>(
        data_mut_ref,
        &mut system,
        &mut system_functions,
        &mut callstack,
    );
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
