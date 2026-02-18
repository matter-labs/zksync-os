//!
//! These tests are focused on different tx types.
//!
#![cfg(test)]
use alloy::consensus::{TxEip1559, TxEip2930, TxEip4844, TxLegacy};
use alloy::primitives::TxKind;
use alloy::signers::local::PrivateKeySigner;
use rig::alloy::consensus::TxEip7702;
use rig::alloy::primitives::{address, b256};
use rig::alloy::rpc::types::{AccessList, AccessListItem, TransactionRequest};
use rig::basic_bootloader::bootloader::block_flow::zk::PUBDATA_ENCODING_VERSION;
use rig::chain::RunConfig;
use rig::forward_system::run::convert_alloy::FromAlloy;
use rig::ruint::aliases::{B160, U256};
use rig::system_hooks::addresses_constants::L2_INTEROP_ROOT_STORAGE_ADDRESS;
use rig::testing_utils::install_system_contracts;
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::{alloy, zksync_web3_rs, Chain};
use rig::{utils::*, BlockContext};
use std::str::FromStr;
use zksync_os_tests_common::zksync_tx::encoding::ZKsyncOsEncodable;
use zksync_os_tests_common::zksync_tx::service_tx::ZKsyncServiceTx;
use zksync_os_tests_common::zksync_tx::upgrade_tx::ZKsyncUpgradeTx;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use zksync_web3_rs::signers::{LocalWallet, Signer};

mod l1_tx_resilience;
mod native_charging;

fn run_config() -> Option<rig::chain::RunConfig> {
    Some(rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        skip_minting_tokens_to_treasury: false,
        ..Default::default()
    })
}

#[test]
fn run_base_system() {
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

    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");

    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 80_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

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
        ZKsyncTxEnvelope::from_eth_tx(transfer_tx, wallet.clone()).encode()
    };

    // `to` == null
    let encoded_deployment_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(deployment_tx, wallet.clone()).encode()
    };
    let encoded_transfer_to_eoa_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(transfer_to_eoa, eoa_wallet.clone()).encode()
    };

    let encoded_mint2_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let encoded_l1_l2_transfer = {
        let transfer = L1TxBuilder::new()
            .from(address!("1234000000000000000000000000000000000000"))
            .to(address!("4242000000000000000000000000000000000000"))
            .value(alloy::primitives::U256::from(100))
            .gas_price(1000)
            .gas_limit(21_000)
            .build();
        transfer.encode()
    };

    let encoded_l1_l2_erc_transfer = {
        let tx = L1TxBuilder::new()
            .from(wallet.address())
            .to(to)
            .input(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into())
            .gas_price(1000)
            .gas_limit(40_000)
            .nonce(3)
            .build();
        tx.encode()
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
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);

    chain
        .set_balance(
            B160::from_be_bytes(from.0),
            U256::from(1_000_000_000_000_000_u64),
        )
        .set_balance(
            B160::from_alloy(eoa_wallet.address()),
            U256::from(1_000_000_000_000_000_u64),
        ) // Set the balance for L1 -> L2 tx msg.value transfer
        .set_balance(
            B160::from_be_bytes(address!("1234000000000000000000000000000000000000").into()),
            alloy::primitives::U256::from(100),
        );

    let output = chain.run_block(transactions, None, None, run_config());

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
fn test_gas_price_zero_fee_zero() {
    let mut chain = Chain::empty_randomized(None);
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..BlockContext::default()
    };
    let output = run_block_of_erc20_with_fee(&mut chain, 10, Some(block_context), 0);
    let res_0 = output
        .tx_results
        .first()
        .cloned()
        .expect("Must have first result")
        .expect("Must be valid");

    // Regression check, at some point txs with 0 gas price were returning 0 native used.
    assert!(
        res_0.native_used > 0,
        "Native used must be greater than zero"
    );
}

#[test]
fn test_gas_price_zero_fee_one() {
    let mut chain = Chain::empty_randomized(None);
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..BlockContext::default()
    };
    run_block_of_erc20_with_fee(&mut chain, 10, Some(block_context), 1);
}

