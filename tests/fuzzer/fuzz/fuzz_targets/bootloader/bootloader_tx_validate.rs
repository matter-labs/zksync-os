#![no_main]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]

use basic_bootloader::bootloader::transaction::ZkSyncTransaction;
use common::{mutate_transaction, serialize_zksync_transaction};
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

    let slice = serialize_zksync_transaction(&tx);
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
