//!
//! These tests are focused on interop support in ZKsync OS.
//!
#![cfg(test)]

mod bytecodes;

use rig::alloy::consensus::TxLegacy;
use rig::alloy::primitives::{address, Address, TxKind, B256 as AlloyB256};
use rig::basic_bootloader::bootloader::transaction::rlp_encoded::transaction_types::service_tx::SET_INTEROP_FEE_SELECTOR;
use rig::crypto::MiniDigest;
use rig::ruint::aliases::{B160, B256, U256};
use rig::system_hooks::addresses_constants::{
    L2_INTEROP_CENTER_ADDRESS, L2_INTEROP_ROOT_STORAGE_ADDRESS, SYSTEM_CONTEXT_ADDRESS,
};
use rig::utils::{
    encode_interop_root_import_calldata, encode_set_settlement_layer_chain_id_calldata,
};
use rig::zk_ee::common_structs::interop_root_storage::InteropRoot as StoredInteropRoot;
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_interface::types::{BlockOutput, ExecutionOutput, ExecutionResult};
use rig::{testing_signer, BlockContext, TestingFramework};
use zksync_os_tests_common::zksync_tx::service_tx::ZKsyncServiceTx;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

use bytecodes::{
    INTEROP_CENTER_FEE_MOCK_BYTECODE, L2_CHAIN_ASSET_HANDLER_BYTECODE,
    L2_INTEROP_ROOT_STORAGE_BYTECODE, SYSTEM_CONTEXT_BYTECODE,
};

const L2_CHAIN_ASSET_HANDLER_ADDRESS: B160 = B160::from_limbs([0x1000a, 0, 0]);
// Slot in the inlined InteropCenter runtime used by these tests.
const INTEROP_PROTOCOL_FEE_SLOT: u64 = 0xcb;
const INTEROP_FEE_UPDATED_EVENT_SIG: [u8; 32] = [
    0xcc, 0x1b, 0x3d, 0x8a, 0x49, 0x7e, 0x60, 0x02, 0xe2, 0x6d, 0x00, 0x75, 0x70, 0xcc, 0x91, 0x77,
    0xa5, 0x0c, 0x5c, 0xaf, 0xc2, 0x2a, 0xbb, 0xa1, 0xff, 0x68, 0x15, 0x5b, 0x8e, 0x88, 0x06, 0xbc,
];

fn b160_to_address(value: B160) -> Address {
    Address::from_slice(&value.to_be_bytes::<20>())
}

fn service_block_context() -> BlockContext {
    BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    }
}

fn with_interop_root_storage_contract(tester: TestingFramework) -> TestingFramework {
    let bytecode =
        hex::decode(L2_INTEROP_ROOT_STORAGE_BYTECODE).expect("interop root bytecode must decode");
    tester.with_evm_contract(b160_to_address(L2_INTEROP_ROOT_STORAGE_ADDRESS), &bytecode)
}

fn with_system_context_contracts(tester: TestingFramework) -> TestingFramework {
    let system_context_bytecode =
        hex::decode(SYSTEM_CONTEXT_BYTECODE).expect("system context bytecode must decode");
    let chain_asset_handler_bytecode = hex::decode(L2_CHAIN_ASSET_HANDLER_BYTECODE)
        .expect("chain asset handler bytecode must decode");

    tester
        .with_evm_contract(
            b160_to_address(SYSTEM_CONTEXT_ADDRESS),
            &system_context_bytecode,
        )
        .with_evm_contract(
            b160_to_address(L2_CHAIN_ASSET_HANDLER_ADDRESS),
            &chain_asset_handler_bytecode,
        )
}

fn with_interop_center_contract(tester: TestingFramework) -> TestingFramework {
    // Mock InteropCenter runtime for fee-path tests.
    let interop_center_bytecode = hex::decode(INTEROP_CENTER_FEE_MOCK_BYTECODE)
        .expect("interop center mock bytecode must decode");

    tester.with_evm_contract(
        b160_to_address(L2_INTEROP_CENTER_ADDRESS),
        &interop_center_bytecode,
    )
}

fn interop_root_import_tx(interop_roots: Vec<StoredInteropRoot>, salt: u64) -> ZKsyncTxEnvelope {
    let calldata = encode_interop_root_import_calldata(interop_roots);
    ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: b160_to_address(L2_INTEROP_ROOT_STORAGE_ADDRESS),
        input: calldata.into(),
        salt,
    })
}

fn set_sl_chain_id_tx(new_sl_chain_id: U256, salt: u64) -> ZKsyncTxEnvelope {
    let calldata = encode_set_settlement_layer_chain_id_calldata(new_sl_chain_id);
    ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: b160_to_address(SYSTEM_CONTEXT_ADDRESS),
        input: calldata.into(),
        salt,
    })
}

