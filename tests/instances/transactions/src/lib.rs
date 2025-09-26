//!
//! These tests are focused on different tx types, AA features.
//!
#![cfg(test)]
use alloy::consensus::{TxEip1559, TxEip2930, TxLegacy};
use alloy::primitives::TxKind;
use alloy::signers::local::PrivateKeySigner;
use rig::alloy::consensus::TxEip7702;
use rig::alloy::primitives::{address, b256};
use rig::alloy::rpc::types::{AccessList, AccessListItem, TransactionRequest};
use rig::ethers::types::Address;
use rig::ruint::aliases::{B160, U256};
use rig::{alloy, ethers, zksync_web3_rs, Chain};
use rig::{utils::*, BlockContext};
use std::str::FromStr;
use zksync_web3_rs::eip712::Eip712Meta;
use zksync_web3_rs::signers::{LocalWallet, Signer};
mod native_charging;

fn run_base_system_common(use_712: bool) {
    let mut chain = Chain::empty(None);
    // FIXME: this address looks very similar to bridgehub/shared bridge on gateway.
    // Which seems to suggest that it is special.
    // Consider changing this one to be more "random".

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    // We used for test where from cannot have deployed code
    let eoa_wallet = PrivateKeySigner::from_str(
        "a226d3a5c8c408741c3446c762aee8dff742f21e381a0e5ab85a96c5c00100be",
    )
    .unwrap();
    let eoa_wallet_ethers = LocalWallet::from_bytes(eoa_wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");
    let meta = Eip712Meta::new().gas_per_pubdata(1);

    let encoded_mint_tx = if use_712 {
        let mint_tx = rig::zksync_web3_rs::eip712::Eip712TransactionRequest::new()
            .chain_id(37)
            .from(from)
            .to(rig::ethers::abi::Address::from_str(to.to_string().as_str()).unwrap())
            .gas_limit(120_000)
            .max_fee_per_gas(1000)
            .max_priority_fee_per_gas(1000)
            .data(hex::decode(ERC_20_MINT_CALLDATA).unwrap())
            .custom_data(meta.clone())
            .nonce(0);
        rig::utils::sign_and_encode_eip712_tx(mint_tx, &wallet_ethers)
    } else {
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

    let encoded_transfer_tx = if use_712 {
        let transfer_tx = zksync_web3_rs::eip712::Eip712TransactionRequest::new()
            .chain_id(37)
            .from(from)
            .to(ethers::abi::Address::from_str(to.to_string().as_str()).unwrap())
            .gas_limit(100_000)
            .max_fee_per_gas(1000)
            .max_priority_fee_per_gas(1000)
            .data(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap())
            .custom_data(meta.clone())
            .nonce(1);
        rig::utils::sign_and_encode_eip712_tx(transfer_tx, &wallet_ethers)
    } else {
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

    // `to` == null
    let encoded_deployment_tx = if use_712 {
        let deployment_tx = zksync_web3_rs::eip712::Eip712TransactionRequest::new()
            .chain_id(37)
            .from(from)
            .gas_limit(1_200_000)
            .max_fee_per_gas(1000)
            .max_priority_fee_per_gas(1000)
            .data(hex::decode(ERC_20_DEPLOYMENT_BYTECODE).unwrap())
            .custom_data(meta.clone())
            .nonce(2);
        rig::utils::sign_and_encode_eip712_tx(
            deployment_tx,
            &LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap(),
        )
    } else {
        let deployment_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 2,
            gas_price: 1000,
            gas_limit: 900_000,
            to: TxKind::Create,
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_DEPLOYMENT_BYTECODE).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(deployment_tx, &wallet)
    };
    let encoded_transfer_to_eoa_tx = if use_712 {
        let eoa_to = "4242000000000000000000000000000000000000";
        let deployment_tx = zksync_web3_rs::eip712::Eip712TransactionRequest::new()
            .chain_id(37)
            .from(eoa_wallet_ethers.address())
            .gas_limit(21_000)
            .max_fee_per_gas(1000)
            .max_priority_fee_per_gas(1000)
            .to(rig::ethers::abi::Address::from_str(eoa_to).unwrap())
            .custom_data(meta.clone())
            .nonce(0);
        rig::utils::sign_and_encode_eip712_tx(
            deployment_tx,
            &LocalWallet::from_bytes(eoa_wallet.to_bytes().as_slice()).unwrap(),
        )
    } else {
        let eoa_to = address!("4242000000000000000000000000000000000000");
        let transfer_to_eoa = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 21_000,
            to: TxKind::Call(eoa_to),
            value: alloy::primitives::U256::from(100),
            access_list: Default::default(),
            input: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(transfer_to_eoa, &eoa_wallet)
    };

    let deployed = Address::from_str("0x14c252e395055507b10f199dd569f2379465d874").unwrap();

    let encoded_mint2_tx = if use_712 {
        let mint_tx = zksync_web3_rs::eip712::Eip712TransactionRequest::new()
            .chain_id(37)
            .from(from)
            .to(deployed)
            .gas_limit(100_000)
            .max_fee_per_gas(1000)
            .max_priority_fee_per_gas(1000)
            .data(hex::decode(ERC_20_MINT_CALLDATA).unwrap())
            .nonce(3);
        rig::utils::sign_and_encode_eip712_tx(mint_tx, &wallet_ethers)
    } else {
        let mint_tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 3,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 60_000,
            to: TxKind::Call(address!("14c252e395055507b10f199dd569f2379465d874")),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let encoded_l1_l2_transfer = {
        let transfer = TransactionRequest {
            chain_id: Some(37),
            from: Some(address!("1234000000000000000000000000000000000000")),
            to: Some(TxKind::Call(address!(
                "4242000000000000000000000000000000000000"
            ))),
            gas: Some(21_000),
            max_fee_per_gas: Some(1000),
            max_priority_fee_per_gas: Some(1000),
            value: Some(alloy::primitives::U256::from(100)),
            nonce: Some(0),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(transfer)
    };

    let encoded_l1_l2_erc_transfer = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(alloy::signers::Signer::address(&wallet)),
            to: Some(TxKind::Call(to)),
            gas: Some(40_000),
            max_fee_per_gas: Some(1000),
            max_priority_fee_per_gas: Some(1000),
            nonce: Some(3),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };

    let transactions = vec![
        encoded_mint_tx,
        encoded_transfer_tx,
        encoded_deployment_tx,
        encoded_transfer_to_eoa_tx,
        encoded_mint2_tx,
        encoded_l1_l2_transfer,
        encoded_l1_l2_erc_transfer,
    ];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain
        .set_balance(
            B160::from_be_bytes(from.0),
            U256::from(1_000_000_000_000_000_u64),
        )
        .set_balance(
            B160::from_be_bytes(eoa_wallet.address().0 .0),
            U256::from(1_000_000_000_000_000_u64),
        );

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}",)
        }
        success
    }));
}

