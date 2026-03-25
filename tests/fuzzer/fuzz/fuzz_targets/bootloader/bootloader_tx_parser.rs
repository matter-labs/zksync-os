#![no_main]
#![feature(allocator_api)]

use common::{mutate_transaction, parse_abi_encoded_transaction, serialize_zksync_transaction};
use libfuzzer_sys::{fuzz_mutator, fuzz_target};
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::Resource;

mod common;

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    mutate_transaction(data, size, max_size, seed)
});

fn fuzz(data: &[u8]) {
    let Ok(transaction) = parse_abi_encoded_transaction(data) else {
        // Input is not valid
        return;
    };

    let mut inf_resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;

    let _ = transaction.tx_type.read();
    let _ = transaction.required_balance();
    let _ = transaction.calldata();
    let _ = transaction.encoding(transaction.paymaster_input.clone());
    let _ = transaction.encoding(transaction.signature.clone());
    let _ = transaction.len();
    let _ = transaction.calculate_hash(&mut inf_resources);
    let _ = serialize_zksync_transaction(&transaction);
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