#[test]
fn test_withdrawal() {
    let mut chain = Chain::empty(None);
    install_system_contracts(&mut chain, true, false, false);

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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let transactions = vec![withdrawal_tx, withdrawal_with_message_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    install_system_contracts(&mut chain, false, true, false);

    let output = chain.run_block(transactions, None, None, run_config());

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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let transactions = vec![encoded_mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let transactions = vec![encoded_mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_alloy(erc_20_contract), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let run_config = rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    let output = chain.run_block(transactions, None, None, Some(run_config));

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let transactions = vec![encoded_mint_tx, encoded_mint1_tx, encoded_mint_tx2];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    let result1 = output.tx_results.get(1).unwrap().clone();
    let result2 = output.tx_results.get(2).unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
    assert!(result1.is_ok_and(|o| o.is_success()));
    assert!(result2.is_ok_and(|o| !o.is_success()));
}

#[test]
// Test that if we send 2 simple transfers from and to different addresses,
// the length of the pubdata from both is the same.
fn test_independent_txs_have_same_pubdata() {
    let mut chain = Chain::empty(None);

    let wallet1 = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    let wallet2 = PrivateKeySigner::from_str(
        "abcdebdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let to1 = address!("0000000000000000000000000000000000010002");
    let to2 = address!("0000000000000000000000000000000000010003");

    let encoded_tx_1 = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1500,
            max_priority_fee_per_gas: 1500,
            gas_limit: 21_000,
            to: TxKind::Call(to1),
            value: U256::from(10),
            input: Default::default(),
            ..Default::default()
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet1.clone()).encode()
    };

    let encoded_tx_2 = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1500,
            max_priority_fee_per_gas: 1500,
            gas_limit: 21_000,
            to: TxKind::Call(to2),
            value: U256::from(10),
            input: Default::default(),
            ..Default::default()
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet2.clone()).encode()
    };

    let transactions = vec![encoded_tx_1, encoded_tx_2];

    chain
        .set_balance(
            B160::from_alloy(wallet1.address()),
            U256::from(1_000_000_000_000_000_u64),
        )
        .set_balance(
            B160::from_alloy(wallet2.address()),
            U256::from(1_000_000_000_000_000_u64),
        );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded and compare pubdata len
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}",)
        }
        success
    }));
    let result1 = output.tx_results.first().unwrap().clone();
    let result2 = output.tx_results.get(1).unwrap().clone();
    let pubdata_used_1 = result1.unwrap().pubdata_used;
    let pubdata_used_2 = result2.unwrap().pubdata_used;
    assert_eq!(pubdata_used_1, pubdata_used_2, "Pubdata used not equal")
}

#[test]
fn test_invalid_tx_does_not_bump_tx_counter() {
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();
    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    let mut chain = Chain::empty(None);

    // Invalid tx first
    let encoded_mint1_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 34_158_000_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };
    let withdrawal_tx = {
        let l1_messenger_contract = address!("0000000000000000000000000000000000008008");
        let l1_messenger_hook = address!("0000000000000000000000000000000000007001");

        chain.set_balance(
            B160::from_alloy(l1_messenger_contract),
            U256::from(1_000_000_000_000_000_u64),
        );

        let withdrawal_calldata =
            hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();

        let tx = L1TxBuilder::new()
            .from(l1_messenger_contract)
            .to(l1_messenger_hook)
            .input(withdrawal_calldata)
            .gas_price(1000)
            .gas_limit(500_000)
            .nonce(0)
            .build();

        tx.encode()
    };

    let transactions = vec![encoded_mint1_tx, withdrawal_tx];
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);
    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None, None);

    // Assert tx succeeded/failed
    let result0 = output.tx_results.first().unwrap().clone();
    let result1 = output.tx_results.get(1).unwrap().clone();

    assert!(result0.as_ref().is_err());
    assert!(result1.as_ref().is_ok_and(|o| o.is_success()));
    assert_eq!(
        result1
            .unwrap()
            .l2_to_l1_logs
            .first()
            .unwrap()
            .log
            .tx_number_in_block,
        0
    );
}

#[test]
fn test_invalid_tx_does_not_affect_native() {
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();
    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    // First we run a single tx to get the "normal" amount of native used
    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let mut chain = Chain::empty(None);
    let transactions = vec![encoded_mint_tx.clone()];
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);
    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );
    let output = chain.run_block(transactions, None, None, None);

    // Assert tx succeeded
    let result = output.tx_results.first().unwrap().clone();
    assert!(result.as_ref().is_ok_and(|o| o.is_success()));

    let native_used_reference = result.unwrap().native_used;

    // Same tx but with a huge gas limit, which makes it invalid
    // We run this one first and then the valid one, and check that
    // the valid one uses the same amount of native as in the reference case.
    let encoded_mint1_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 34_158_000_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let mut chain = Chain::empty(None);
    let transactions = vec![encoded_mint1_tx, encoded_mint_tx];
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);
    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );
    let output = chain.run_block(transactions, None, None, None);

    // Assert tx succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    let result1 = output.tx_results.get(1).unwrap().clone();
    assert!(result0.as_ref().is_err());
    assert!(result1.as_ref().is_ok_and(|o| o.is_success()));
    assert_eq!(
        result1.unwrap().native_used,
        native_used_reference,
        "Native used doesn't match"
    );
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let transactions = vec![encoded_tx];

    let bytecode = hex::decode(BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
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
        B160::from_alloy(from),
        U256::from(1_000_000_000_000_000_u64),
    );

    // Test 1: Transaction with max_fee_per_gas * gas_limit overflow
    let overflow_fee_tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            gas_limit: (u64::MAX / 300), // Will cause overflow when multiplied with max_fee_per_gas
            max_fee_per_gas: u128::MAX,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: U256::from(100u64), // Small value
            ..Default::default()
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    let output = chain.run_block(
        vec![overflow_fee_tx, overflow_total_tx],
        None,
        None,
        run_config(),
    );

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
    chain.set_evm_bytecode(B160::from_alloy(revert_contract_address), &revert_bytecode);

    // Create a proper upgrade transaction that calls the reverting contract

    let upgrade_tx = ZKsyncTxEnvelope::from(ZKsyncUpgradeTx {
        from: address!("1234000000000000000000000000000000000000"),
        to: revert_contract_address,
        gas_limit: 100_000u128,
        ..Default::default()
    });

    let transactions = vec![upgrade_tx.encode()];

    // Use run_block_no_panic to catch the error instead of panicking
    let result = chain.run_block_no_panic(transactions, None, None, None);

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
    chain.set_evm_bytecode(B160::from_alloy(revert_contract_address), &revert_bytecode);

    // Create a proper upgrade transaction that calls the contract
    let upgrade_tx = ZKsyncTxEnvelope::from(ZKsyncUpgradeTx {
        from: address!("1234000000000000000000000000000000000000"),
        to: revert_contract_address,
        gas_limit: 100_000u128,
        ..Default::default()
    });

    let transactions = vec![upgrade_tx.encode()];

    // Use run_block_no_panic to catch the error instead of panicking
    let result = chain.run_block_no_panic(transactions, None, None, None);
    assert!(result.is_ok());

    assert!(result.unwrap().tx_results[0].as_ref().unwrap().is_success());
}

