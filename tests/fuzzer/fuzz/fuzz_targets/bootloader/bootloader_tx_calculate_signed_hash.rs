#![no_main]
#![feature(allocator_api)]

use basic_bootloader::bootloader::transaction::Transaction;
use common::{mutate_transaction, parse_abi_encoded_transaction};
use libfuzzer_sys::{fuzz_mutator, fuzz_target};

mod common;

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    mutate_transaction(data, size, max_size, seed)
});

fn fuzz(data: &[u8]) {
    let Ok(tx) = parse_abi_encoded_transaction(data) else {
        // Input is not valid
        return;
    };

    let mut transaction = Transaction::Abi(tx);
    let _ = transaction.signed_hash();
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
