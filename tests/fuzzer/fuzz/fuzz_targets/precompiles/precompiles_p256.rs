#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use revm::precompile;
use rig::ethers::signers::Signer;
use ruint::aliases::{B160, U256};
mod common;

const P256_SRC_REQUIRED_LENGTH: usize = 160;

#[derive(Debug, Arbitrary)]
struct Input {
    src: [u8; P256_SRC_REQUIRED_LENGTH],
}

fuzz_target!(|input: Input| {
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_wallet();
    let tx = rig::utils::sign_and_encode_ethers_legacy_tx(
        common::get_tx(
            "0000000000000000000000000000000000000100",
            input.src.as_ref(),
        ),
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
    let bytes: alloy::primitives::Bytes = input.src.into();
    let revm_res = precompile::secp256r1::p256_verify(&bytes, 1 << 27);

    match revm_res {
        Ok(revm) => assert_eq!(zksync_os_bytes, revm.bytes.to_vec()),
        Err(_) => assert!(common::is_zero(zksync_os_bytes)),
    }
});
