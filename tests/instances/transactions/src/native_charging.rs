use crate::TxEip1559;
use crate::ERC_20_TRANSFER_CALLDATA;
use alloy::primitives::TxKind;
use alloy::signers::local::PrivateKeySigner;
use rig::alloy::primitives::address;
use rig::alloy::rpc::types::TransactionRequest;
use rig::ruint::aliases::{B160, U256};
use rig::{alloy, zksync_web3_rs, BlockContext, Chain};
use std::str::FromStr;
use zksync_web3_rs::signers::{LocalWallet, Signer};

const WALLET: &str = "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7";
const TO: alloy::primitives::Address = address!("0000000000000000000000000000000000010002");

const AVG_RATIO: u64 = 150;
const LOW_RATIO: u64 = 1;
const HIGH_RATIO: u64 = 1_000_000;

fn run_tx(tx: Vec<u8>, basefee: u64, native_price: u64, should_succeed: bool, simulation: bool) {
    let mut chain = Chain::empty(None);

    let transactions = vec![tx];

    let bytecode = hex::decode(crate::ERC_20_BYTECODE).unwrap();
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(TO.into_array()), &bytecode);
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();
    let from = wallet_ethers.address();

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );
    let key = crate::compute_erc20_balance_slot(wallet.address());
    let value = rig::ruint::aliases::B256::from(U256::from(1_000_000_000_000_000_u64));
    chain.set_storage_slot(B160::from_be_bytes(TO.0 .0), key, value);

    let block_context = BlockContext {
        native_price: U256::from(native_price),
        eip1559_basefee: U256::from(basefee),
        ..Default::default()
    };
    let output = if simulation {
        chain.simulate_block(transactions, Some(block_context))
    } else {
        chain.run_block(transactions, Some(block_context), None)
    };

    // Assert all txs succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {} failed with: {:?}", i, r)
        }
        should_succeed == success
    }))
}

// Test with a low cycles/gas ratio, should fail
#[test]
fn test_l1_tx_low_ratio() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
    // L1 Txs have a hard-coded native price of 10
    let native_price = 10;
    let gas_price = native_price * LOW_RATIO;
    let tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(alloy::signers::Signer::address(&wallet)),
            to: Some(TxKind::Call(TO)),
            gas: Some(70_000),
            max_fee_per_gas: Some(gas_price.into()),
            max_priority_fee_per_gas: Some(gas_price.into()),
            nonce: Some(0),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };
    run_tx(tx, gas_price, native_price, false, false)
}

// Test with a avg cycles/gas ratio, should succeed
#[test]
fn test_l1_tx_avg_ratio() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
    // L1 Txs have a hard-coded native price of 10
    let native_price = 10;
    let gas_price = native_price * AVG_RATIO;
    let tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(alloy::signers::Signer::address(&wallet)),
            to: Some(TxKind::Call(TO)),
            gas: Some(70_000),
            max_fee_per_gas: Some(gas_price.into()),
            max_priority_fee_per_gas: Some(gas_price.into()),
            nonce: Some(0),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with a high cycles/gas ratio, should succeed
#[test]
fn test_l1_tx_high_ratio() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
    // L1 Txs have a hard-coded native price of 10
    let native_price = 10;
    let gas_price = native_price * HIGH_RATIO;
    let tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(alloy::signers::Signer::address(&wallet)),
            to: Some(TxKind::Call(TO)),
            gas: Some(70_000),
            max_fee_per_gas: Some(gas_price.into()),
            max_priority_fee_per_gas: Some(gas_price.into()),
            nonce: Some(0),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with a low cycles/gas ratio, should fail
#[test]
fn test_l2_tx_low_ratio() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
    let native_price = 100;
    let gas_price = 100 * LOW_RATIO;
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };
    run_tx(tx, gas_price, native_price, false, false)
}

// Test with a avg cycles/gas ratio, should succeed
#[test]
fn test_l2_tx_avg_ratio() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with a high cycles/gas ratio, should succeed
#[test]
fn test_l2_tx_high_ratio() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };
    run_tx(tx, gas_price, native_price, true, false)
}

// Test with 0 gas limit, both l1 and l2 txs.
// Also call as simulation to skip validation step.
#[test]
fn test_0_gas_limit() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };
    run_tx(tx.clone(), gas_price, native_price, false, false);
    run_tx(tx, gas_price, native_price, false, true);

    let tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(alloy::signers::Signer::address(&wallet)),
            to: Some(TxKind::Call(TO)),
            gas: Some(0),
            max_fee_per_gas: Some(gas_price.into()),
            max_priority_fee_per_gas: Some(gas_price.into()),
            nonce: Some(0),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };
    run_tx(tx.clone(), gas_price, native_price, false, false);
    run_tx(tx, gas_price, native_price, false, true);
}

// Test with 0 gas price, both l1 and l2 txs.
// Also call as simulation to skip validation step.
#[test]
fn test_0_gas_price() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };
    run_tx(tx.clone(), gas_price, native_price, true, false);
    run_tx(tx, gas_price, native_price, true, true);

    let tx = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(alloy::signers::Signer::address(&wallet)),
            to: Some(TxKind::Call(TO)),
            gas: Some(70_000),
            max_fee_per_gas: Some(gas_price.into()),
            max_priority_fee_per_gas: Some(gas_price.into()),
            nonce: Some(0),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };
    run_tx(tx.clone(), gas_price, native_price, true, false)
}

// Test delta gas, pass lower ratio
#[test]
fn test_delta_gas() {
    let wallet = PrivateKeySigner::from_str(WALLET).unwrap();
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };
    // Should succeed
    run_tx(tx, gas_price, native_price, true, false)
}