#[test]
fn test_block_of_erc20() {
    let mut chain = Chain::empty_randomized(None);
    run_block_of_erc20(&mut chain, 10, None);
}

#[test]
fn test_gas_price_zero() {
    let mut chain = Chain::empty_randomized(None);
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..BlockContext::default()
    };
    run_block_of_erc20(&mut chain, 10, Some(block_context));
}

#[test]
fn test_withdrawal() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();

    // L2 base token address
    let to = address!("000000000000000000000000000000000000800a");

    let withdrawal_calldata =
        hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

    let withdrawal_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: U256::from(10),
            input: withdrawal_calldata.into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let mut withdrawal_with_message_calldata =
        hex::decode("84bc3eb0000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();
    // Offset (64)
    withdrawal_with_message_calldata.extend_from_slice(&U256::from(64).to_be_bytes::<32>());
    // length, 2 bytes
    withdrawal_with_message_calldata.extend_from_slice(&U256::from(2).to_be_bytes::<32>());
    // Extra data
    withdrawal_with_message_calldata.extend_from_slice(&[1u8, 2u8]);

    let withdrawal_with_message_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 1,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: U256::from(5),
            input: withdrawal_with_message_calldata.into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![withdrawal_tx, withdrawal_with_message_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}")
        }
        success
    }));

    // Check preimage of withdrawal
    let mut expected_preimage =
        hex::decode("6c0960f9aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    expected_preimage.extend_from_slice(&U256::from(10).to_be_bytes::<32>());

    let logs = output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .unwrap()
        .l2_to_l1_logs;

    let first_log = logs.first().unwrap().clone();
    let returned_preimage = first_log.preimage.unwrap();
    assert_eq!(expected_preimage, returned_preimage);
}

