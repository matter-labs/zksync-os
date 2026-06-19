use rig::alloy::consensus::TxLegacy;
use rig::alloy::primitives::{address, Address, TxKind, B256};
use rig::basic_bootloader::bootloader::block_flow::eip_2935_historical_block_hash::HISTORY_STORAGE_ADDRESS;
use rig::ruint::aliases::{B256 as RuintB256, U256};
use rig::zk_ee::system::EIP7702_DELEGATION_MARKER;
use rig::zksync_os_interface::types::{ExecutionOutput, ExecutionResult};
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{testing_signer, TestingFramework};

const HISTORY_GETTER_BYTECODE: &str = "6000355460005260206000f3";
const DEFAULT_TEST_BALANCE: u64 = 1_000_000_000_000_000;

fn history_storage_address() -> Address {
    Address::from_slice(&HISTORY_STORAGE_ADDRESS.to_be_bytes::<20>())
}

fn slot_key(slot_idx: u64) -> U256 {
    U256::from(slot_idx)
}

fn slot_value(slot_idx: u64) -> B256 {
    B256::from(slot_key(slot_idx).to_be_bytes::<32>())
}

fn setup_word(value: B256) -> RuintB256 {
    RuintB256::from_be_bytes(value.0)
}

fn parent_hash_window(parent_hash: B256) -> [U256; 256] {
    let mut hashes = [U256::ZERO; 256];
    hashes[255] = U256::from_be_bytes(parent_hash.0);
    hashes
}

fn history_getter_tester() -> TestingFramework {
    let signer = testing_signer(0);

    TestingFramework::new()
        .with_balance(signer.address(), U256::from(DEFAULT_TEST_BALANCE))
        .with_evm_contract(
            history_storage_address(),
            &rig::alloy::hex::decode(HISTORY_GETTER_BYTECODE).expect("valid getter bytecode"),
        )
}

fn history_getter_tx(slot_idx: u64, nonce: u64) -> ZKsyncTxEnvelope {
    let signer = testing_signer(0);

    ZKsyncTxEnvelope::from_eth_tx(
        TxLegacy {
            chain_id: 37u64.into(),
            nonce,
            gas_price: 1000,
            gas_limit: 100_000,
            to: TxKind::Call(history_storage_address()),
            value: Default::default(),
            input: slot_key(slot_idx).to_be_bytes_vec().into(),
        },
        signer,
    )
}

fn read_call_output_bytes(output: &rig::BlockOutput) -> Vec<u8> {
    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("history getter tx must execute");
    assert!(tx_result.is_success(), "history getter tx must succeed");

    match &tx_result.execution_result {
        ExecutionResult::Success(ExecutionOutput::Call(bytes)) => bytes.clone(),
        other => panic!("history getter tx must return from a successful call, got {other:?}"),
    }
}

fn assert_history_slot(tester: &mut TestingFramework, slot_idx: u64, expected: B256) {
    let actual = tester
        .get_storage_slot(&history_storage_address(), slot_key(slot_idx))
        .expect("history slot must exist");
    assert_eq!(
        actual.as_u8_array(),
        expected.0,
        "unexpected value in history slot {slot_idx}"
    );
}

