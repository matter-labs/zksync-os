use crate::TxEip1559;
use crate::ERC_20_TRANSFER_CALLDATA;
use rig::alloy;
use rig::alloy::primitives::{address, TxKind};
use rig::ruint::aliases::{B256, U256};
use rig::testing_signer;
use rig::utils::L1TxBuilder;
use rig::{BlockContext, TestingFramework};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

const TO: alloy::primitives::Address = address!("0000000000000000000000000000000000010002");

const AVG_RATIO: u64 = 150;
const LOW_RATIO: u64 = 1;
const HIGH_RATIO: u64 = 1_000_000;

fn run_tx(
    tx: ZKsyncTxEnvelope,
    basefee: u64,
    native_price: u64,
    should_succeed: bool,
    simulation: bool,
) {
    let bytecode = hex::decode(crate::ERC_20_BYTECODE).unwrap();
    let wallet = testing_signer(0);
    let key = crate::compute_erc20_balance_slot(wallet.address());
    let value = B256::from(U256::from(1_000_000_000_000_000_u64));

    let block_context = BlockContext {
        native_price: U256::from(native_price),
        eip1559_basefee: U256::from(basefee),
        ..Default::default()
    };

    let mut tester = TestingFramework::new()
        .with_evm_contract(TO, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_storage_slot(TO, key, value)
        .with_block_context(block_context);

    let output = if simulation {
        tester.simulate_block(vec![tx])
    } else {
        tester.execute_block(vec![tx])
    };

    // Assert all txs succeeded/failed as expected.
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}")
        }
        should_succeed == success
    }))
}

// Test with a low cycles/gas ratio, should fail
#[test]
fn test_l1_tx_low_ratio() {
    let wallet = testing_signer(0);
    // L1 Txs have a hard-coded native price of 10
    let native_price = 10;
    let gas_price = native_price * LOW_RATIO;
    let tx = L1TxBuilder::new()
        .from(wallet.address())
        .to(TO)
        .gas_price(gas_price.into())
        .gas_limit(70_000)
        .input(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap())
        .build()
        .into();
    run_tx(tx, gas_price, native_price, false, false)
}

