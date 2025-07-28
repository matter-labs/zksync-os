#![no_main]

use libfuzzer_sys::fuzz_target;
use revm::precompile::bn128;
use rig::ethers::signers::Signer;
use ruint::aliases::{B160, U256};
mod common;

fuzz_target!(|data: &[u8]| {
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_wallet();
    let tx = rig::utils::sign_and_encode_ethers_legacy_tx(
        common::get_tx("0000000000000000000000000000000000000007", data),
        &wallet,
    );
    chain.set_balance(
        B160::from_be_bytes(wallet.address().0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let block_output = chain.run_block(vec![tx], None, None);

    #[allow(unused_variables)]
    let output = block_output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .expect("Tx should have succeeded");

    let zksync_os_bytes = output.as_returned_bytes();
    let revm_res = bn128::run_mul(data, 1 << 27, 1 << 27);

    match revm_res {
        Ok(revm) => assert_eq!(zksync_os_bytes, revm.bytes.to_vec()),
        Err(_) => assert!(common::is_zero(zksync_os_bytes)),
    }
});