#[test]
fn test_tx_with_access_list() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let encoded_mint_tx = {
        let access_list = AccessList::from(vec![AccessListItem {
            address: to,
            storage_keys: vec![b256!(
                "0x0000000000000000000000000000000000000000000000000000000000000000"
            )],
        }]);
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 75_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
            access_list,
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

#[cfg(feature = "pectra")]
#[test]
fn test_tx_with_authorization_list() {
    use rig::alloy::eips::eip7702::*;
    use rig::alloy::signers::SignerSync;
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let delegate = PrivateKeySigner::from_str(
        "a226d3a5c8c408741c3446c762aee8dff742f21e381a0e5ab85a96c5c00100be",
    )
    .unwrap();

    let from = wallet_ethers.address();
    let to = delegate.address();

    let erc_20_contract = address!("0000000000000000000000000000000000010002");

    let encoded_mint_tx = {
        let authorization = Authorization {
            chain_id: U256::from(37u64),
            address: erc_20_contract,
            nonce: 0,
        };
        let signed_hash = authorization.signature_hash();
        let sig = delegate.sign_hash_sync(&signed_hash).expect("must sign");
        let signed = authorization.into_signed(sig);
        let authorization_list = vec![signed];
        let mint_tx = TxEip7702 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 100_000,
            to,
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
            access_list: Default::default(),
            authorization_list,
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(erc_20_contract.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

#[test]
fn test_deployment_tx_with_authorization_list_fails() {
    use rig::alloy::eips::eip7702::*;
    use rig::alloy::signers::SignerSync;
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let delegate = PrivateKeySigner::from_str(
        "a226d3a5c8c408741c3446c762aee8dff742f21e381a0e5ab85a96c5c00100be",
    )
    .unwrap();

    let from = wallet_ethers.address();

    let erc_20_contract = address!("0000000000000000000000000000000000010002");

    let encoded_mint_tx = {
        let authorization = Authorization {
            chain_id: U256::from(37u64),
            address: erc_20_contract,
            nonce: 0,
        };
        let signed_hash = authorization.signature_hash();
        let sig = delegate.sign_hash_sync(&signed_hash).expect("must sign");
        let signed = authorization.into_signed(sig);
        let authorization_list = vec![signed];
        let mint_tx = TxEip7702 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 100_000,
            to: alloy::primitives::Address::ZERO,
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
            access_list: Default::default(),
            authorization_list,
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(erc_20_contract.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None);

    // Assert all txs failed
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_err());
}

// Test that slots made warm in a tx are cold in the next tx
#[test]

fn test_cold_in_new_tx() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 68_358,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    // Gas is just enough to succeed.
    let encoded_mint1_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 1,
            gas_price: 1000,
            gas_limit: 34158,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    // Any lower gas amount should fail
    let encoded_mint_tx2 = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 2,
            gas_price: 1000,
            gas_limit: 34158 - 1,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_mint_tx, encoded_mint1_tx, encoded_mint_tx2];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    let result1 = output.tx_results.get(1).unwrap().clone();
    let result2 = output.tx_results.get(2).unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
    assert!(result1.is_ok_and(|o| o.is_success()));
    assert!(result2.is_ok_and(|o| !o.is_success()));
}

// TODO: find better place for regression tests
#[test]
fn test_regression_returndata_empty_3541() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();
    // Code for:
    // PUSH13 0x63EF0000006000526004601CF3
    // PUSH1  0x00
    // MSTORE
    // PUSH1  0x0D
    // PUSH1  0x13
    // PUSH1  0x00
    // CREATE
    // RETURNDATASIZE
    // ISZERO
    // PUSH1  0x08
    // PC
    // ADD
    // JUMPI
    // PUSH1  0x00
    // PUSH1  0x00
    // REVERT
    // JUMPDEST
    // PUSH1  0x00
    // PUSH1  0x00
    // RETURN
    // This code tries to deploy a contract with code starting with EF and
    // expects returndata to be empty, otherwise it reverts.
    const BYTECODE: &str =
        "6c63ef0000006000526004601cf3600052600d60136000f03d15600858015760006000fd5b60006000f3";

    let from = wallet_ethers.address();

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let encoded_tx = {
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 1_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_tx];

    let bytecode = hex::decode(BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None);

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

#[test]
fn run_base_system() {
    run_base_system_common(false);
}

#[test]
fn run_base_712_system() {
    run_base_system_common(true);
}

/// Test that transactions with balance calculation overflow are properly rejected
#[test]
fn test_balance_overflow_protection() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    let from = alloy::primitives::Address::from_slice(&wallet.address().as_slice());
    let to = address!("0000000000000000000000000000000000010002");

    // Set a reasonable balance that would be sufficient for normal transactions
    chain.set_balance(
        B160::from_be_bytes(from.into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

    // Test 1: Transaction with max_fee_per_gas * gas_limit overflow
    let overflow_fee_tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            gas_limit: u64::MAX, // Will cause overflow when multiplied with max_fee_per_gas
            max_fee_per_gas: u128::MAX,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: U256::from(100u64), // Small value
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    // Test 2: Transaction with value + fee_amount overflow
    let overflow_total_tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 1,
            gas_limit: 100_000,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: U256::MAX, // Maximum value will cause overflow when adding fees
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let output = chain.run_block(vec![overflow_fee_tx, overflow_total_tx], None, None);

    assert!(
        output.tx_results.get(0).unwrap().is_err(),
        "Transaction with fee overflow should fail"
    );
    assert!(
        output.tx_results.get(1).unwrap().is_err(),
        "Transaction with total balance overflow should fail"
    );
}

/// Test that upgrade transactions (L1 -> L2) that revert raise an internal error
/// instead of a validation error.
#[test]
fn test_upgrade_tx_revert_internal_error() {
    let mut chain = Chain::empty(None);

    // Create a contract that always reverts
    let revert_contract_address = address!("0000000000000000000000000000000000010003");
    // Simple contract bytecode that just does REVERT(0, 0)
    let revert_bytecode = hex::decode("60006000fd").unwrap(); // PUSH1 0, PUSH1 0, REVERT
    chain.set_evm_bytecode(
        B160::from_be_bytes(revert_contract_address.into_array()),
        &revert_bytecode,
    );

    // Create a proper upgrade transaction that calls the reverting contract
    let upgrade_tx = encode_upgrade_tx(TransactionRequest {
        chain_id: Some(37),
        from: Some(address!("1234000000000000000000000000000000000000")),
        to: Some(TxKind::Call(revert_contract_address)),
        gas: Some(100_000u64),
        max_fee_per_gas: Some(0),
        max_priority_fee_per_gas: Some(0),
        value: Some(alloy::primitives::U256::from(0)),
        nonce: Some(0),
        ..TransactionRequest::default()
    });

    let transactions = vec![upgrade_tx];

    // Use run_block_no_panic to catch the error instead of panicking
    let result = chain.run_block_no_panic(transactions, None, None, false);

    // The upgrade transaction should fail with an internal error (not validation error)
    assert!(result.is_err());

    // The error should be an internal error containing "Upgrade transaction must succeed"
    let error = result.unwrap_err();
    let error_debug = format!("{:?}", error);
    assert!(
        error_debug.contains("Upgrade transaction must succeed"),
        "Expected error to contain 'Upgrade transaction must succeed', got: {}",
        error_debug
    );
}

#[test]
fn test_upgrade_tx_succeeds() {
    let mut chain = Chain::empty(None);

    // Create a contract that always succeeds
    let revert_contract_address = address!("0000000000000000000000000000000000010003");
    // Simple contract bytecode that just does RETURN(0, 0)
    let revert_bytecode = hex::decode("60006000f3").unwrap(); // PUSH1 0, PUSH1 0, RETURN
    chain.set_evm_bytecode(
        B160::from_be_bytes(revert_contract_address.into_array()),
        &revert_bytecode,
    );

    // Create a proper upgrade transaction that calls the contract
    let upgrade_tx = encode_upgrade_tx(TransactionRequest {
        chain_id: Some(37),
        from: Some(address!("1234000000000000000000000000000000000000")),
        to: Some(TxKind::Call(revert_contract_address)),
        gas: Some(100_000u64),
        max_fee_per_gas: Some(0),
        max_priority_fee_per_gas: Some(0),
        value: Some(alloy::primitives::U256::from(0)),
        nonce: Some(0),
        ..TransactionRequest::default()
    });

    let transactions = vec![upgrade_tx];

    // Use run_block_no_panic to catch the error instead of panicking
    let result = chain.run_block_no_panic(transactions, None, None, false);
    assert!(result.is_ok());

    assert!(result.unwrap().tx_results[0].as_ref().unwrap().is_success());
}

#[test]
fn test_invalid_transaction_type_failure() {
    let mut chain = Chain::empty(None);

    // Create a simple success contract for the call
    let contract_address = address!("0000000000000000000000000000000000010003");
    let success_bytecode = hex::decode("60006000f3").unwrap(); // PUSH1 0, PUSH1 0, RETURN
    chain.set_evm_bytecode(
        B160::from_be_bytes(contract_address.into_array()),
        &success_bytecode,
    );

    let transaction_types = vec![0x7d, 0x80, 0xFF]; // Some invalid types;

    for tx_type in transaction_types {
        let invalid_tx = encode_special_tx_type(
            TransactionRequest {
                chain_id: Some(37),
                from: Some(address!("1234000000000000000000000000000000000000")),
                to: Some(TxKind::Call(contract_address)),
                gas: Some(100_000u64),
                max_fee_per_gas: Some(0),
                max_priority_fee_per_gas: Some(0),
                value: Some(alloy::primitives::U256::from(0)),
                nonce: Some(0),
                ..TransactionRequest::default()
            },
            tx_type,
        );

        let transactions = vec![invalid_tx];
        let result = chain.run_block(transactions, None, None);
        assert!(
            result.tx_results[0].is_err(),
            "Transaction with invalid type should fail"
        );
    }
}
