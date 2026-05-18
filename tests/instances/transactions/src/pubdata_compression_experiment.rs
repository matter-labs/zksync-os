//! Experimental measurement of deflate compression over the v2 pubdata blob.
//!
//! Reuses an existing meaty workload (`run_base_system`-style: mint + transfer
//! + deploy + L1->L2 + service-tx) to obtain a pubdata blob with realistic
//! shape (initial + repeated storage diffs, an account/code deploy, L2->L1
//! logs), then runs the host-side helper in
//! `rig::pubdata_compression` to report compressed-vs-uncompressed sizes at a
//! few compression levels and with the zlib wrapper. Prints a single summary
//! line per scenario to stderr so it surfaces under `cargo test -- --nocapture`.
//!
//! No protocol behavior is changed. This is purely instrumentation to answer
//! "how much would deflate save on real pubdata?" before deciding whether to
//! invest in an in-circuit compression envelope.

use rig::alloy;
use rig::alloy::consensus::{TxEip1559, TxEip2930, TxLegacy};
use rig::alloy::primitives::{address, TxKind};
use rig::pubdata_compression::{deflate, deflate_zlib, measure_deflate};
use rig::ruint::aliases::U256;
use rig::utils::*;
use rig::{common_target_address, testing_signer, TestingFramework};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

fn report(label: &str, raw: &[u8]) {
    let mut rows: Vec<String> = Vec::new();
    for level in [1u8, 6u8, 9u8] {
        let m = measure_deflate(raw, level);
        rows.push(format!(
            "lvl{}={}B ({:.1}%)",
            level,
            m.compressed_len,
            m.ratio() * 100.0
        ));
    }
    let zlib = deflate_zlib(raw, 9);
    rows.push(format!(
        "zlib9={}B ({:.1}%)",
        zlib.len(),
        if raw.is_empty() {
            100.0
        } else {
            zlib.len() as f64 / raw.len() as f64 * 100.0
        }
    ));
    eprintln!(
        "[pubdata_compression] {} raw={}B  {}",
        label,
        raw.len(),
        rows.join("  ")
    );
}

/// Single ERC-20 mint + transfer in one block. Smallest realistic workload —
/// useful as a lower-bound for the deflate overhead on short blobs.
#[test]
fn measure_mint_transfer_block() {
    let wallet = testing_signer(0);
    let to = address!("0000000000000000000000000000000000010002");

    let mint_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 80_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        },
        wallet.clone(),
    );
    let transfer_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxEip1559 {
            chain_id: 37u64,
            nonce: 1,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 60_000,
            to: TxKind::Call(to),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        },
        wallet.clone(),
    );

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_prefunded_account(wallet.address());

    let _ = tester.execute_block(vec![mint_tx, transfer_tx]);
    let pubdata = tester
        .last_executed_block_info()
        .expect("must have block info")
        .pubdata
        .clone();

    report("mint+transfer", &pubdata);
    // Sanity: deflate must produce at least one byte and not balloon the input
    // beyond a sane upper bound. We do not assert a savings target — the goal
    // is observation, not regression-locking compression numbers.
    let d = deflate(&pubdata, 9);
    assert!(!d.is_empty());
    assert!(d.len() <= pubdata.len() * 2 + 128);
}

/// Mixed-workload block matching `run_base_system`: ERC-20 mint, transfer,
/// fresh contract deploy, plain value transfer, mint to a different address,
/// and two L1->L2 txs. Exercises initial + repeated storage diffs, account
/// code deploy bytes, and L2->L1 log emission.
#[test]
fn measure_mixed_block() {
    let wallet = testing_signer(0);
    let eoa_wallet = testing_signer(1);
    let to = address!("0000000000000000000000000000000000010002");

    let mint_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 80_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        },
        wallet.clone(),
    );
    let transfer_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxEip1559 {
            chain_id: 37u64,
            nonce: 1,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 60_000,
            to: TxKind::Call(to),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        },
        wallet.clone(),
    );
    let deployment_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxEip2930 {
            chain_id: 37u64,
            nonce: 2,
            gas_price: 1000,
            gas_limit: 900_000,
            to: TxKind::Create,
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_DEPLOYMENT_BYTECODE).unwrap().into(),
        },
        wallet.clone(),
    );
    let transfer_to_eoa_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 21_000,
            to: TxKind::Call(common_target_address()),
            value: alloy::primitives::U256::from(100),
            access_list: Default::default(),
            input: Default::default(),
        },
        eoa_wallet.clone(),
    );
    let mint2_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxEip1559 {
            chain_id: 37u64,
            nonce: 3,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 60_000,
            to: TxKind::Call(address!("14c252e395055507b10f199dd569f2379465d874")),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        },
        wallet.clone(),
    );

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_prefunded_account(wallet.address())
        .with_prefunded_account(eoa_wallet.address())
        .with_balance(
            address!("1234000000000000000000000000000000000000"),
            U256::from(100),
        );

    let _ = tester.execute_block(vec![
        mint_tx,
        transfer_tx,
        deployment_tx,
        transfer_to_eoa_tx,
        mint2_tx,
    ]);
    let pubdata = tester
        .last_executed_block_info()
        .expect("must have block info")
        .pubdata
        .clone();

    report("mint+transfer+deploy+eoa+mint2", &pubdata);
}

/// `run_block_of_erc20(N)` runs a synthetic ERC-20 block with N transfers to
/// freshly-seeded recipients. This is the workload the rig already uses to
/// stress storage-diff emission, so the pubdata is close to a "typical busy
/// block" shape.
///
/// Uses the non-randomized tree: the randomized tree pre-fills slots at high
/// indices, which on v2 currently overflows the default 5-byte
/// `repeated_write_index_encoding_length` and is orthogonal to compression.
#[test]
fn measure_block_of_erc20() {
    let mut tester = TestingFramework::new();
    let _ = tester.run_block_of_erc20(20, None);
    let pubdata = tester
        .last_executed_block_info()
        .expect("must have block info")
        .pubdata
        .clone();
    report("block_of_erc20(20)", &pubdata);
}