fn set_interop_fee_tx(new_fee: U256, salt: u64) -> ZKsyncTxEnvelope {
    let mut calldata = Vec::with_capacity(4 + 32);
    calldata.extend_from_slice(&SET_INTEROP_FEE_SELECTOR);
    calldata.extend_from_slice(&new_fee.to_be_bytes::<32>());

    ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: b160_to_address(L2_INTEROP_CENTER_ADDRESS),
        input: calldata.into(),
        salt,
    })
}

fn read_sl_chain_id_slot(tester: &mut TestingFramework) -> U256 {
    tester
        .get_storage_slot(&b160_to_address(SYSTEM_CONTEXT_ADDRESS), U256::ZERO)
        .map(|slot| slot.into_u256_be())
        .expect("SYSTEM_CONTEXT slot 0 should be initialized")
}

fn interop_root_mapping_slot(chain_id: U256, block_or_batch_number: U256) -> U256 {
    // mapping(uint256 => mapping(uint256 => bytes32)) interopRoots; // slot 0
    let mut hasher = rig::crypto::sha3::Keccak256::new();
    hasher.update(chain_id.to_be_bytes::<32>());
    hasher.update(U256::ZERO.to_be_bytes::<32>());
    let chain_mapping_slot = hasher.finalize();

    let mut hasher = rig::crypto::sha3::Keccak256::new();
    hasher.update(block_or_batch_number.to_be_bytes::<32>());
    hasher.update(chain_mapping_slot);
    U256::from_be_bytes(hasher.finalize())
}

fn read_interop_root_slot(
    tester: &mut TestingFramework,
    chain_id: U256,
    block_or_batch_number: U256,
) -> Option<Bytes32> {
    tester.get_storage_slot(
        &b160_to_address(L2_INTEROP_ROOT_STORAGE_ADDRESS),
        interop_root_mapping_slot(chain_id, block_or_batch_number),
    )
}

fn read_interop_protocol_fee_slot(tester: &mut TestingFramework) -> Option<U256> {
    tester
        .get_storage_slot(
            &b160_to_address(L2_INTEROP_CENTER_ADDRESS),
            U256::from(INTEROP_PROTOCOL_FEE_SLOT),
        )
        .map(|slot| slot.into_u256_be())
}

fn assert_interop_fee_updated_log(
    tx_logs: &[rig::zksync_os_interface::types::Log],
    expected_old_fee: U256,
    expected_new_fee: U256,
) {
    assert_eq!(tx_logs.len(), 1, "exactly one event should be emitted");
    let log = &tx_logs[0];
    assert_eq!(log.address, b160_to_address(L2_INTEROP_CENTER_ADDRESS));

    let topics = log.topics();
    assert_eq!(
        topics[0],
        AlloyB256::from_slice(&INTEROP_FEE_UPDATED_EVENT_SIG)
    );

    let (old_fee, new_fee) = match topics.len() {
        // old/new fee are non-indexed event fields.
        1 => {
            let data = log.data.data.as_ref();
            assert_eq!(
                data.len(),
                64,
                "InteropFeeUpdated should encode oldFee and newFee in data"
            );
            (
                U256::from_be_slice(&data[..32]),
                U256::from_be_slice(&data[32..64]),
            )
        }
        // old/new fee are indexed event fields.
        3 => (
            U256::from_be_slice(topics[1].as_slice()),
            U256::from_be_slice(topics[2].as_slice()),
        ),
        _ => panic!("unexpected InteropFeeUpdated topic layout"),
    };

    assert_eq!(old_fee, expected_old_fee);
    assert_eq!(new_fee, expected_new_fee);
}