#[test]
fn test_eip2935_historical_block_hash_happy_path() {
    let sentinel_slot0 = B256::from([0x11; 32]);

    let mut block1_tester = history_getter_tester()
        .with_storage_slot(
            history_storage_address(),
            U256::ZERO,
            setup_word(sentinel_slot0),
        )
        .with_next_block_number(1);
    let block1_output = block1_tester.execute_block(vec![history_getter_tx(0, 0)]);

    assert_eq!(
        read_call_output_bytes(&block1_output),
        B256::ZERO.0.to_vec(),
        "tx must observe the parent hash already written in the pre-tx loop"
    );
    assert_history_slot(&mut block1_tester, 0, B256::ZERO);

    let wrap_parent_hash = B256::from([0x22; 32]);
    let mut wrap_tester = history_getter_tester()
        .with_storage_slot(
            history_storage_address(),
            U256::ZERO,
            setup_word(sentinel_slot0),
        )
        .with_next_block_number(8192)
        .with_block_hashes(parent_hash_window(wrap_parent_hash));
    let wrap_output = wrap_tester.execute_block(vec![]);

    assert!(
        wrap_output.storage_writes.iter().any(|write| {
            write.account == history_storage_address()
                && write.account_key == B256::ZERO
                && write.value == wrap_parent_hash
        }),
        "block 8192 must wrap around and overwrite history slot 0"
    );
    assert_history_slot(&mut wrap_tester, 0, wrap_parent_hash);

    let preserved_slot0 = B256::from([0x33; 32]);
    let parent_hash_99 = B256::from([0x44; 32]);
    let mut slot_index_tester = history_getter_tester()
        .with_storage_slot(
            history_storage_address(),
            U256::ZERO,
            setup_word(preserved_slot0),
        )
        .with_storage_slot(
            history_storage_address(),
            slot_key(99),
            setup_word(B256::from([0x55; 32])),
        )
        .with_next_block_number(100)
        .with_block_hashes(parent_hash_window(parent_hash_99));
    let slot_index_output = slot_index_tester.execute_block(vec![]);

    assert!(
        slot_index_output.storage_writes.iter().any(|write| {
            write.account == history_storage_address()
                && write.account_key == slot_value(99)
                && write.value == parent_hash_99
        }),
        "block 100 must write the parent hash into history slot 99"
    );
    assert!(
        !slot_index_output.storage_writes.iter().any(|write| {
            write.account == history_storage_address() && write.account_key == B256::ZERO
        }),
        "block 100 must not touch history slot 0"
    );
    assert_history_slot(&mut slot_index_tester, 99, parent_hash_99);
    assert_history_slot(&mut slot_index_tester, 0, preserved_slot0);
}

#[test]
fn test_eip2935_historical_block_hash_is_noop_without_history_contract() {
    let expected_parent_hash = B256::from([0x66; 32]);
    let mut tester = TestingFramework::new()
        .with_next_block_number(100)
        .with_block_hashes(parent_hash_window(expected_parent_hash));
    let output = tester.execute_block(vec![]);

    assert!(
        output
            .storage_writes
            .iter()
            .all(|write| write.account != history_storage_address()),
        "missing history contract must skip the EIP-2935 write"
    );
    assert_eq!(
        tester.get_storage_slot(&history_storage_address(), U256::ZERO),
        None,
        "missing history contract must leave storage untouched"
    );
}

#[test]
fn test_eip2935_historical_block_hash_skips_delegated_account() {
    let delegated_history_target = address!("1111111111111111111111111111111111111111");
    let mut delegation_bytecode = EIP7702_DELEGATION_MARKER.to_vec();
    delegation_bytecode.extend_from_slice(delegated_history_target.as_slice());

    let sentinel_slot0 = B256::from([0x77; 32]);
    let expected_parent_hash = B256::from([0x88; 32]);

    let mut tester = TestingFramework::new()
        .with_evm_contract(history_storage_address(), &delegation_bytecode)
        .with_storage_slot(
            history_storage_address(),
            U256::ZERO,
            setup_word(sentinel_slot0),
        )
        .with_next_block_number(100)
        .with_block_hashes(parent_hash_window(expected_parent_hash));
    let output = tester.execute_block(vec![]);

    let props = tester.get_account_properties(&history_storage_address());
    assert!(
        props.versioning_data.is_delegated(),
        "test setup must use an EIP-7702 delegated account"
    );
    assert!(
        output
            .storage_writes
            .iter()
            .all(|write| write.account != history_storage_address()),
        "delegated history account must skip the EIP-2935 write"
    );
    assert_history_slot(&mut tester, 0, sentinel_slot0);
}
