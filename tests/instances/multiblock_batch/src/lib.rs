//!
//! These tests are focused on different tx types
//!
#![cfg(test)]

use alloy::consensus::{TxEip1559, TxLegacy};
use alloy::primitives::TxKind;
use alloy::signers::local::PrivateKeySigner;
use rig::alloy::primitives::address;
use rig::forward_system::run::generate_batch_proof_input;
use rig::log::debug;
use rig::ruint::aliases::{B160, U256};
use rig::utils::{ERC_20_BYTECODE, ERC_20_MINT_CALLDATA, ERC_20_TRANSFER_CALLDATA};
use rig::zk_ee::common_structs::DACommitmentScheme;
use rig::zk_ee::system::tracer::NopTracer;
use rig::{alloy, zksync_web3_rs, Chain};
use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
use std::path::PathBuf;
use std::str::FromStr;
use zksync_web3_rs::signers::{LocalWallet, Signer};

fn run_multiblock_batch_proof_run(da_commitment_scheme: DACommitmentScheme) {
    let mut chain = Chain::empty(None);
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 80_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let block1_result = chain
        .run_block_with_extra_stats(
            vec![encoded_mint_tx],
            None,
            Some(da_commitment_scheme),
            None,
            &mut NopTracer::default(),
        )
        .unwrap();
    let encoded_transfer_tx = {
        let transfer_tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 1,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 60_000,
            to: TxKind::Call(to),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(transfer_tx, &wallet)
    };

    let block2_result = chain
        .run_block_with_extra_stats(
            vec![encoded_transfer_tx],
            None,
            Some(da_commitment_scheme),
            None,
            &mut NopTracer::default(),
        )
        .unwrap();

    let batch_input = generate_batch_proof_input(
        vec![block1_result.2.as_slice(), block2_result.2.as_slice()],
        da_commitment_scheme,
        vec![
            block1_result.0.pubdata.as_slice(),
            block2_result.0.pubdata.as_slice(),
        ],
    );

    let multinblock_program_path = PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap())
        .join("zksync_os")
        .join("multiblock_batch.bin");

    let proof_output = zksync_os_runner::run(
        multinblock_program_path,
        None,
        1 << 36,
        QuasiUARTSource::new_with_reads(batch_input),
    );

    debug!("Proof running output = 0x",);
    for word in proof_output.into_iter() {
        debug!("{word:08x}");
    }

    // Ensure that proof running didn't fail: check that output is not zero
    assert!(proof_output.into_iter().any(|word| word != 0));
}

#[test]
fn run_multiblock_batch_proof_run_calldata() {
    run_multiblock_batch_proof_run(DACommitmentScheme::BlobsAndPubdataKeccak256);
}

#[test]
fn run_multiblock_batch_proof_run_blobs() {
    run_multiblock_batch_proof_run(DACommitmentScheme::BlobsZKsyncOS);
}