fn simple_tx(nonce: u64) -> ZKsyncTxEnvelope {
    let wallet = testing_signer(0);
    let tx = TxLegacy {
        chain_id: 37u64.into(),
        nonce,
        gas_price: 1000,
        gas_limit: 21_000,
        to: TxKind::Call(address!("4200000000000000000000000000000000000000")),
        value: Default::default(),
        input: Default::default(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
}

fn assert_single_tx_succeeded(output: &BlockOutput) {
    assert_eq!(output.tx_results.len(), 1);
    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("transaction should pass validation");
    assert!(tx_result.is_success(), "transaction should be successful");
}

fn assert_single_successful_call(output: &BlockOutput, expected_logs: usize) {
    assert_eq!(output.tx_results.len(), 1);
    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("transaction should pass validation");
    assert!(tx_result.is_success(), "transaction should be successful");
    match &tx_result.execution_result {
        ExecutionResult::Success(ExecutionOutput::Call(_)) => {}
        _ => panic!("execution result must be a successful call"),
    }
    assert_eq!(tx_result.logs.len(), expected_logs);
}

fn run_interop_roots_test_inner(
    interop_roots: Vec<StoredInteropRoot>,
) -> (TestingFramework, BlockOutput) {
    let mut tester = with_interop_root_storage_contract(TestingFramework::new())
        .with_next_block_number(1)
        .with_block_context(service_block_context());
    let output = tester.execute_block(vec![interop_root_import_tx(interop_roots, 0)]);
    (tester, output)
}

#[test]
fn run_processes_one_interop_root() {
    let interop_roots = vec![StoredInteropRoot {
        root: Bytes32::from_u256_be(&U256::ONE),
        block_or_batch_number: U256::from(42),
        chain_id: U256::ONE,
    }];

    let (mut tester, output) = run_interop_roots_test_inner(interop_roots);
    assert_single_successful_call(&output, 1);
    assert_eq!(
        read_interop_root_slot(&mut tester, U256::ONE, U256::from(42)),
        Some(Bytes32::from_u256_be(&U256::ONE))
    );
    // TODO(EVM-1227): when batch commitment extraction from prover input is exposed in
    // `TestingFramework`, assert interop_roots_rolling_hash for this block.
}

#[test]
fn run_fails_if_interop_root_is_incorrect() {
    let interop_roots = vec![StoredInteropRoot {
        root: Bytes32::zero(), // Root cannot be zero.
        block_or_batch_number: U256::from(42),
        chain_id: U256::ONE,
    }];

    let (mut tester, output) = run_interop_roots_test_inner(interop_roots);
    assert_eq!(output.tx_results.len(), 1);
    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("transaction should pass validation");
    assert!(
        !tx_result.is_success(),
        "transaction with zero interop root must fail"
    );
    assert_eq!(
        read_interop_root_slot(&mut tester, U256::ONE, U256::from(42)),
        None
    );
}

#[test]
fn run_processes_several_interop_roots() {
    let mut interop_roots = Vec::new();
    for i in 1..=20 {
        interop_roots.push(StoredInteropRoot {
            root: Bytes32::from_u256_be(&U256::from(0x1000 + i)),
            block_or_batch_number: U256::from(100 + i),
            chain_id: U256::from(i),
        });
    }

    let (mut tester, output) = run_interop_roots_test_inner(interop_roots.clone());
    assert_single_successful_call(&output, 20);
    for root in interop_roots {
        assert_eq!(
            read_interop_root_slot(&mut tester, root.chain_id, root.block_or_batch_number),
            Some(root.root)
        );
    }
    // TODO(EVM-1227): when batch commitment extraction from prover input is exposed in
    // `TestingFramework`, assert interop_roots_rolling_hash for multi-root import.
}

#[test]
fn run_processes_empty_interop_roots() {
    let (mut tester, output) = run_interop_roots_test_inner(vec![]);
    assert_single_successful_call(&output, 0);
    assert_eq!(
        read_interop_root_slot(&mut tester, U256::ONE, U256::ONE),
        None
    );
    // TODO(EVM-1227): once batch output is available in tests, assert that
    // interop_roots_rolling_hash remains unchanged for empty input.
}

#[test]
fn run_processes_interop_roots_max_amount() {
    let interop_roots = vec![
        StoredInteropRoot {
            root: Bytes32::from_u256_be(&U256::MAX),
            block_or_batch_number: U256::MAX,
            chain_id: U256::MAX,
        },
        StoredInteropRoot {
            root: Bytes32::from_u256_be(&U256::ONE),
            block_or_batch_number: U256::ZERO,
            chain_id: U256::ONE,
        },
        StoredInteropRoot {
            root: Bytes32::from_u256_be(&(U256::MAX - U256::ONE)),
            block_or_batch_number: U256::ONE,
            chain_id: U256::ONE,
        },
    ];

    let (mut tester, output) = run_interop_roots_test_inner(interop_roots.clone());
    assert_single_successful_call(&output, 3);
    for root in interop_roots {
        assert_eq!(
            read_interop_root_slot(&mut tester, root.chain_id, root.block_or_batch_number),
            Some(root.root)
        );
    }
    // TODO(EVM-1227): once batch output is exposed in the testing framework, verify
    // rolling-hash accumulation with max-valued roots.
}

#[test]
fn test_new_sl_chain_id_no_update() {
    let sl_chain_id = 1u64;
    let wallet = testing_signer(0);
    let mut tester = with_system_context_contracts(TestingFramework::new())
        .with_storage_slot(
            b160_to_address(SYSTEM_CONTEXT_ADDRESS),
            U256::ZERO,
            B256::from_limbs([sl_chain_id, 0, 0, 0]),
        )
        .with_prefunded_account(wallet.address());

    let output = tester.execute_block(vec![simple_tx(0)]);
    assert_single_tx_succeeded(&output);
    assert_eq!(read_sl_chain_id_slot(&mut tester), U256::from(sl_chain_id));
}

#[test]
fn test_new_sl_chain_id_one_update() {
    let old_sl_chain_id = 1u64;
    let new_sl_chain_id = U256::from(42);
    let mut tester = with_system_context_contracts(TestingFramework::new())
        .with_storage_slot(
            b160_to_address(SYSTEM_CONTEXT_ADDRESS),
            U256::ZERO,
            B256::from_limbs([old_sl_chain_id, 0, 0, 0]),
        )
        .with_block_context(service_block_context());

    let output = tester.execute_block(vec![set_sl_chain_id_tx(new_sl_chain_id, 0)]);
    assert_single_successful_call(&output, 1);
    assert_eq!(read_sl_chain_id_slot(&mut tester), new_sl_chain_id);
}

#[test]
fn test_new_sl_chain_id_two_updates_fail() {
    let old_sl_chain_id = 1u64;
    let mut tester = with_system_context_contracts(TestingFramework::new())
        .with_storage_slot(
            b160_to_address(SYSTEM_CONTEXT_ADDRESS),
            U256::ZERO,
            B256::from_limbs([old_sl_chain_id, 0, 0, 0]),
        )
        .with_block_context(service_block_context());
    let tx1 = set_sl_chain_id_tx(U256::from(43), 0);
    let tx2 = set_sl_chain_id_tx(U256::from(44), 1);
    tester
        .execute_block_no_panic(vec![tx1, tx2])
        .expect_err("block with two settlement layer chain id updates should fail");
    assert_eq!(
        read_sl_chain_id_slot(&mut tester),
        U256::from(old_sl_chain_id)
    );
}

#[test]
fn test_set_sl_chain_id_first_block_batch() {
    let wallet = testing_signer(0);
    let mut tester = with_system_context_contracts(TestingFramework::new())
        .with_prefunded_account(wallet.address());

    tester.set_block_context(Some(service_block_context()));
    let block1_output = tester.execute_block(vec![set_sl_chain_id_tx(U256::from(42), 0)]);
    assert_single_successful_call(&block1_output, 1);

    tester.set_block_context(None);
    let block2_output = tester.execute_block(vec![simple_tx(0)]);
    assert_single_tx_succeeded(&block2_output);
    assert_eq!(read_sl_chain_id_slot(&mut tester), U256::from(42));

    // TODO(EVM-1227): replace this forward-only smoke test with a real multiblock-batch
    // commitment check once prover-input batch commitment extraction is available.
}

#[test]
#[ignore = "TODO(EVM-1227): requires multiblock-batch commitment checks from prover input runs"]
fn test_set_sl_chain_id_not_first_block_batch_fails() {
    // TODO(EVM-1227): port this assertion once tests can compare batch-level commitment/state
    // between forward and proving paths for a multiblock batch.
}

#[test]
fn test_set_interop_fee_updates_slot_and_emits_event() {
    let mut tester = with_interop_center_contract(TestingFramework::new())
        .with_block_context(service_block_context());
    assert_eq!(read_interop_protocol_fee_slot(&mut tester), None);

    let new_fee = U256::from(123_456u64);
    let output = tester.execute_block(vec![set_interop_fee_tx(new_fee, 0)]);
    assert_single_successful_call(&output, 1);

    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("transaction should pass validation");
    assert_interop_fee_updated_log(&tx_result.logs, U256::ZERO, new_fee);
    assert_eq!(read_interop_protocol_fee_slot(&mut tester), Some(new_fee));
}

#[test]
fn test_set_interop_fee_two_updates_in_one_block() {
    let mut tester = with_interop_center_contract(TestingFramework::new())
        .with_block_context(service_block_context());
    let fee1 = U256::from(10u64);
    let fee2 = U256::from(42u64);

    let output = tester.execute_block(vec![
        set_interop_fee_tx(fee1, 0),
        set_interop_fee_tx(fee2, 1),
    ]);
    assert_eq!(output.tx_results.len(), 2);
    for tx_result in &output.tx_results {
        assert!(tx_result.as_ref().is_ok_and(|r| r.is_success()));
    }

    let tx0 = output.tx_results[0]
        .as_ref()
        .expect("first tx should pass validation");
    let tx1 = output.tx_results[1]
        .as_ref()
        .expect("second tx should pass validation");
    assert_interop_fee_updated_log(&tx0.logs, U256::ZERO, fee1);
    assert_interop_fee_updated_log(&tx1.logs, fee1, fee2);
    assert_eq!(read_interop_protocol_fee_slot(&mut tester), Some(fee2));
}
