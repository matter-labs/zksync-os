//!
//! These tests are focused on multiblock batch proving inputs.
//!
#![cfg(test)]

use rig::alloy::consensus::{TxEip1559, TxLegacy};
use rig::alloy::primitives::address;
use rig::alloy::primitives::keccak256;
use rig::alloy::primitives::TxKind;
use rig::basic_bootloader::bootloader::block_flow::mandatory_pubdata_prefix_len;
use rig::chain::{get_zksync_os_dist_dir, RunConfig};
use rig::forward_system::run::{
    generate_batch_proof_input, generate_legacy_batch_proof_input, BatchBlockInput,
};
use rig::log::debug;
use rig::ruint::aliases::U256;
use rig::utils::{ERC_20_BYTECODE, ERC_20_MINT_CALLDATA, ERC_20_TRANSFER_CALLDATA};
use rig::zk_ee::common_structs::da_commitment_scheme::{DACommitmentScheme, DAMode};
use rig::zk_ee::system::metadata::chain_config::{ChainConfig, DEFAULT_MAX_TX_GAS_LIMIT};
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{testing_signer, BlockContext, BlockOutput, TestingFramework};

const TEST_STACK_SIZE: usize = 64 << 20;

fn first_mismatch<T: PartialEq>(lhs: &[T], rhs: &[T]) -> Option<usize> {
    lhs.iter()
        .zip(rhs.iter())
        .position(|(lhs_item, rhs_item)| lhs_item != rhs_item)
        .or_else(|| (lhs.len() != rhs.len()).then_some(lhs.len().min(rhs.len())))
}

fn assert_block_output_matches(actual: &BlockOutput, expected: &BlockOutput, context: &str) {
    assert_eq!(
        actual.header.inner(),
        expected.header.inner(),
        "{context}: block header mismatch"
    );
    assert_eq!(
        actual.computational_native_used, expected.computational_native_used,
        "{context}: computational_native_used mismatch"
    );
    assert_eq!(
        actual.pubdata_used, expected.pubdata_used,
        "{context}: pubdata_used mismatch"
    );
    assert_eq!(
        actual.published_preimages, expected.published_preimages,
        "{context}: published preimages mismatch"
    );
    assert_eq!(
        format!("{:?}", actual.tx_results),
        format!("{:?}", expected.tx_results),
        "{context}: tx_results mismatch"
    );
    assert_eq!(
        format!("{:?}", actual.storage_writes),
        format!("{:?}", expected.storage_writes),
        "{context}: storage_writes mismatch"
    );
    assert_eq!(
        format!("{:?}", actual.account_diffs),
        format!("{:?}", expected.account_diffs),
        "{context}: account_diffs mismatch"
    );
}

fn legacy_singleblock_run_config() -> RunConfig {
    let mut config = RunConfig::with_riscv_run();
    config.app = Some("singleblock_batch".to_string());
    config.check_storage_diff_hashes = false;
    config
}

fn new_multiblock_batch_tester() -> TestingFramework {
    let wallet = testing_signer(0);
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_minted_tokens_to_treasury()
}

