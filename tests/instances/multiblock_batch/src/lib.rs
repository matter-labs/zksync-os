//!
//! These tests are focused on different tx types
//!
#![cfg(test)]

use alloy::consensus::{TxEip1559, TxLegacy};
use alloy::primitives::TxKind;
use rig::alloy::primitives::address;
use rig::chain::RunConfig;
use rig::forward_system::run::generate_batch_proof_input;
use rig::log::debug;
use rig::ruint::aliases::U256;
use rig::utils::{ERC_20_BYTECODE, ERC_20_MINT_CALLDATA, ERC_20_TRANSFER_CALLDATA};
use rig::zk_ee::common_structs::DACommitmentScheme;
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{alloy, testing_signer, TestingFramework};
use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;
use std::path::PathBuf;

fn run_multiblock_batch_proof_run(da_commitment_scheme: DACommitmentScheme) {
    let wallet = testing_signer(0);

    let to = address!("0000000000000000000000000000000000010002");

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    let mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 80_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_run_config(RunConfig::with_riscv_run())
        .with_da_commitment_scheme(da_commitment_scheme);

    tester.execute_block(vec![mint_tx]);
    let block1_info = tester.last_executed_block_info().unwrap();
    let block1_proof_input = block1_info.proof_input.clone();
    let block1_pubdata = block1_info.pubdata.clone();
    assert!(
        !block1_proof_input.is_empty(),
        "block1 proof input must be non-empty; proving run is required"
    );

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
        ZKsyncTxEnvelope::from_eth_tx(transfer_tx, wallet.clone())
    };

    tester.execute_block(vec![encoded_transfer_tx]);
    let block2_info = tester.last_executed_block_info().unwrap();
    let block2_proof_input = block2_info.proof_input.clone();
    let block2_pubdata = block2_info.pubdata.clone();
    assert!(
        !block2_proof_input.is_empty(),
        "block2 proof input must be non-empty; proving run is required"
    );

    let batch_input = generate_batch_proof_input(
        vec![&block1_proof_input, &block2_proof_input],
        da_commitment_scheme,
        vec![block1_pubdata.as_slice(), block2_pubdata.as_slice()],
    );

    let multinblock_program_path = PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap())
        .join("zksync_os")
        .join("multiblock_batch.bin");

    let proof_output = zksync_os_runner::run(
        multinblock_program_path,
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