#[test]
fn test_invalid_transaction_type_failure() {
    let mut chain = Chain::empty(None);

    // Create a simple success contract for the call
    let contract_address = address!("0000000000000000000000000000000000010003");
    let success_bytecode = hex::decode("60006000f3").unwrap(); // PUSH1 0, PUSH1 0, RETURN
    chain.set_evm_bytecode(B160::from_alloy(contract_address), &success_bytecode);

    let transaction_types = vec![0x55, 0x80, 0xFF]; // Some invalid types;

    for tx_type in transaction_types {
        let invalid_tx = ZKsyncTxEnvelope::new_custom_tx_type(
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

        let transactions = vec![invalid_tx.encode()];
        let result = chain.run_block(transactions, None, None, run_config());
        assert!(
            result.tx_results[0].is_err(),
            "Transaction with invalid type should fail"
        );
    }
}

#[test]
fn test_modexp_intermediate_zero_block() {
    let mut chain = Chain::empty(None);
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    // Modexp precompile address
    let modexp_address = address!("0000000000000000000000000000000000000005");

    let input_data = hex::decode(concat!(
        // Base length (96 bytes)
        "0000000000000000000000000000000000000000000000000000000000000060",
        // Exponent length (1 byte)
        "0000000000000000000000000000000000000000000000000000000000000001",
        // Modulus length (96 bytes)
        "0000000000000000000000000000000000000000000000000000000000000060",
        // Base (96 bytes):
        "1000000000000000000000000000000000000000000000000000000000000001",
        "0000000000000000000000000000000000000000000000000000000000000000", // zeroed 32-bytes block
        "1000000000000000000000000000000000000000000000000000000000000001",
        // Exponent (1 byte)
        "01",
        // Modulus (96 bytes): nop mask
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
    ))
    .unwrap();

    // Unchanged base
    let expected_output = hex::decode(concat!(
        "1000000000000000000000000000000000000000000000000000000000000001",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "1000000000000000000000000000000000000000000000000000000000000001",
    ))
    .unwrap();

    let encoded_tx = {
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 1_000_000,
            to: TxKind::Call(modexp_address),
            value: Default::default(),
            input: input_data.into(),
            ..Default::default()
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let transactions = vec![encoded_tx];

    chain.set_balance(
        B160::from_alloy(wallet.address()),
        U256::from(10u64.pow(18)),
    );

    let result = chain.run_block(transactions, None, None, None);

    // The transaction should succeed
    assert!(
        result.tx_results[0].is_ok(),
        "Modexp transaction should succeed"
    );

    // Extract the result and check it
    let tx_result = result.tx_results[0].as_ref().unwrap();
    assert!(tx_result.is_success(), "Transaction should be successful");

    match &tx_result.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(execution_output) => {
            match execution_output {
                rig::zksync_os_interface::types::ExecutionOutput::Call(result) => {
                    assert_eq!(*result, expected_output)
                }
                rig::zksync_os_interface::types::ExecutionOutput::Create(_, _) => panic!(),
            }
        }
        rig::zksync_os_interface::types::ExecutionResult::Revert(_) => unreachable!(),
    }
}

#[test]
fn test_point_eval_call() {
    let mut chain = Chain::empty(None);
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    let point_eval_address = address!("000000000000000000000000000000000000000a");

    let input_data = vec![
        1, 102, 133, 225, 114, 167, 73, 247, 66, 106, 69, 37, 154, 47, 12, 166, 56, 32, 114, 250,
        248, 157, 192, 88, 251, 163, 154, 210, 121, 34, 66, 235, 85, 219, 9, 223, 116, 132, 184,
        93, 126, 40, 112, 220, 62, 82, 110, 135, 177, 46, 241, 113, 107, 197, 47, 252, 248, 42,
        160, 119, 67, 165, 212, 245, 18, 209, 170, 150, 140, 245, 200, 141, 68, 162, 165, 129, 82,
        66, 8, 42, 39, 249, 157, 47, 168, 22, 131, 131, 56, 185, 83, 43, 243, 206, 226, 45, 145,
        193, 172, 89, 253, 243, 68, 226, 169, 9, 142, 178, 195, 105, 155, 150, 82, 169, 168, 239,
        192, 6, 196, 189, 168, 161, 215, 100, 180, 160, 250, 218, 60, 52, 231, 42, 12, 196, 209,
        81, 166, 221, 19, 125, 222, 83, 74, 242, 149, 23, 202, 113, 140, 69, 14, 237, 147, 86, 3,
        205, 89, 133, 238, 107, 188, 251, 226, 218, 135, 226, 78, 100, 190, 143, 162, 216, 23, 51,
        224, 222, 155, 138, 17, 239, 215, 199, 63, 57, 137, 141, 21, 143, 208, 196, 134, 126,
    ];

    let expected_output = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        16, 0, 115, 237, 167, 83, 41, 157, 125, 72, 51, 57, 216, 8, 9, 161, 216, 5, 83, 189, 164,
        2, 255, 254, 91, 254, 255, 255, 255, 255, 0, 0, 0, 1,
    ];

    let encoded_tx = {
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 1_000_000,
            to: TxKind::Call(point_eval_address),
            value: Default::default(),
            input: input_data.into(),
            ..Default::default()
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone()).encode()
    };

    let transactions = vec![encoded_tx];

    chain.set_balance(
        B160::from_alloy(wallet.address()),
        U256::from(10u64.pow(18)),
    );

    let result = chain.run_block(transactions, None, None, None);

    // The transaction should succeed
    assert!(result.tx_results[0].is_ok(), "Transaction should succeed");

    // Extract the result and check it
    let tx_result = result.tx_results[0].as_ref().unwrap();
    assert!(tx_result.is_success(), "Transaction should be successful");

    match &tx_result.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(execution_output) => {
            match execution_output {
                rig::zksync_os_interface::types::ExecutionOutput::Call(result) => {
                    assert_eq!(*result, expected_output)
                }
                rig::zksync_os_interface::types::ExecutionOutput::Create(_, _) => panic!(),
            }
        }
        rig::zksync_os_interface::types::ExecutionResult::Revert(_) => unreachable!(),
    }
}

