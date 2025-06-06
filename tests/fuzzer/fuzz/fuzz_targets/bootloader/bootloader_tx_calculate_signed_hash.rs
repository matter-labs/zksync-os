#![no_main]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]

use basic_bootloader::bootloader::transaction::ZkSyncTransaction;
use rig::forward_system::system::system::ForwardRunningSystem;
use zk_ee::system::System;

use common::mock_oracle;
use common::mutate_transaction;
use libfuzzer_sys::{fuzz_mutator, fuzz_target};
mod common;

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    mutate_transaction(data, size, max_size, seed)
});

fn fuzz(data: &[u8]) {
    let mut data = data.to_owned();
    let Ok(tx) = ZkSyncTransaction::try_from_slice(&mut data) else {
        if data.len() != 0 {
            panic!("input is not valid {:?}", data);
        }
        return;
    };

    let system = System::<ForwardRunningSystem<_, _, _>>::init_from_oracle(mock_oracle())
        .expect("Failed to initialize the mock system");
    let chain_id = system.get_chain_id();
    let _ = tx.calculate_signed_hash(chain_id);
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