// Test with a avg cycles/gas ratio, should succeed
#[test]
fn test_l1_tx_avg_ratio() {
    let wallet = testing_signer(0);
    // L1 Txs have a hard-coded native price of 10
    let native_price = 10;
    let gas_price = native_price * AVG_RATIO;
    let tx = L1TxBuilder::new()
        .from(wallet.address())
        .to(TO)
        .gas_price(gas_price.into())
        .gas_limit(70_000)
        .input(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap())
        .build()
        .into();
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with a high cycles/gas ratio, should succeed
#[test]
fn test_l1_tx_high_ratio() {
    let wallet = testing_signer(0);
    // L1 Txs have a hard-coded native price of 10
    let native_price = 10;
    let gas_price = native_price * HIGH_RATIO;
    let tx = L1TxBuilder::new()
        .from(wallet.address())
        .to(TO)
        .gas_price(gas_price.into())
        .gas_limit(70_000)
        .input(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap())
        .build()
        .into();
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with a low cycles/gas ratio, should fail
#[test]
fn test_l2_tx_low_ratio() {
    let wallet = testing_signer(0);
    let native_price = 100;
    // This ratio passes validation but runs out of native during execution.
    let gas_price = native_price * 20u64;
    let gas_limit = 60_000;
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: gas_price.into(),
            max_priority_fee_per_gas: gas_price.into(),
            gas_limit,
            to: TxKind::Call(TO),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let bytecode = hex::decode(crate::ERC_20_BYTECODE).unwrap();
    let key = crate::compute_erc20_balance_slot(wallet.address());
    let value = B256::from(U256::from(1_000_000_000_000_000_u64));
    let block_context = BlockContext {
        native_price: U256::from(native_price),
        eip1559_basefee: U256::from(gas_price),
        ..Default::default()
    };

    let mut tester = TestingFramework::new()
        .with_evm_contract(TO, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_storage_slot(TO, key, value)
        .with_block_context(block_context);

    let output = tester.execute_block(vec![tx]);
    let tx_result = output
        .tx_results
        .first()
        .expect("Must have a tx result")
        .as_ref()
        .expect("Tx should be processed as a top-level revert");

    assert!(
        !tx_result.is_success(),
        "Low native-per-gas tx should revert by running out of native"
    );
    assert_eq!(
        tx_result.gas_used, gas_limit,
        "Out-of-native tx must consume full gas limit"
    );
}

#[test]
fn test_l2_tx_not_enough_native_for_pubdata_uses_full_gas_limit() {
    let wallet = testing_signer(0);
    let from = wallet.address();
    let gas_limit = 250_000;
    let bytecode = hex::decode(
        "602a600052600160005560016001556001600255600160035560016004556001600555600160065560016007556001600855600160095560206000f3",
    )
    .unwrap();

    let make_tx = || {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit,
            to: TxKind::Call(TO),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    // Control execution should succeed, so the failing case below is specific to
    // post-execution pubdata charging.
    let control_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        native_price: U256::ONE,
        pubdata_price: U256::ONE,
        ..Default::default()
    };
    let mut control_tester = TestingFramework::new()
        .with_evm_contract(TO, &bytecode)
        .with_balance(from, U256::from(1_000_000_000_000_000_u64))
        .with_block_context(control_context);
    let control_output = control_tester.execute_block(vec![make_tx()]);
    let control_tx = control_output.tx_results[0]
        .as_ref()
        .expect("Control tx should be processed");
    assert!(
        control_tx.is_success(),
        "Control tx must succeed with regular pubdata pricing"
    );

    // Expensive pubdata causes a post-execution revert due to insufficient native.
    let expensive_pubdata_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        native_price: U256::ONE,
        pubdata_price: U256::from(700_000u64),
        ..Default::default()
    };
    let mut tester = TestingFramework::new()
        .with_evm_contract(TO, &bytecode)
        .with_balance(from, U256::from(1_000_000_000_000_000_u64))
        .with_block_context(expensive_pubdata_context);
    let output = tester.execute_block(vec![make_tx()]);
    let tx_result = output.tx_results[0]
        .as_ref()
        .expect("Tx should be processed even when reverted");

    assert!(
        !tx_result.is_success(),
        "Tx should revert when pubdata cannot be paid after execution"
    );
    assert_eq!(
        tx_result.gas_used, gas_limit,
        "Tx reverted by post-execution pubdata charging must consume full gas limit"
    );
}

// Test with a avg cycles/gas ratio, should succeed
#[test]
fn test_l2_tx_avg_ratio() {
    let wallet = testing_signer(0);
    let native_price = 100;
    let gas_price = native_price * AVG_RATIO;
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: gas_price.into(),
            max_priority_fee_per_gas: gas_price.into(),
            gas_limit: 60_000,
            to: TxKind::Call(TO),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
    };
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with a high cycles/gas ratio, should succeed
#[test]
fn test_l2_tx_high_ratio() {
    let wallet = testing_signer(0);
    let native_price = 100;
    let gas_price = native_price * HIGH_RATIO;
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: gas_price.into(),
            max_priority_fee_per_gas: gas_price.into(),
            gas_limit: 60_000,
            to: TxKind::Call(TO),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
    };
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with 0 gas limit, both l1 and l2 txs.
// Also call as simulation to skip validation step.
#[test]
fn test_0_gas_limit() {
    let wallet = testing_signer(0);
    let native_price = 10;
    let gas_price = native_price * AVG_RATIO;
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: gas_price.into(),
            max_priority_fee_per_gas: gas_price.into(),
            gas_limit: 0,
            to: TxKind::Call(TO),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    run_tx(tx.clone(), gas_price, native_price, false, false);
    run_tx(tx, gas_price, native_price, false, true);

    let tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(wallet.address())
        .to(TO)
        .gas_price(gas_price.into())
        .gas_limit(0)
        .input(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap())
        .build()
        .into();
    run_tx(tx.clone(), gas_price, native_price, false, false);
    run_tx(tx, gas_price, native_price, false, true);
}

// Test with 0 gas price, both l1 and l2 txs.
// Also call as simulation to skip validation step.
#[test]
fn test_0_gas_price() {
    let wallet = testing_signer(0);
    let native_price = 10;
    let gas_price = 0;
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: gas_price.into(),
            max_priority_fee_per_gas: gas_price.into(),
            gas_limit: 70_000,
            to: TxKind::Call(TO),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    run_tx(tx.clone(), gas_price, native_price, true, false);
    run_tx(tx, gas_price, native_price, true, true);

    let tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(wallet.address())
        .to(TO)
        .gas_price(gas_price.into())
        .gas_limit(70_000)
        .input(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap())
        .build()
        .into();
    run_tx(tx.clone(), gas_price, native_price, true, false)
}

// Test delta gas, pass lower ratio
#[test]
fn test_delta_gas() {
    let wallet = testing_signer(0);
    let native_price = 100;
    // Low enough that tx will fail without priority fee
    let ratio = 20;
    let gas_price = native_price * ratio;
    // First tx, no priority fee, should fail
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: gas_price.into(),
            max_priority_fee_per_gas: gas_price.into(),
            gas_limit: 60_000,
            to: TxKind::Call(TO),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    // Should fail
    run_tx(tx, gas_price, native_price, false, false);
    // Second tx, high priority fee, should succeed
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: (5 * gas_price).into(),
            max_priority_fee_per_gas: (5 * gas_price).into(),
            gas_limit: 60_000,
            to: TxKind::Call(TO),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    // Should succeed
    run_tx(tx, gas_price, native_price, true, false)
}