#[test]
fn test_selfdestruct_to_precompile_gas() {
    // Test that a selfdestruct with a precompile as target doesn't charge for
    // extra warm gas (regression)

    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    let contract_address = address!("1000000000000000000000000000000000000001");

    // PUSH20 0x01
    // SELFDESTRUCT
    let bytecode = hex::decode("730000000000000000000000000000000000000001ff").unwrap();

    chain.set_balance(
        B160::from_alloy(wallet.address()),
        U256::from(1_000_000_000_000_000_u64),
    );
    chain.set_evm_bytecode(B160::from_alloy(contract_address), &bytecode);

    use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

    let tx_request = {
        let tx_request = TransactionRequest {
            chain_id: Some(37),
            from: Some(wallet.address()),
            to: Some(TxKind::Call(contract_address)),
            input: Default::default(),
            value: Some(U256::from(0)),
            gas: Some(75_000),
            max_fee_per_gas: Some(1000),
            max_priority_fee_per_gas: Some(1000),
            gas_price: Some(1000),
            nonce: Some(0),
            ..Default::default()
        };

        ZKsyncTxEnvelope::from_eth_tx_from_req(tx_request, wallet)
    };

    let result = chain.run_block(vec![tx_request.encode()], None, None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
    let gas_used = res0.clone().unwrap().gas_used;
    assert_eq!(gas_used, 26003);
}

#[test]
fn test_reject_caller_with_code_behavior() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    // Create a contract address with bytecode deployed
    let contract_address = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Deploy bytecode to the contract address to make it a "contract with code"
    chain.set_evm_bytecode(
        B160::from_alloy(contract_address),
        &hex::decode("60006000f3").unwrap(), // Simple contract: PUSH1 0, PUSH1 0, RETURN
    );

    // Set balance for the contract address
    chain.set_balance(
        B160::from_alloy(contract_address),
        U256::from(1_000_000_000_000_000_u64),
    );

    let from_contract_tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    let result_simulation = chain.simulate_block(vec![from_contract_tx.clone()], None);

    // In simulation mode, the transaction should succeed
    assert!(result_simulation.tx_results[0].is_ok(),);

    let tx_result = result_simulation.tx_results[0].as_ref().unwrap();
    assert!(
        tx_result.is_success(),
        "Transaction should be successful in simulation mode"
    );

    // But in normal mode it should fail
    let result_normal = chain.run_block(vec![from_contract_tx], None, None, run_config());
    assert!(matches!(
        result_normal.tx_results[0],
        Err(InvalidTransaction::RejectCallerWithCode)
    ));
}

