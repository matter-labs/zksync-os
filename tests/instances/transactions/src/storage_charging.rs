use alloy::consensus::TxEip1559;
use alloy::primitives::TxKind;
use rig::alloy;
use rig::alloy::primitives::address;
use rig::basic_bootloader::bootloader::constants::L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST;
use rig::basic_system::system_implementation::flat_storage_model::cost_constants::{
    COLD_EXISTING_STORAGE_READ_NATIVE_COST, COLD_EXISTING_STORAGE_WRITE_EXTRA_NATIVE_COST,
    COLD_NEW_STORAGE_READ_NATIVE_COST, COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST,
    WARM_STORAGE_READ_NATIVE_COST, WARM_STORAGE_WRITE_EXTRA_NATIVE_COST,
};
use rig::evm_bytecode::BytecodeBuilder;
use rig::ruint::aliases::B256;
use rig::ruint::aliases::U256;
use rig::testing_signer;
use rig::{BlockContext, TestingFramework};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

/// Block context with a clean native_per_gas = basefee / native_price = 1000 / 10 = 100.
fn storage_test_block_context() -> BlockContext {
    BlockContext {
        native_price: U256::from(10),
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    }
}

/// Writing the same storage slot twice in a single tx should be cheaper than
/// writing two different slots, because the second write to an already-warm
/// slot only pays the warm price instead of a full cold new-slot price.
#[test]
fn test_repeated_storage_write_cheaper_than_cold() {
    let contract_a_addr = address!("0000000000000000000000000000000000020001");
    let contract_b_addr = address!("0000000000000000000000000000000000020002");

    // Contract A: write slot 0 twice (second write is warm).
    let bytecode_repeated = BytecodeBuilder::new()
        .push_u8(1)
        .push_u8(0)
        .sstore() // SSTORE(0, 1)
        .push_u8(2)
        .push_u8(0)
        .sstore() // SSTORE(0, 2) -- warm
        .return_empty()
        .finish();

    // Contract B: write two different slots (both cold).
    let bytecode_two_slots = BytecodeBuilder::new()
        .push_u8(1)
        .push_u8(0)
        .sstore() // SSTORE(0, 1)
        .push_u8(1)
        .push_u8(1)
        .sstore() // SSTORE(1, 1) -- cold
        .return_empty()
        .finish();

    let wallet_a = testing_signer(0);
    let wallet_b = testing_signer(1);
    let block_context = storage_test_block_context();

    // Run contract A (repeated writes to same slot).
    let mut tester_a = TestingFramework::new()
        .with_evm_contract(contract_a_addr, &bytecode_repeated)
        .with_balance(wallet_a.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context.clone());

    let tx_a = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(contract_a_addr),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet_a.clone())
    };
    let output_a = tester_a.execute_block(vec![tx_a]);
    let result_a = output_a.tx_results[0]
        .as_ref()
        .expect("Contract A tx should be processed");
    assert!(
        result_a.is_success(),
        "Contract A (repeated writes) should succeed, got: {:?}",
        output_a.tx_results[0]
    );
    let native_a = result_a.computational_native_used;

    // Run contract B (writes to two different slots).
    let mut tester_b = TestingFramework::new()
        .with_evm_contract(contract_b_addr, &bytecode_two_slots)
        .with_balance(wallet_b.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context);

    let tx_b = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(contract_b_addr),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet_b.clone())
    };
    let output_b = tester_b.execute_block(vec![tx_b]);
    let result_b = output_b.tx_results[0]
        .as_ref()
        .expect("Contract B tx should be processed");
    assert!(
        result_b.is_success(),
        "Contract B (two cold writes) should succeed, got: {:?}",
        output_b.tx_results[0]
    );
    let native_b = result_b.computational_native_used;

    // The second cold slot costs a full cold new read + cold new write extra,
    // while the warm repeat costs only warm read + warm write extra.
    let expected_min_savings = (COLD_NEW_STORAGE_READ_NATIVE_COST
        + COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST)
        - (WARM_STORAGE_READ_NATIVE_COST + WARM_STORAGE_WRITE_EXTRA_NATIVE_COST);

    assert!(
        native_a < native_b,
        "Repeated writes to the same slot (native={native_a}) should be cheaper \
         than two cold slot writes (native={native_b})"
    );
    let savings = native_b - native_a;
    assert!(
        savings >= expected_min_savings,
        "Expected at least {expected_min_savings} savings from warm write path, \
         but only saved {savings} native"
    );
}

