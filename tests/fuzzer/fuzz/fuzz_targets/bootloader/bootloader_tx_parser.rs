#![no_main]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]

use basic_bootloader::bootloader::transaction::ZkSyncTransaction;
use common::mutate_transaction;
use libfuzzer_sys::{fuzz_mutator, fuzz_target};
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
mod common;
use zk_ee::system::Resource;

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    mutate_transaction(data, size, max_size, seed)
});

fn fuzz(data: &[u8]) {
    let mut data = data.to_owned();
    let Ok(transaction) = ZkSyncTransaction::try_from_slice(&mut data) else {
        if data.len() != 0 {
            panic!("input is not valid {:?}", data);
        }
        return;
    };

    let mut inf_resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;

    let _ = transaction.tx_type.read();
    let _ = transaction.required_balance();
    let _ = transaction.calldata();
    let _ = transaction.is_eip_712();
    let _ = transaction.paymaster_input();
    let _ = transaction.signature();
    let _ = transaction.tx_body_length();

    let chain_id = 0;
    let _ = transaction.calculate_signed_hash(chain_id, &mut inf_resources);
    let _ = transaction.calculate_hash(chain_id, &mut inf_resources);
    let _ = transaction.get_user_gas_per_pubdata_limit();

    let mut transaction = transaction;
    let _ = transaction.underlying_buffer();
    let _ = transaction.pre_tx_buffer();
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