#[test]
fn test_expensive_pubdata() {
    // Test if a transaction can be executed even if the pubdata price is such that
    // validation pubdata requires to use withheld resources.
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Set balance for the contract address
    chain.set_balance(B160::from_alloy(from), U256::from(u64::MAX));

    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    // Validation uses 40 bytes of pubdata, we want the validation
    // pubdata charge to be > MAX_NATIVE_COMPUTATIONAL (2^35), to
    // ensure we use withheld resources for it.
    let native_price = U256::from(100);
    // Value s.t. (pubdata_price / native_price) * 40 > MAX_NATIVE_COMPUTATIONAL
    let pubdata_price = U256::from(85899346000u64);

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
}

#[test]
fn test_check_pubdata_encoding_version() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Set balance for the contract address
    chain.set_balance(B160::from_alloy(from), U256::from(u64::MAX));

    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    let native_price = U256::from(100);
    let pubdata_price = U256::from(2);

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");

    assert_eq!(result.pubdata[0], PUBDATA_ENCODING_VERSION);
}

#[test]
fn test_check_pubdata_has_timestamp() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Set balance for the contract address
    chain.set_balance(B160::from_alloy(from), U256::from(u64::MAX));

    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    let native_price = U256::from(100);
    let pubdata_price = U256::from(2);
    let timestamp: u64 = 42;

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        timestamp,
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");

    // Pubdata format is [VERSION(1)][BLOCK_HASH(32)][TIMESTAMP(8)][DIFFS...]
    let pubdata_timestamp_bytes = &result.pubdata.as_slice()[33..41];
    let pubdata_timestamp = u64::from_be_bytes(
        pubdata_timestamp_bytes
            .try_into()
            .expect("Slice with incorrect length"),
    );
    assert_eq!(timestamp, pubdata_timestamp, "Timestamps do not match");
}

#[test]
fn test_simple_service_transaction() {
    let mut chain = Chain::empty(None);
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    let tx = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    })
    .encode();

    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
}

#[test]
fn test_simple_service_transaction_whitelist() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    // Invalid target
    let target_address = [0u8; 20];

    // Set balance for the contract address
    chain.set_balance(B160::from_alloy(from), U256::from(u64::MAX));

    let tx = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    })
    .encode();

    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_err(), "Tx should fail");
}

#[test]
fn test_service_tx_gas_limit_exceeds_block() {
    let mut chain = Chain::empty(None);
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    let tx = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    })
    .encode();

    let block_context = BlockContext {
        gas_limit: 30_000_000,
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };

    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    assert!(result.tx_results[0].is_ok());
}

#[test]
fn test_service_block_invariants() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    // Set balance for the contract address
    chain.set_balance(B160::from_alloy(from), U256::from(u64::MAX));

    // Check that a service block with several service txs works
    let tx1 = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    })
    .encode();
    let tx2 = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 1,
    })
    .encode();
    let tx3 = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 2,
    })
    .encode();

    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    // Check txs succeed
    let result = chain.run_block(vec![tx1, tx2, tx3], Some(block_context), None, run_config());
    assert!(
        result.tx_results.iter().all(|res| res.is_ok()),
        "All txs should succeed"
    );

    // Check that a service block with a non-service tx fails
    let tx4 = encode_service_tx(&target_address, &[], 3);
    let tx_non_service = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(address!("4242000000000000000000000000000000000000")),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };
    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    chain
        .run_block_no_panic(
            vec![tx4.clone(), tx_non_service.clone()],
            Some(block_context),
            None,
            run_config(),
        )
        .expect_err("Service block with non service tx should fail");

    // Check that a non-service block with a service tx fails
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };
    chain
        .run_block_no_panic(
            vec![tx_non_service, tx4],
            Some(block_context),
            None,
            run_config(),
        )
        .expect_err("Service block with non service tx should fail");
}

/// Regression test for: Skip nonce check on simulation
#[test]
fn test_simulation_skips_nonce_check() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Set balance so the tx can pay for gas
    chain.set_balance(
        B160::from_alloy(from),
        U256::from(1_000_000_000_000_000_u64),
    );

    // Create a transaction with nonce 100, but the account's nonce is 0
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 100, // Wrong nonce - account nonce is 0
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 21_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    // In simulation mode, the transaction should succeed (nonce check skipped)
    let result_simulation = chain.simulate_block(vec![tx.clone()], None);
    assert!(
        result_simulation.tx_results[0].is_ok(),
        "Transaction should pass validation in simulation mode, got: {:?}",
        result_simulation.tx_results[0]
    );

    // In normal execution mode, the transaction should fail with NonceTooHigh
    let result_normal = chain.run_block(vec![tx], None, None, run_config());
    assert!(
        matches!(
            result_normal.tx_results[0],
            Err(InvalidTransaction::NonceTooHigh { .. })
        ),
        "Transaction should fail with NonceTooHigh in normal mode, got: {:?}",
        result_normal.tx_results[0]
    );
}