/// A single SSTORE to a new slot should cost native resources in an expected
/// range. This catches gross under-charging or over-charging regressions.
#[test]
fn test_single_sstore_native_cost_in_expected_range() {
    let contract_addr = address!("0000000000000000000000000000000000030001");

    // Contract: single SSTORE(0, 1) then return.
    let bytecode = BytecodeBuilder::new()
        .push_u8(1)
        .push_u8(0)
        .sstore()
        .return_empty()
        .finish();

    let wallet = testing_signer(0);
    let block_context = storage_test_block_context();

    let mut tester = TestingFramework::new()
        .with_evm_contract(contract_addr, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context);

    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(contract_addr),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let output = tester.execute_block(vec![tx]);
    let result = output.tx_results[0]
        .as_ref()
        .expect("Single-SSTORE tx should be processed");
    assert!(
        result.is_success(),
        "Single-SSTORE tx should succeed, got: {:?}",
        output.tx_results[0]
    );

    let native_used = result.computational_native_used;

    // Lower bound: intrinsic cost + the SSTORE's cold new read + cold new write extra.
    // This is conservative — actual also includes contract account access, EVM overhead, etc.
    let min_expected = L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST
        + COLD_NEW_STORAGE_READ_NATIVE_COST
        + COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST;

    // Upper bound: the lower bound covers the dominant costs; execution overhead
    // (contract cold read, EVM dispatch, CALL frame) should not double the total.
    let max_expected = min_expected * 2;

    assert!(
        native_used >= min_expected && native_used <= max_expected,
        "Single-SSTORE computational_native_used={native_used} is outside expected range \
         [{min_expected}, {max_expected}]"
    );
}

/// A transfer to a new account (A -> B) should cost more native than a
/// self-transfer (A -> A) because the new recipient B requires a cold account
/// read plus a persist charge for writing its account properties to the tree.
#[test]
fn test_multi_account_persist_charges() {
    let wallet = testing_signer(0);
    let recipient = address!("0000000000000000000000000000000000040001");
    let block_context = storage_test_block_context();

    // TX 1: Transfer ETH from wallet to a fresh recipient (touches 2 distinct accounts).
    let mut tester_two_accounts = TestingFramework::new()
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context.clone());

    let tx_to_other = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(recipient),
            value: U256::from(100),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let output_two = tester_two_accounts.execute_block(vec![tx_to_other]);
    let result_two = output_two.tx_results[0]
        .as_ref()
        .expect("Transfer-to-other tx should be processed");
    assert!(
        result_two.is_success(),
        "Transfer to new account should succeed, got: {:?}",
        output_two.tx_results[0]
    );
    let native_two_accounts = result_two.computational_native_used;

    // TX 2: Self-transfer (wallet -> wallet). Only 1 distinct account is touched
    // (sender == recipient, no extra persist charge for a new account).
    let mut tester_self = TestingFramework::new()
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context);

    let tx_to_self = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(wallet.address()),
            value: U256::from(100),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let output_self = tester_self.execute_block(vec![tx_to_self]);
    let result_self = output_self.tx_results[0]
        .as_ref()
        .expect("Self-transfer tx should be processed");
    assert!(
        result_self.is_success(),
        "Self-transfer should succeed, got: {:?}",
        output_self.tx_results[0]
    );
    let native_self = result_self.computational_native_used;

    // Transfer to a new account should cost more due to cold account read +
    // persist charge. The persist write alone is at least one merkle path.
    let expected_min_diff =
        COLD_NEW_STORAGE_READ_NATIVE_COST + COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST;

    assert!(
        native_two_accounts > native_self,
        "Transfer to new account (native={native_two_accounts}) should cost more \
         than self-transfer (native={native_self}) due to recipient persist charges"
    );
    let diff = native_two_accounts - native_self;
    assert!(
        diff >= expected_min_diff,
        "Expected at least {expected_min_diff} cost difference for new account \
         (cold read + persist), but diff was only {diff} native"
    );
}

