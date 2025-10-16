#![no_main]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]

use basic_bootloader::bootloader::transaction::AbiEncodedTransaction;
use rig::forward_system::system::system::ForwardRunningSystem;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::reference_implementations::DecreasingNative;
use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
use zk_ee::system::Resource;
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
    let Ok(tx) = AbiEncodedTransaction::try_from_slice(&mut data) else {
        // Input is not valid
        return;
    };
    let mut inf_resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;
    let (metadata, oracle) = mock_oracle();
    let chain_id = BlockMetadataFromOracle::new_for_test().chain_id;
    let _ = tx.calculate_signed_hash(chain_id, &mut inf_resources);
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