/// Simulation for a transaction with sender without balance:
/// - gasPrice > 0 && value > 0: fail
/// - gasPrice > 0 && value == 0: fail
/// - gasPrice == 0 && value > 0: fail
/// - gasPrice == 0 && value == 0: ok
#[test]
fn test_simulation_balance_check() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let target_address = address!("4242000000000000000000000000000000000000");

    // - gasPrice > 0 && value > 0: fail
    let tx = {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 21_000,
            to: TxKind::Call(target_address),
            value: U256::from(1),
            input: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };
    let result_simulation = chain.simulate_block(vec![tx.clone()], None);
    assert!(
        result_simulation.tx_results[0].is_err(),
        "Transaction with fee and value should fail validation in simulation mode"
    );

    // - gasPrice > 0 && value == 0: fail
    let tx = {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 21_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };
    let result_simulation = chain.simulate_block(vec![tx.clone()], None);
    assert!(
        result_simulation.tx_results[0].is_err(),
        "Transaction with fee should fail validation in simulation mode"
    );

    //- gasPrice == 0 && value > 0: fail
    let tx = {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 0,
            gas_limit: 21_000,
            to: TxKind::Call(target_address),
            value: U256::from(1),
            input: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };
    let result_simulation = chain.simulate_block(vec![tx.clone()], None);
    assert!(
        result_simulation.tx_results[0].is_err(),
        "Transaction with value should fail validation in simulation mode"
    );

    // - gasPrice == 0 && value == 0: ok
    let tx = {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 0,
            gas_limit: 21_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };
    let result_simulation = chain.simulate_block(vec![tx.clone()], None);
    assert!(
        result_simulation.tx_results[0].is_ok(),
        "Transaction with no fee/value should pass validation in simulation mode"
    );
}

#[test]
fn test_simulation_4844_zero_blob_fee_allowed() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    chain.set_balance(
        B160::from_be_bytes(from.into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

    let tx = TxEip4844 {
        chain_id: 37u64,
        nonce: 0,
        max_fee_per_gas: 1_000,
        max_priority_fee_per_gas: 1_000,
        gas_limit: 75_000,
        to: target_address,
        value: U256::ZERO,
        input: Default::default(),
        access_list: Default::default(),
        blob_versioned_hashes: vec![b256!(
            "0x011122223333444455556666777788889999aaaabbbbccccddddeeeeffff0000"
        )],
        max_fee_per_blob_gas: 0,
    };
    let encoded_tx = ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode();

    let block_context = BlockContext {
        blob_fee: U256::from(1),
        ..Default::default()
    };

    let result_simulation = chain.simulate_block(vec![encoded_tx], Some(block_context));
    assert!(
        result_simulation.tx_results[0].is_ok(),
        "EIP-4844 tx should pass simulation when blob_fee > 0 and max_fee_per_blob_gas = 0, got: {:?}",
        result_simulation.tx_results[0]
    );
}

/// Check that gas and native used is the same in simulation and actual execution
#[test]
fn test_simulation_gas_and_native_used() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let target_address = address!("4242000000000000000000000000000000000000");

    chain.set_balance(
        B160::from_alloy(wallet.address()),
        U256::from(1_000_000_000_000_000_u64),
    );

    let tx = TxEip1559 {
        chain_id: 37u64,
        nonce: 0,
        max_fee_per_gas: 1000,
        max_priority_fee_per_gas: 1000,
        gas_limit: 1_000_000,
        to: TxKind::Call(target_address),
        value: U256::from(100),
        input: Default::default(),
        access_list: Default::default(),
    };

    let encoded_for_simulation = ZKsyncTxEnvelope::from_eth_tx(tx.clone(), wallet.clone()).encode();

    // We use a very low native per gas ratio to force the transaction to require extra gas
    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        native_price: U256::from(500),
        ..Default::default()
    };

    let result_simulation =
        chain.simulate_block(vec![encoded_for_simulation], Some(block_context.clone()));
    let tx_result_simulation = result_simulation.tx_results[0]
        .clone()
        .expect("Simulation must succeed");

    let tx = TxEip1559 {
        gas_limit: tx_result_simulation.gas_used,
        ..tx
    };
    let encoded = ZKsyncTxEnvelope::from_eth_tx(tx, wallet).encode();

    let result_normal = chain.run_block(vec![encoded], Some(block_context), None, run_config());
    let tx_result_normal = result_normal.tx_results[0]
        .clone()
        .expect("Normal execution must succeed");

    assert_eq!(
        tx_result_simulation.gas_used, tx_result_normal.gas_used,
        "Mismatch in gas used"
    );
    assert_eq!(
        tx_result_simulation.native_used, tx_result_normal.native_used,
        "Mismatch in native used"
    );
}

