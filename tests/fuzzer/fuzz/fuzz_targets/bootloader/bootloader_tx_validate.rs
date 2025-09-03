#![no_main]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]

use basic_bootloader::bootloader::transaction::ZkSyncTransaction;
use common::{mutate_transaction, serialize_zksync_transaction};
use libfuzzer_sys::{fuzz_mutator, fuzz_target};
use zk_ee::{
    reference_implementations::{BaseResources, DecreasingNative},
    system::Resource,
};
mod common;

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    mutate_transaction(data, size, max_size, seed)
});

fn parse_full_tx(data: &mut [u8]) -> Result<ZkSyncTransaction, ()> {
    let tx = ZkSyncTransaction::try_from_slice(data)?;
    let mut inf_resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;
    // Just for parsing the access list
    tx.calculate_hash(37, &mut inf_resources).map_err(|_| ())?;
    Ok(tx)
}

fn fuzz(data: &[u8]) {
    let mut data = data.to_owned();
    let Ok(tx) = parse_full_tx(&mut data) else {
        // Input is not valid
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
