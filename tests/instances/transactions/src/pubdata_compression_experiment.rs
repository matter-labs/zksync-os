//! Sanity / measurement tests for the v2 deflate-wrapped pubdata body.
//!
//! Runs realistic workloads (mint + transfer, mixed deploy block, ERC-20
//! sweep), captures the v2 pubdata, and prints a one-line size summary
//! plus how much a *second* deflate pass would shave off. The second pass
//! should add a handful of overhead bytes — that confirms the in-STF
//! compressor is doing its job; a sudden ability to compress further
//! would mean the STF stopped emitting deflated bytes.
//!
//! The `inflate_roundtrip_…` tests are the protocol-level guard: they
//! parse the 41-byte header, run a host-side inflate over the body, and
//! confirm the inflated bytes have the v2 storage-diffs header structure.

use rig::alloy;
use rig::alloy::consensus::{TxEip1559, TxEip2930, TxLegacy};
use rig::alloy::primitives::{address, TxKind};
use rig::basic_bootloader::bootloader::block_flow::zk::PUBDATA_ENCODING_VERSION;
use rig::pubdata_compression::{deflate, deflate_zlib, measure_deflate};
use rig::ruint::aliases::U256;
use rig::utils::*;
use rig::{common_target_address, testing_signer, TestingFramework};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

/// v2 layout: `[VERSION:1][BLOCK_HASH:32][TIMESTAMP:8][DEFLATE(BODY)]`.
const PUBDATA_HEADER_LEN: usize = 1 + 32 + 8;

/// Split a captured v2 pubdata blob into the uncompressed header bytes and
/// the inflated body bytes. Panics if the header or deflate stream is malformed.
fn inflate_pubdata(pubdata: &[u8]) -> (Vec<u8>, Vec<u8>) {
    assert!(
        pubdata.len() >= PUBDATA_HEADER_LEN,
        "pubdata shorter than 41-byte header"
    );
    assert_eq!(
        pubdata[0], PUBDATA_ENCODING_VERSION,
        "version byte mismatch"
    );
    let header = pubdata[..PUBDATA_HEADER_LEN].to_vec();
    let deflated = &pubdata[PUBDATA_HEADER_LEN..];
    let body = miniz_oxide::inflate::decompress_to_vec(deflated)
        .expect("deflate body must inflate cleanly");
    (header, body)
}

/// Minimal v2-body structure check: header is
/// `[TOTAL_DIFFS:4][NB_ACCOUNT_INITIAL:4][NB_SLOT_INITIAL:4][INDEX_LEN:1]`,
/// and `NB_ACCOUNT_INITIAL + NB_SLOT_INITIAL <= TOTAL_DIFFS`. We don't
/// reproduce the full storage-diff parser — that lives in the L1 contract
/// — but a malformed body would fail this basic invariant.
fn assert_v2_body_shape(body: &[u8]) {
    assert!(body.len() >= 13, "body shorter than v2 diffs header");
    let total = u32::from_be_bytes(body[0..4].try_into().unwrap());
    let nb_acc = u32::from_be_bytes(body[4..8].try_into().unwrap());
    let nb_slot = u32::from_be_bytes(body[8..12].try_into().unwrap());
    let index_len = body[12];
    assert!(
        nb_acc.saturating_add(nb_slot) <= total,
        "initial counts {nb_acc}+{nb_slot} exceed total diffs {total}"
    );
    assert!(
        (1..=8).contains(&index_len),
        "repeated_write_index_encoding_length {index_len} out of sane range"
    );
}

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

/// v3 protocol round-trip: emit pubdata from the mixed deploy block, host-
/// side inflate the body, and confirm the inflated bytes start with a valid
/// v2-diffs header. This is the canonical check that the deflate envelope
/// is recoverable by an external consumer.
#[test]
fn inflate_roundtrip_mixed_block() {
    let wallet = testing_signer(0);
    let to = address!("0000000000000000000000000000000000010002");

    let deployment_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 900_000,
            to: TxKind::Create,
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_DEPLOYMENT_BYTECODE).unwrap().into(),
        },
        wallet.clone(),
    );
    let mint_tx = ZKsyncTxEnvelope::from_eth_tx(
        TxLegacy {
            chain_id: 37u64.into(),
            nonce: 1,
            gas_price: 1000,
            gas_limit: 80_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        },
        wallet.clone(),
    );

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_prefunded_account(wallet.address());

    let _ = tester.execute_block(vec![deployment_tx, mint_tx]);
    let pubdata = tester
        .last_executed_block_info()
        .expect("must have block info")
        .pubdata
        .clone();

    let (header, body) = inflate_pubdata(&pubdata);
    assert_eq!(header.len(), PUBDATA_HEADER_LEN);
    assert_v2_body_shape(&body);
    eprintln!(
        "[pubdata_compression] roundtrip header={}B  deflate_body={}B  inflated_body={}B",
        header.len(),
        pubdata.len() - header.len(),
        body.len(),
    );
}

/// v3 with effectively no storage activity — only an EOA→EOA value transfer.
/// Confirms the empty / nearly-empty body case round-trips too.
#[test]
fn inflate_roundtrip_minimal_block() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let target = common_target_address();

    let tx = ZKsyncTxEnvelope::from_eth_tx(
        TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(target),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        },
        wallet.clone(),
    );
    tester = tester.with_balance(wallet.address(), U256::from(u64::MAX));

    let _ = tester.execute_block(vec![tx]);
    let pubdata = tester
        .last_executed_block_info()
        .expect("must have block info")
        .pubdata
        .clone();

    let (header, body) = inflate_pubdata(&pubdata);
    assert_eq!(header[0], PUBDATA_ENCODING_VERSION);
    assert_v2_body_shape(&body);
}