/// Writing to a pre-existing storage slot should be cheaper than writing to a
/// new slot. Existing slots cost 1 merkle path for both read and write extra,
/// while new slots cost 2 paths for read and 3 for write extra.
#[test]
fn test_existing_slot_write_cheaper_than_new_slot() {
    let contract_addr = address!("0000000000000000000000000000000000050001");

    // Contract: SSTORE(0, 42) — writes value 42 to slot 0.
    let bytecode = BytecodeBuilder::new()
        .push_u8(42)
        .push_u8(0)
        .sstore()
        .return_empty()
        .finish();

    let wallet_a = testing_signer(0);
    let wallet_b = testing_signer(1);
    let block_context = storage_test_block_context();

    // Case A: slot 0 is pre-populated (existing slot write).
    let mut tester_existing = TestingFramework::new()
        .with_evm_contract(contract_addr, &bytecode)
        .with_storage_slot(contract_addr, U256::from(0), B256::from(U256::from(1)))
        .with_balance(wallet_a.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context.clone());

    let tx_existing = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(contract_addr),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet_a.clone())
    };
    let output_existing = tester_existing.execute_block(vec![tx_existing]);
    let result_existing = output_existing.tx_results[0]
        .as_ref()
        .expect("Existing-slot tx should be processed");
    assert!(
        result_existing.is_success(),
        "Existing-slot write should succeed, got: {:?}",
        output_existing.tx_results[0]
    );
    let native_existing = result_existing.computational_native_used;

    // Case B: slot 0 is empty (new slot write). Use a different contract address
    // to ensure no pre-populated state, and a different wallet for a clean nonce.
    let contract_b_addr = address!("0000000000000000000000000000000000050002");
    let mut tester_new = TestingFramework::new()
        .with_evm_contract(contract_b_addr, &bytecode)
        .with_balance(wallet_b.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context);

    let tx_new = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(contract_b_addr),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet_b.clone())
    };
    let output_new = tester_new.execute_block(vec![tx_new]);
    let result_new = output_new.tx_results[0]
        .as_ref()
        .expect("New-slot tx should be processed");
    assert!(
        result_new.is_success(),
        "New-slot write should succeed, got: {:?}",
        output_new.tx_results[0]
    );
    let native_new = result_new.computational_native_used;

    // New slot costs 2 read paths + 3 write-extra paths = 5 total.
    // Existing slot costs 1 read path + 1 write-extra path = 2 total.
    // Difference: 3 merkle paths.
    let expected_min_savings = (COLD_NEW_STORAGE_READ_NATIVE_COST
        + COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST)
        - (COLD_EXISTING_STORAGE_READ_NATIVE_COST + COLD_EXISTING_STORAGE_WRITE_EXTRA_NATIVE_COST);

    assert!(
        native_existing < native_new,
        "Existing-slot write (native={native_existing}) should be cheaper \
         than new-slot write (native={native_new})"
    );
    let savings = native_new - native_existing;
    assert!(
        savings >= expected_min_savings,
        "Expected at least {expected_min_savings} savings (3 merkle paths) for \
         existing vs new slot, but only saved {savings} native"
    );
}

/// SLOAD followed by SSTORE to the same slot should cost at least as much
/// native as a standalone SSTORE (without prior SLOAD). The SLOAD warms the
/// EVM access but must NOT discount the native write extra — the tree still
/// needs the full write merkle paths.
#[test]
fn test_sload_then_sstore_charges_cold_write_extra() {
    let contract_a_addr = address!("0000000000000000000000000000000000060001");
    let contract_b_addr = address!("0000000000000000000000000000000000060002");

    // Contract A: SLOAD slot 0, then SSTORE slot 0.
    let bytecode_sload_sstore = BytecodeBuilder::new()
        .push_u8(0)
        .sload() // SLOAD(0) — warms access but does NOT pay write extra
        .pop()
        .push_u8(42)
        .push_u8(0)
        .sstore() // SSTORE(0, 42) — must pay cold write extra
        .return_empty()
        .finish();

    // Contract B: standalone SSTORE slot 0 (no prior read).
    let bytecode_sstore_only = BytecodeBuilder::new()
        .push_u8(42)
        .push_u8(0)
        .sstore() // SSTORE(0, 42) — cold access + cold write extra
        .return_empty()
        .finish();

    let wallet_a = testing_signer(0);
    let wallet_b = testing_signer(1);
    let block_context = storage_test_block_context();

    // Run contract A (SLOAD→SSTORE).
    let mut tester_a = TestingFramework::new()
        .with_evm_contract(contract_a_addr, &bytecode_sload_sstore)
        .with_balance(wallet_a.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context.clone());

    let tx_a = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(contract_a_addr),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet_a.clone())
    };
    let output_a = tester_a.execute_block(vec![tx_a]);
    let result_a = output_a.tx_results[0]
        .as_ref()
        .expect("SLOAD→SSTORE tx should be processed");
    assert!(
        result_a.is_success(),
        "SLOAD→SSTORE should succeed, got: {:?}",
        output_a.tx_results[0]
    );
    let native_a = result_a.computational_native_used;

    // Run contract B (standalone SSTORE).
    let mut tester_b = TestingFramework::new()
        .with_evm_contract(contract_b_addr, &bytecode_sstore_only)
        .with_balance(wallet_b.address(), U256::from(1_000_000_000_000_000_u64))
        .with_block_context(block_context);

    let tx_b = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 200_000,
            to: TxKind::Call(contract_b_addr),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet_b.clone())
    };
    let output_b = tester_b.execute_block(vec![tx_b]);
    let result_b = output_b.tx_results[0]
        .as_ref()
        .expect("Standalone SSTORE tx should be processed");
    assert!(
        result_b.is_success(),
        "Standalone SSTORE should succeed, got: {:?}",
        output_b.tx_results[0]
    );
    let native_b = result_b.computational_native_used;

    // SLOAD→SSTORE should cost at least as much native as standalone SSTORE.
    // The SLOAD adds a warm read cost but the SSTORE still pays cold write extra.
    // If the SLOAD incorrectly discounted the write, native_a would be less than native_b.
    assert!(
        native_a >= native_b,
        "SLOAD→SSTORE (native={native_a}) should cost at least as much as \
         standalone SSTORE (native={native_b}) — SLOAD must not discount write extra"
    );
}