fn run_multiblock_batch_proof_run(da_commitment_scheme: DACommitmentScheme, da_mode: DAMode) {
    let wallet = testing_signer(0);
    let to = address!("0000000000000000000000000000000000010002");
    // The DA mode is a chain-level rule carried in the chain config (and thereby committed via the
    // public input's chain config hash), so both the batch run and the per-block legacy runs must use
    // a config with this mode.
    let chain_config = ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT)
        .unwrap()
        .with_da_mode(da_mode);
    let mut batch_tester = new_multiblock_batch_tester().with_chain_config(chain_config);
    let block1_context = BlockContext {
        timestamp: 42,
        ..Default::default()
    };
    let block2_context = BlockContext {
        timestamp: 43,
        ..Default::default()
    };

    batch_tester.ensure_account_exists(block1_context.coinbase);
    batch_tester.ensure_account_exists(block2_context.coinbase);

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

    // Batch proving starts from the batch pre-state before block 1 and
    // receives the per-block metadata/tx streams separately.
    let initial_proof_data = batch_tester.prepare_batch_initial_proof_data();
    let batch_state = batch_tester.prepare_batch_state();
    let block1_batch_input =
        batch_tester.prepare_batch_block_input(vec![mint_tx.clone()], Some(block1_context.clone()));

    // Legacy multiblock proving still runs `singleblock_batch` once per block
    // and stitches the resulting witnesses/pubdata together afterwards.
    let mut legacy_tester = new_multiblock_batch_tester()
        .with_da_commitment_scheme(da_commitment_scheme)
        .with_chain_config(chain_config)
        .with_run_config(legacy_singleblock_run_config());
    legacy_tester.set_block_context(Some(block1_context.clone()));
    let _ = legacy_tester.execute_block(vec![mint_tx]);
    let block1_run = legacy_tester
        .last_executed_block_info()
        .expect("legacy block1 run must record proof input");
    let block1_output = block1_run.block_output.clone();
    let block1_proof_input = block1_run.proof_input.clone();
    let block1_pubdata = block1_run.pubdata.clone();
    assert!(
        !block1_proof_input.is_empty(),
        "block1 proof input must be non-empty; proving run is required"
    );

    let transfer_tx = {
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
    let block2_batch_input = legacy_tester
        .prepare_batch_block_input(vec![transfer_tx.clone()], Some(block2_context.clone()));
    legacy_tester.set_block_context(Some(block2_context.clone()));
    let _ = legacy_tester.execute_block(vec![transfer_tx]);
    let block2_run = legacy_tester
        .last_executed_block_info()
        .expect("legacy block2 run must record proof input");
    let block2_output = block2_run.block_output.clone();
    let block2_proof_input = block2_run.proof_input.clone();
    let block2_pubdata = block2_run.pubdata.clone();
    assert!(
        !block2_proof_input.is_empty(),
        "block2 proof input must be non-empty; proving run is required"
    );

    // The main assertion in this test: batch witness generation must match the
    // existing RISC-V-based multiblock flow for the same batch.
    let legacy_batch_input = generate_legacy_batch_proof_input(
        vec![&block1_proof_input, &block2_proof_input],
        da_commitment_scheme,
        da_mode,
        vec![block1_pubdata.as_slice(), block2_pubdata.as_slice()],
    );
    let batch_output = generate_batch_proof_input(
        initial_proof_data,
        batch_state,
        vec![block1_batch_input, block2_batch_input],
        da_commitment_scheme,
        batch_tester.chain_config(),
    )
    .expect("batch prover input generation failed");

    let proof_input_mismatch = first_mismatch(&batch_output.prover_input, &legacy_batch_input);
    let mismatch_window = proof_input_mismatch.map(|idx| {
        let start = idx.saturating_sub(4);
        let end = (idx + 5)
            .min(batch_output.prover_input.len())
            .min(legacy_batch_input.len());
        (
            start,
            end,
            batch_output.prover_input[start..end].to_vec(),
            legacy_batch_input[start..end].to_vec(),
        )
    });
    assert!(
        batch_output.prover_input == legacy_batch_input,
        "batch prover input mismatch at {:?} (batch len {}, legacy len {}, window {:?})",
        proof_input_mismatch,
        batch_output.prover_input.len(),
        legacy_batch_input.len(),
        mismatch_window,
    );
    assert_eq!(
        batch_output.batch_output.first_block_timestamp, block1_context.timestamp,
        "batch first block timestamp mismatch"
    );
    assert_eq!(
        batch_output.batch_output.last_block_timestamp, block2_context.timestamp,
        "batch last block timestamp mismatch"
    );
    assert_eq!(
        batch_output.pubdata,
        [block1_pubdata.clone(), block2_pubdata.clone()].concat(),
        "batch pubdata mismatch"
    );
    // Cross-check the DA commitment against an independent recomputation for the calldata-keccak
    // mechanism (`BlobsAndPubdataKeccak256`). The commitment shape is the Era-compatible structured
    // `keccak256(stateDiffHash(0) || keccak(committed) || 1 || blobHash(0))`; the *committed* bytes
    // depend on the DA mode — the full pubdata stream in `Rollup`, or only the mandatory logs prefix
    // in `Validium`. (The blob mechanism is exercised by the RISC-V proof check below.)
    if da_commitment_scheme == DACommitmentScheme::BlobsAndPubdataKeccak256 {
        let committed_pubdata = if da_mode.commits_full_pubdata() {
            [block1_pubdata.as_slice(), block2_pubdata.as_slice()].concat()
        } else {
            [
                &block1_pubdata[..mandatory_pubdata_prefix_len(&block1_pubdata)],
                &block2_pubdata[..mandatory_pubdata_prefix_len(&block2_pubdata)],
            ]
            .concat()
        };
        let pubdata_keccak = keccak256(&committed_pubdata);
        let mut da_commitment_preimage = Vec::new();
        da_commitment_preimage.extend_from_slice(&[0u8; 32]); // state diffs hash is not validated
        da_commitment_preimage.extend_from_slice(pubdata_keccak.as_slice());
        da_commitment_preimage.push(1); // single "fake" blob for calldata schemes
        da_commitment_preimage.extend_from_slice(&[0u8; 32]); // blob hash ignored on the settlement layer
        let expected_commitment = keccak256(&da_commitment_preimage);
        assert_eq!(
            batch_output.batch_output.pubdata_commitment.as_u8_ref(),
            expected_commitment.as_slice(),
            "DA commitment mismatch for {da_commitment_scheme:?} / {da_mode:?}"
        );
    }
    assert_eq!(
        batch_output.block_outputs.len(),
        2,
        "batch prover input should return one BlockOutput per block"
    );
    assert_block_output_matches(&batch_output.block_outputs[0], &block1_output, "block1");
    assert_block_output_matches(&batch_output.block_outputs[1], &block2_output, "block2");
    assert_eq!(
        batch_output.batch_public_input.batch_output,
        batch_output.batch_output.hash().into(),
        "batch public input should commit to the batch output hash"
    );

    // Feed the batch witness into the proving binary to ensure it is not
    // only equal to the legacy witness, but also produces the expected final
    // public input hash when executed by the batch prover.

    let multiblock_dist_dir = get_zksync_os_dist_dir(&Some("multiblock_batch".to_string()));

    let proof_output = zksync_os_runner::Runner::new(multiblock_dist_dir)
        .run(&batch_output.prover_input)
        .output;

    debug!("Proof running output = 0x");
    for word in proof_output.into_iter() {
        debug!("{word:08x}");
    }

    let proof_output_u8: [u8; 32] = unsafe { core::mem::transmute(proof_output) };
    assert_eq!(
        proof_output_u8,
        batch_output.batch_public_input.hash(),
        "RISC-V batch proof output mismatch"
    );
}