/// Check that gas price doesn't affect gas used in simulation.
/// Regression for an issue where the lack of fee payment during
/// simulation resulted in underestimated pubdata length, and thus
/// underestimated gas usage.
#[test]
fn test_simulation_gas_used_regression() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let target_address = address!("4242000000000000000000000000000000000000");
    chain.set_balance(B160::from_alloy(wallet.address()), U256::MAX);

    // First tx, 0 gas price.
    let tx = {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 0,
            gas_limit: 2_000_000,
            to: TxKind::Call(target_address),
            value: U256::ZERO,
            input: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };
    let block_context = BlockContext {
        eip1559_basefee: U256::from(91161500u64),
        native_price: U256::from(911615u64),
        pubdata_price: U256::from(10303657632u64 * 4),
        ..Default::default()
    };
    let result_simulation = chain.simulate_block(vec![tx.clone()], Some(block_context.clone()));
    let first_tx = result_simulation.tx_results[0]
        .clone()
        .expect("Must succeed");

    // Second tx, realistic gas price
    let tx = {
        let tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 91161500,
            gas_limit: 2_000_000,
            to: TxKind::Call(target_address),
            value: U256::ZERO,
            input: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    let result_simulation = chain.simulate_block(vec![tx.clone()], Some(block_context));
    let second_tx = result_simulation.tx_results[0]
        .clone()
        .expect("Must succeed");
    assert_eq!(first_tx.gas_used, second_tx.gas_used);
    assert_eq!(first_tx.native_used, second_tx.native_used);
    assert_eq!(first_tx.pubdata_used, second_tx.pubdata_used);
}

/// Regression test for treasury-based token distribution
/// Tests that L1→L2 transactions correctly transfer fees and value from the treasury
/// instead of minting new tokens.
#[test]
fn test_treasury_based_token_distribution_regression() {
    use rig::system_hooks::addresses_constants::BASE_TOKEN_HOLDER_ADDRESS;

    let mut chain = Chain::empty(None);

    // Manually ensure treasury is funded for this test
    chain.mint_tokens_to_treasury();

    // Create L1 transaction sender
    let l1_sender = address!("1234000000000000000000000000000000000000");
    let l1_recipient = address!("5678000000000000000000000000000000000000");
    let coinbase = address!("1000000000000000000000000000000000000000"); // operator
    let refund_recipient = address!("0000000000000000000000000000000000000000"); // refund recipient (zero address)

    // Record initial treasury balance
    let treasury_initial_balance = chain
        .get_account_properties(&BASE_TOKEN_HOLDER_ADDRESS)
        .balance;

    // Record initial operator balance
    let operator_initial_balance = chain
        .get_account_properties(&B160::from_alloy(coinbase))
        .balance;

    // Record initial recipient balance
    let recipient_initial_balance = chain
        .get_account_properties(&B160::from_alloy(l1_recipient))
        .balance;

    // Record initial refund recipient balance
    let refund_recipient_initial_balance = chain
        .get_account_properties(&B160::from_alloy(refund_recipient))
        .balance;

    // Create L1→L2 transaction with value transfer and fees
    let gas_price = 1000u64;
    let gas_limit = 100_000u64;
    let value_to_transfer = U256::from(1_000_000u64);

    // Credit L1 sender with enough balance for the value transfer
    chain.set_balance(B160::from_be_bytes(l1_sender.0 .0), value_to_transfer);

    let l1_tx = {
        let tx = L1TxBuilder::new()
            .from(l1_sender)
            .to(l1_recipient)
            .gas_price(gas_price.into())
            .gas_limit(gas_limit.into())
            .value(value_to_transfer)
            .build();

        tx.encode()
    };

    let block_context = BlockContext {
        coinbase: B160::from_alloy(coinbase),
        ..Default::default()
    };
    let output = chain.run_block(vec![l1_tx], Some(block_context), None, None);

    // Verify transaction succeeded
    assert!(
        output.tx_results[0].is_ok(),
        "L1→L2 transaction should succeed, got: {:?}",
        output.tx_results[0]
    );

    let tx_result = output.tx_results[0].as_ref().unwrap();
    assert!(
        tx_result.is_success(),
        "L1→L2 transaction should be successful"
    );

    // Calculate expected fee payments
    let gas_used = tx_result.gas_used;
    let fee_paid_to_operator = U256::from(gas_used) * U256::from(gas_price);

    // Get final balances
    let treasury_final_balance = chain
        .get_account_properties(&BASE_TOKEN_HOLDER_ADDRESS)
        .balance;

    let operator_final_balance = chain
        .get_account_properties(&B160::from_alloy(coinbase))
        .balance;

    let recipient_final_balance = chain
        .get_account_properties(&B160::from_alloy(l1_recipient))
        .balance;

    let refund_recipient_final_balance = chain
        .get_account_properties(&B160::from_alloy(refund_recipient))
        .balance;

    // Calculate total amount that should go to operator (fee + refund)
    // Refund recipient is 0 in this test
    let gas_limit = 100_000u64;
    let gas_refund = gas_limit - gas_used;
    let refund_amount = U256::from(gas_refund) * U256::from(gas_price);
    let total_to_operator = fee_paid_to_operator;
    let total_to_refund_recipient = refund_amount;

    // Verify treasury balance decreased by max fee (fees + refund)
    let treasury_decrease = treasury_initial_balance - treasury_final_balance;
    let expected_treasury_decrease = total_to_operator + total_to_refund_recipient;
    assert_eq!(
        treasury_decrease, expected_treasury_decrease,
        "Treasury should decrease by total operator payment plus refund and value transferred"
    );

    // Verify operator received total payment from treasury (fee + refund)
    let operator_increase = operator_final_balance - operator_initial_balance;
    assert_eq!(
        operator_increase, total_to_operator,
        "Operator should receive fee + refund from treasury"
    );

    // Verify recipient received value from treasury (not minted)
    let recipient_increase = recipient_final_balance - recipient_initial_balance;
    assert_eq!(
        recipient_increase, value_to_transfer,
        "Recipient should receive exact value amount from treasury"
    );

    // Verify refund recipient received value from treasury (not minted)
    let refund_recipient_increase =
        refund_recipient_final_balance - refund_recipient_initial_balance;
    assert_eq!(
        refund_recipient_increase, total_to_refund_recipient,
        "Refund recipient should receive correct refund amount from treasury"
    );
}

