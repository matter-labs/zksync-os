#![no_main]
#![feature(allocator_api)]

use basic_bootloader::bootloader::transaction::AbiEncodedTransaction;
use common::{mutate_transaction, parse_abi_encoded_transaction, serialize_zksync_transaction};
use libfuzzer_sys::{fuzz_mutator, fuzz_target};
use std::alloc::Global;
use zk_ee::{
    reference_implementations::{BaseResources, DecreasingNative},
    system::Resource,
};

mod common;

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    mutate_transaction(data, size, max_size, seed)
});

fn parse_full_tx(data: &[u8]) -> Result<AbiEncodedTransaction<Global>, ()> {
    let tx = parse_abi_encoded_transaction(data)?;
    let mut inf_resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;
    // Just for parsing the access list
    tx.calculate_hash(&mut inf_resources).map_err(|_| ())?;
    Ok(tx)
}

fn fuzz(data: &[u8]) {
    let Ok(tx) = parse_full_tx(data) else {
        // Input is not valid
        return;
    };

    let Some(slice) = serialize_zksync_transaction(&tx) else {
        return;
    };
    assert_eq!(
        data.len(),
        slice.len(),
        "data.len = {}, slice.len = {},\ndata ={},\nslice={}",
        data.len(),
        slice.len(),
        hex::encode(data),
        hex::encode(slice)
    );
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