fn run_singleblock_batch_proof_run(da_commitment_scheme: DACommitmentScheme, da_mode: DAMode) {
    let wallet = testing_signer(0);
    let to = address!("0000000000000000000000000000000000010002");
    let chain_config = ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT)
        .unwrap()
        .with_da_mode(da_mode);
    let mut batch_tester = new_multiblock_batch_tester().with_chain_config(chain_config);
    let block_context = BlockContext {
        timestamp: 42,
        ..Default::default()
    };

    batch_tester.ensure_account_exists(block_context.coinbase);

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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet)
    };

    let initial_proof_data = batch_tester.prepare_batch_initial_proof_data();
    let batch_state = batch_tester.prepare_batch_state();
    let block_batch_input =
        batch_tester.prepare_batch_block_input(vec![mint_tx.clone()], Some(block_context.clone()));

    let mut legacy_tester = new_multiblock_batch_tester()
        .with_da_commitment_scheme(da_commitment_scheme)
        .with_chain_config(chain_config)
        .with_run_config(legacy_singleblock_run_config());
    legacy_tester.set_block_context(Some(block_context.clone()));
    let _ = legacy_tester.execute_block(vec![mint_tx]);
    let block_run = legacy_tester
        .last_executed_block_info()
        .expect("legacy block run must record proof input");
    let legacy_block_output = block_run.block_output.clone();
    let legacy_proof_input = block_run.proof_input.clone();
    let legacy_pubdata = block_run.pubdata.clone();

    let legacy_batch_input = generate_legacy_batch_proof_input(
        vec![legacy_proof_input.as_slice()],
        da_commitment_scheme,
        da_mode,
        vec![legacy_pubdata.as_slice()],
    );
    let batch_output = generate_batch_proof_input(
        initial_proof_data,
        batch_state,
        vec![block_batch_input],
        da_commitment_scheme,
        batch_tester.chain_config(),
    )
    .expect("single-block batch prover input generation failed");

    assert_eq!(
        batch_output.prover_input, legacy_batch_input,
        "single-block batch prover input mismatch"
    );
    assert_eq!(
        batch_output.batch_output.first_block_timestamp, block_context.timestamp,
        "single-block batch first block timestamp mismatch"
    );
    assert_eq!(
        batch_output.batch_output.last_block_timestamp, block_context.timestamp,
        "single-block batch last block timestamp mismatch"
    );
    assert_eq!(
        batch_output.pubdata, legacy_pubdata,
        "single-block batch pubdata mismatch"
    );
    assert_eq!(
        batch_output.block_outputs.len(),
        1,
        "single-block batch prover input should return one BlockOutput"
    );
    assert_block_output_matches(
        &batch_output.block_outputs[0],
        &legacy_block_output,
        "single-block",
    );
    assert_eq!(
        batch_output.batch_public_input.batch_output,
        batch_output.batch_output.hash().into(),
        "single-block batch public input should commit to the batch output hash"
    );

    let multiblock_dist_dir = get_zksync_os_dist_dir(&Some("multiblock_batch".to_string()));
    let proof_output = zksync_os_runner::Runner::new(multiblock_dist_dir)
        .run(&batch_output.prover_input)
        .output;

    let proof_output_u8: [u8; 32] = unsafe { core::mem::transmute(proof_output) };
    assert_eq!(
        proof_output_u8,
        batch_output.batch_public_input.hash(),
        "single-block RISC-V batch proof output mismatch"
    );
}