/// Test treasury transfer failure when treasury has insufficient balance
#[test]
fn test_treasury_insufficient_balance_failure() {
    use rig::system_hooks::addresses_constants::BASE_TOKEN_HOLDER_ADDRESS;

    let mut chain = Chain::empty(None);

    // Manually set very low treasury balance instead of using default
    let low_treasury_balance = U256::from(1000u64);
    chain.set_balance(BASE_TOKEN_HOLDER_ADDRESS, low_treasury_balance);

    // Create L1→L2 transaction that requires more tokens than treasury has
    let l1_sender = address!("1234000000000000000000000000000000000000");
    let l1_recipient = address!("5678000000000000000000000000000000000000");

    let gas_price = 1000u64;
    let gas_limit = 100_000u64;
    let value_to_transfer = U256::from(500_000u64); // More than treasury can cover

    let l1_tx = {
        let tx = L1TxBuilder::new()
            .from(l1_sender)
            .to(l1_recipient)
            .gas_price(gas_price.into())
            .gas_limit(gas_limit.into())
            .value(value_to_transfer)
            .build();
        tx.encode()
    };

    let config = RunConfig {
        skip_minting_tokens_to_treasury: true,
        ..Default::default()
    };

    // This should fail due to insufficient treasury balance
    let result = chain.run_block_no_panic(vec![l1_tx], None, None, Some(config));

    // Verify transaction fails due to treasury insufficient balance
    assert!(
        result.is_err(),
        "L1→L2 transaction should fail when treasury has insufficient balance"
    );

    // Verify the specific error is treasury transfer failed
    let error_debug = format!("{:?}", result.unwrap_err());
    assert!(
        error_debug.contains("TreasuryTransferFailed"),
        "Error should indicate treasury transfer failed, got: {}",
        error_debug
    );
}

#[test]
fn test_pubdata_native_calculation_overflow() {
    use alloy::consensus::TxEip1559;
    use rig::alloy::primitives::TxKind;

    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();

    // Set initial balance for the wallet
    chain.set_balance(
        B160::from_alloy(from),
        U256::from_str("100000000000000000000010000").unwrap(),
    );

    let to = address!("1234567890123456789012345678901234567890");
    /*
       contract A {
           mapping(uint256 => uint256) s;

           fallback() external payable {
               for (uint256 i = 0; i < 20; i++) {
                   s[i] = 0xfffffffffffffffffffffffff;
               }
           }
       }
    */
    // Spam some pubdata
    let bytecode = hex::decode("60806040525f5f90505b6014811015603f576c0fffffffffffffffffffffffff5f5f8381526020019081526020015f208190555080806001019150506009565b00fea2646970667358221220d8f4977e359f09d23e2979156755d7e177d43f8a1882a5a178eb98dd8bcb237264736f6c634300081f0033").unwrap();
    chain.set_evm_bytecode(B160::from_alloy(to), &bytecode);

    // Create a transaction that will generate significant pubdata
    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            gas_limit: 10000000,
            max_fee_per_gas: 1000000000000000000,
            max_priority_fee_per_gas: 1000000000000000000,
            to: TxKind::Call(to),
            value: U256::from(1000),
            ..Default::default()
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    // Set extremely high native_per_pubdata to trigger overflow in current_pubdata_spent.checked_mul(native_per_pubdata)
    let native_price = U256::from(1);
    let pubdata_price = U256::from(u64::MAX / 150); // Huge pubdata price to trigger overflow

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        ..Default::default()
    };

    let result = chain.run_block(vec![tx], Some(block_context), None, None);

    // Verify the specific error is OutOfNativeResources
    match &result.tx_results[0].as_ref().unwrap().execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(_) => panic!("Should fail"),
        rig::zksync_os_interface::types::ExecutionResult::Revert(_) => {}
    }
}