#[test]
fn run_multiblock_batch_proof_run_calldata() {
    // The proving flow overflows the default libtest stack in this instance.
    std::thread::Builder::new()
        .name("multiblock_batch_calldata".to_owned())
        .stack_size(TEST_STACK_SIZE)
        .spawn(|| {
            run_multiblock_batch_proof_run(
                DACommitmentScheme::BlobsAndPubdataKeccak256,
                DAMode::Rollup,
            )
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn run_multiblock_batch_proof_run_blobs() {
    std::thread::Builder::new()
        .name("multiblock_batch_blobs".to_owned())
        .stack_size(TEST_STACK_SIZE)
        .spawn(|| run_multiblock_batch_proof_run(DACommitmentScheme::BlobsZKsyncOS, DAMode::Rollup))
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn run_multiblock_batch_proof_run_validium() {
    // Validium = the calldata mechanism with the DA mode set to `Validium`, so only the mandatory
    // logs section is committed.
    std::thread::Builder::new()
        .name("multiblock_batch_validium".to_owned())
        .stack_size(TEST_STACK_SIZE)
        .spawn(|| {
            run_multiblock_batch_proof_run(
                DACommitmentScheme::BlobsAndPubdataKeccak256,
                DAMode::Validium,
            )
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn run_multiblock_batch_proof_run_validium_blobs() {
    std::thread::Builder::new()
        .name("multiblock_batch_validium_blobs".to_owned())
        .stack_size(TEST_STACK_SIZE)
        .spawn(|| {
            run_multiblock_batch_proof_run(DACommitmentScheme::BlobsZKsyncOS, DAMode::Validium)
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn batch_helpers_reject_empty_batches() {
    let tester = new_multiblock_batch_tester();
    let initial_proof_data = tester.prepare_batch_initial_proof_data();
    let batch_state = tester.prepare_batch_state();
    let chain_config = tester.chain_config();

    let batch_rejected = std::panic::catch_unwind(|| {
        let empty_blocks: Vec<BatchBlockInput<rig::zksync_os_interface::traits::TxListSource>> =
            Vec::new();
        let _ = generate_batch_proof_input(
            initial_proof_data,
            batch_state,
            empty_blocks,
            DACommitmentScheme::BlobsAndPubdataKeccak256,
            chain_config,
        );
    });
    assert!(
        batch_rejected.is_err(),
        "batch prover input should reject empty batches"
    );
}

#[test]
fn run_singleblock_batch_proof_run_calldata() {
    std::thread::Builder::new()
        .name("singleblock_batch_calldata".to_owned())
        .stack_size(TEST_STACK_SIZE)
        .spawn(|| {
            run_singleblock_batch_proof_run(
                DACommitmentScheme::BlobsAndPubdataKeccak256,
                DAMode::Rollup,
            )
        })
        .unwrap()
        .join()
        .unwrap();
}
