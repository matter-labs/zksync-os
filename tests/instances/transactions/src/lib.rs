//!
//! These tests are focused on different tx types.
//!
#![cfg(test)]
use alloy::consensus::{TxEip1559, TxEip2930, TxEip4844, TxLegacy};
use alloy::primitives::TxKind;
use rig::alloy::consensus::TxEip7702;
use rig::alloy::primitives::{address, b256};
use rig::alloy::rpc::types::{AccessList, AccessListItem, TransactionRequest};
use rig::basic_bootloader::bootloader::block_flow::zk::PUBDATA_ENCODING_VERSION;
use rig::forward_system::run::convert_alloy::{FromAlloy, IntoAlloy};
use rig::ruint::aliases::{B160, U256};
use rig::system_hooks::addresses_constants::L2_INTEROP_ROOT_STORAGE_ADDRESS;
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::{alloy, common_target_address, testing_signer, TestingFramework};
use rig::{utils::*, BlockContext};
use std::str::FromStr;
use zksync_os_tests_common::zksync_tx::service_tx::ZKsyncServiceTx;
use zksync_os_tests_common::zksync_tx::upgrade_tx::ZKsyncUpgradeTx;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

mod l1_tx_resilience;
mod native_charging;

#[test]
fn run_base_system() {
    // FIXME: this address looks very similar to bridgehub/shared bridge on gateway.
    // Which seems to suggest that it is special.
    // Consider changing this one to be more "random".

    let wallet = testing_signer(0);

    // We used for test where from cannot have deployed code
    let eoa_wallet = testing_signer(1);

    let to = address!("0000000000000000000000000000000000010002");

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

    // `to` == null
    let deployment_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(deployment_tx, wallet.clone())
    };
    let transfer_to_eoa_tx = {
        let eoa_to = common_target_address();
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
        ZKsyncTxEnvelope::from_eth_tx(transfer_to_eoa, eoa_wallet.clone())
    };

    let mint2_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let l1_l2_transfer = {
        L1TxBuilder::new()
            .from(address!("1234000000000000000000000000000000000000"))
            .to(common_target_address())
            .value(alloy::primitives::U256::from(100))
            .gas_price(1000)
            .gas_limit(21_000)
            .build()
            .into()
    };

    let l1_l2_erc_transfer = {
        L1TxBuilder::new()
            .from(wallet.address())
            .to(to)
            .input(hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into())
            .gas_price(1000)
            .gas_limit(40_000)
            .nonce(3)
            .build()
            .into()
    };

    let transactions = vec![
        mint_tx,
        transfer_tx,
        deployment_tx,
        transfer_to_eoa_tx,
        mint2_tx,
        l1_l2_transfer,
        l1_l2_erc_transfer,
    ];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_prefunded_account(wallet.address())
        .with_prefunded_account(eoa_wallet.address())
        .with_balance(
            address!("1234000000000000000000000000000000000000"),
            U256::from(100),
        );

    let output = tester.execute_block(transactions);

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
    let mut tester = TestingFramework::new_with_randomized_tree();
    tester.run_block_of_erc20(10, None);
}

#[test]
fn test_gas_price_zero_fee_zero() {
    let mut tester = TestingFramework::new_with_randomized_tree();
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..BlockContext::default()
    };
    let output = tester.run_block_of_erc20_with_fee(10, Some(block_context), 0);
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
    let mut tester = TestingFramework::new_with_randomized_tree();
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..BlockContext::default()
    };
    tester.run_block_of_erc20_with_fee(10, Some(block_context), 1);
}

#[test]
fn test_withdrawal() {
    let wallet = testing_signer(0);

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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let transactions = vec![withdrawal_tx, withdrawal_with_message_tx];

    let mut tester = TestingFramework::new()
        .with_system_contracts(true, true, false)
        .with_prefunded_account(wallet.address());

    let output = tester.execute_block(transactions);

    tester.assert_all_txs_succeeded(&output);

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
    let wallet = testing_signer(0);

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let mint_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let transactions = vec![mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_prefunded_account(wallet.address());

    let output = tester.execute_block(transactions);

    tester.assert_all_txs_succeeded(&output);
}

#[test]
fn test_tx_with_authorization_list() {
    use rig::alloy::eips::eip7702::*;
    use rig::alloy::signers::SignerSync;

    let wallet = testing_signer(0);

    let delegate = testing_signer(1);

    let to = delegate.address();

    let erc_20_contract = address!("0000000000000000000000000000000000010002");

    let mint_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    let mut tester = TestingFramework::new()
        .with_evm_contract(erc_20_contract, &bytecode)
        .with_prefunded_account(wallet.address());

    let output = tester.execute_block(vec![mint_tx]);

    tester.assert_all_txs_succeeded(&output);
}

// Test that slots made warm in a tx are cold in the next tx
#[test]
fn test_cold_in_new_tx() {
    let wallet = testing_signer(0);

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 68_358,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    // Gas is just enough to succeed.
    let mint_tx_1 = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 1,
            gas_price: 1000,
            gas_limit: 34158,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    // Any lower gas amount should fail
    let mint_tx_2 = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 2,
            gas_price: 1000,
            gas_limit: 34158 - 1,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let transactions = vec![mint_tx, mint_tx_1, mint_tx_2];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_prefunded_account(wallet.address());

    let output = tester.execute_block(transactions);

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
    let wallet1 = testing_signer(0);

    let wallet2 = testing_signer(2);

    let to1 = address!("0000000000000000000000000000000000010002");
    let to2 = address!("0000000000000000000000000000000000010003");

    let tx_1 = {
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet1.clone())
    };

    let tx_2 = {
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet2.clone())
    };

    let transactions = vec![tx_1, tx_2];

    let mut tester = TestingFramework::new()
        .with_balance(wallet1.address(), U256::from(1_000_000_000_000_000_u64))
        .with_balance(wallet2.address(), U256::from(1_000_000_000_000_000_u64));

    let output = tester.execute_block(transactions);

    // Assert all txs succeeded and compare pubdata len
    tester.assert_all_txs_succeeded(&output);

    let result1 = output.tx_results.first().unwrap().clone();
    let result2 = output.tx_results.get(1).unwrap().clone();
    let pubdata_used_1 = result1.unwrap().pubdata_used;
    let pubdata_used_2 = result2.unwrap().pubdata_used;
    assert_eq!(pubdata_used_1, pubdata_used_2, "Pubdata used not equal")
}

#[test]
fn test_invalid_tx_does_not_bump_tx_counter() {
    let wallet = testing_signer(0);
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    let l1_messenger_contract = address!("0000000000000000000000000000000000008008");
    let l1_messenger_hook = address!("0000000000000000000000000000000000007001");

    // Invalid tx first
    let mint_tx_1 = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 34_158_000_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };
    let withdrawal_tx = {
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

        tx.into()
    };

    let transactions = vec![mint_tx_1, withdrawal_tx];

    let mut tester = TestingFramework::new()
        .with_balance(l1_messenger_contract, U256::from(1_000_000_000_000_000_u64))
        .with_evm_contract(to, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));
    let output = tester.execute_block(transactions);

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
    let wallet = testing_signer(0);
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    // First we run a single tx to get the "normal" amount of native used
    let mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let transactions = vec![mint_tx.clone()];
    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

    let output = tester.execute_block(transactions);

    // Assert tx succeeded
    tester.assert_all_txs_succeeded(&output);

    let native_used_reference = output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .unwrap()
        .native_used;

    // Same tx but with a huge gas limit, which makes it invalid
    // We run this one first and then the valid one, and check that
    // the valid one uses the same amount of native as in the reference case.
    let mint_tx_1 = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 34_158_000_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));
    let transactions = vec![mint_tx_1, mint_tx];
    let output = tester.execute_block(transactions);

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
    let wallet = testing_signer(0);
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

    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(BYTECODE).unwrap();
    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let tx_envelope = {
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 1_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            ..Default::default()
        };
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let transactions = vec![tx_envelope];

    let output = tester.execute_block(transactions);

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

#[test]
fn test_returndata_cleared_when_reverted_after_execution() {
    let wallet = testing_signer(0);
    let from = wallet.address();
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(
        "602a600052600160005560016001556001600255600160035560016004556001600555600160065560016007556001600855600160095560206000f3",
    )
    .unwrap();

    let make_tx = |nonce: u64| {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 250_000,
            to: TxKind::Call(to),
            value: U256::ZERO,
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    // First run with regular block context: call succeeds and returns non-empty data.
    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(from, U256::from(1_000_000_000_000_000_u64));
    let success_output = tester.execute_block(vec![make_tx(0)]);
    let success_tx = success_output.tx_results[0]
        .as_ref()
        .expect("Control transaction should be processed");
    assert!(
        success_tx.is_success(),
        "Control transaction should succeed, got: {:?}",
        success_output.tx_results[0]
    );
    match &success_tx.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(
            rig::zksync_os_interface::types::ExecutionOutput::Call(output),
        ) => {
            assert!(
                !output.is_empty(),
                "Control transaction should return non-empty returndata"
            );
        }
        other => panic!("Unexpected control execution result: {other:?}"),
    }

    // Run the same tx with expensive pubdata so post-execution pubdata check forces a revert.
    // Regression: such reverts must not keep the successful call returndata.
    let expensive_pubdata_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        native_price: U256::ONE,
        pubdata_price: U256::from(700_000u64),
        ..Default::default()
    };
    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &bytecode)
        .with_balance(from, U256::from(1_000_000_000_000_000_u64))
        .with_block_context(expensive_pubdata_context);
    let reverted_output = tester.execute_block(vec![make_tx(0)]);
    let reverted_tx = reverted_output.tx_results[0]
        .as_ref()
        .expect("Transaction should be processed even if reverted");
    assert!(
        !reverted_tx.is_success(),
        "Transaction should be reverted by pubdata check, got: {:?}",
        reverted_output.tx_results[0]
    );
    match &reverted_tx.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Revert(output) => {
            assert!(
                output.is_empty(),
                "Returndata must be cleared when converting success into revert"
            );
        }
        other => panic!("Expected revert result, got: {other:?}"),
    }
}

/// Test that transactions with balance calculation overflow are properly rejected
#[test]
fn test_balance_overflow_protection() {
    let wallet = testing_signer(0);

    let to = address!("0000000000000000000000000000000000010002");
    let mut tester = TestingFramework::new()
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let output = tester.execute_block(vec![overflow_fee_tx, overflow_total_tx]);

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
    // Create a contract that always reverts
    let revert_contract_address = address!("0000000000000000000000000000000000010003");
    // Simple contract bytecode that just does REVERT(0, 0)
    let revert_bytecode = hex::decode("60006000fd").unwrap(); // PUSH1 0, PUSH1 0, REVERT
    let mut tester =
        TestingFramework::new().with_evm_contract(revert_contract_address, &revert_bytecode);

    // Create a proper upgrade transaction that calls the reverting contract

    let upgrade_tx = ZKsyncTxEnvelope::from(ZKsyncUpgradeTx {
        from: address!("1234000000000000000000000000000000000000"),
        to: revert_contract_address,
        gas_limit: 100_000u128,
        ..Default::default()
    });

    let transactions = vec![upgrade_tx];

    // Use execute_block_no_panic to catch the error instead of panicking
    let result = tester.execute_block_no_panic(transactions);

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
    // Create a contract that always succeeds
    let contract_address = address!("0000000000000000000000000000000000010003");
    // Simple contract bytecode that just does RETURN(0, 0)
    let bytecode = hex::decode("60006000f3").unwrap(); // PUSH1 0, PUSH1 0, RETURN
    let mut tester = TestingFramework::new().with_evm_contract(contract_address, &bytecode);

    // Create a proper upgrade transaction that calls the contract
    let upgrade_tx = ZKsyncTxEnvelope::from(ZKsyncUpgradeTx {
        from: address!("1234000000000000000000000000000000000000"),
        to: contract_address,
        gas_limit: 100_000u128,
        ..Default::default()
    });

    let transactions = vec![upgrade_tx];

    // Use execute_block_no_panic to catch the error instead of panicking
    let result = tester.execute_block_no_panic(transactions);
    assert!(result.is_ok());
    let tx_output = result.as_ref().unwrap().tx_results[0].as_ref().unwrap();
    assert!(tx_output.is_success());
    // make sure that it didn't produce l1 log with upgrade tx result
    // (such logs were present in the past, but we removed them)
    assert!(tx_output.l2_to_l1_logs.is_empty());
}

#[test]
fn test_invalid_transaction_type_failure() {
    // Create a simple success contract for the call
    let contract_address = address!("0000000000000000000000000000000000010003");
    let success_bytecode = hex::decode("60006000f3").unwrap(); // PUSH1 0, PUSH1 0, RETURN
    let mut tester = TestingFramework::new().with_evm_contract(contract_address, &success_bytecode);

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

        let transactions = vec![invalid_tx];
        let result = tester.execute_block(transactions);
        assert!(
            result.tx_results[0].is_err(),
            "Transaction with invalid type should fail"
        );
    }
}

#[test]
fn test_modexp_intermediate_zero_block() {
    let wallet = testing_signer(0);
    let mut tester =
        TestingFramework::new().with_balance(wallet.address(), U256::from(10u64.pow(18)));

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

    let tx_envelope = {
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let transactions = vec![tx_envelope];

    let result = tester.execute_block(transactions);

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
    let wallet = testing_signer(0);
    let mut tester =
        TestingFramework::new().with_balance(wallet.address(), U256::from(10u64.pow(18)));

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

    let tx_envelope = {
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
        ZKsyncTxEnvelope::from_eth_tx(mint_tx, wallet.clone())
    };

    let transactions = vec![tx_envelope];

    let result = tester.execute_block(transactions);

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

    let mut tester = TestingFramework::new();
    let wallet = tester.prefunded_random_signer();

    let contract_address = address!("1000000000000000000000000000000000000001");

    // PUSH20 0x01
    // SELFDESTRUCT
    let bytecode = hex::decode("730000000000000000000000000000000000000001ff").unwrap();

    tester = tester.with_evm_contract(contract_address, &bytecode);

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

    let result = tester.execute_block(vec![tx_request]);
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
    let gas_used = res0.clone().unwrap().gas_used;
    assert_eq!(gas_used, 26003);
}

#[test]
fn test_reject_caller_with_code_behavior() {
    let mut tester = TestingFramework::new();
    let wallet = tester.prefunded_random_signer();

    // Create a contract address with bytecode deployed
    let contract_address = wallet.address();
    let target_address = common_target_address();

    // Deploy bytecode to the contract address to make it a "contract with code"
    tester.set_evm_contract(
        contract_address,
        &hex::decode("60006000f3").unwrap(), // Simple contract: PUSH1 0, PUSH1 0, RETURN
    );

    // Set balance for the contract address
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let result_simulation = tester.simulate_block(vec![from_contract_tx.clone()]);

    // In simulation mode, the transaction should succeed
    assert!(result_simulation.tx_results[0].is_ok(),);

    let tx_result = result_simulation.tx_results[0].as_ref().unwrap();
    assert!(
        tx_result.is_success(),
        "Transaction should be successful in simulation mode"
    );

    // But in normal mode it should fail
    let result_normal = tester.execute_block(vec![from_contract_tx]);
    assert!(matches!(
        result_normal.tx_results[0],
        Err(InvalidTransaction::RejectCallerWithCode)
    ));
}

#[test]
fn test_expensive_pubdata() {
    // Test if a transaction can be executed even if the pubdata price is such that
    // validation pubdata requires to use withheld resources.
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let target_address = common_target_address();

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
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
    tester = tester
        .with_balance(wallet.address(), U256::from(u64::MAX))
        .with_block_context(block_context);
    // Check tx succeeds
    let result = tester.execute_block(vec![tx]);
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
}

#[test]
fn test_check_pubdata_encoding_version() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let target_address = common_target_address();

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    let native_price = U256::from(100);
    let pubdata_price = U256::from(2);

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        ..Default::default()
    };
    tester = tester
        .with_balance(wallet.address(), U256::from(u64::MAX))
        .with_block_context(block_context);
    // Check tx succeeds
    let result = tester.execute_block(vec![tx]);
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");

    assert_eq!(result.pubdata[0], PUBDATA_ENCODING_VERSION);
}

#[test]
fn test_check_pubdata_has_timestamp() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let target_address = common_target_address();

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
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
    tester = tester
        .with_balance(wallet.address(), U256::from(u64::MAX))
        .with_block_context(block_context);
    // Check tx succeeds
    let result = tester.execute_block(vec![tx]);
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
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    let tx = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    });

    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    let mut tester = TestingFramework::new().with_block_context(block_context);
    let result = tester.execute_block(vec![tx]);
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
}

#[test]
fn test_simple_service_transaction_whitelist() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    // Invalid target
    let target_address = [0u8; 20];

    let tx = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    });

    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    tester = tester
        .with_balance(wallet.address(), U256::from(u64::MAX))
        .with_block_context(block_context);
    // Check tx succeeds
    let result = tester.execute_block(vec![tx]);
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_err(), "Tx should fail");
}

#[test]
fn test_service_tx_gas_limit_exceeds_block() {
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    let tx = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    });

    let block_context = BlockContext {
        gas_limit: 30_000_000,
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    let mut tester = TestingFramework::new().with_block_context(block_context);
    let result = tester.execute_block(vec![tx]);
    assert!(result.tx_results[0].is_ok());
}

#[test]
fn test_service_block_invariants() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    tester = tester.with_balance(wallet.address(), U256::from(u64::MAX));

    // Check that a service block with several service txs works
    let tx1 = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 0,
    });
    let tx2 = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 1,
    });
    let tx3 = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 2,
    });

    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    tester = tester.with_block_context(block_context);
    // Check txs succeed
    let result = tester.execute_block(vec![tx1, tx2, tx3]);
    assert!(
        result.tx_results.iter().all(|res| res.is_ok()),
        "All txs should succeed"
    );

    // Check that a service block with a non-service tx fails
    let tx4 = ZKsyncTxEnvelope::from(ZKsyncServiceTx {
        to: alloy::primitives::Address::from_slice(&target_address),
        input: Default::default(),
        salt: 3,
    });
    let tx_non_service = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(common_target_address()),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        ..Default::default()
    };
    tester = tester.with_block_context(block_context);
    tester
        .execute_block_no_panic(vec![tx4.clone(), tx_non_service.clone()])
        .expect_err("Service block with non service tx should fail");

    // Check that a non-service block with a service tx fails
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };
    tester = tester.with_block_context(block_context);
    tester
        .execute_block_no_panic(vec![tx_non_service, tx4])
        .expect_err("Service block with non service tx should fail");
}

/// Regression test for: Skip nonce check on simulation
#[test]
fn test_simulation_skips_nonce_check() {
    let mut tester = TestingFramework::new();
    let wallet = tester.prefunded_random_signer();
    let target_address = common_target_address();

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    // In simulation mode, the transaction should succeed (nonce check skipped)
    let result_simulation = tester.simulate_block(vec![tx.clone()]);
    assert!(
        result_simulation.tx_results[0].is_ok(),
        "Transaction should pass validation in simulation mode, got: {:?}",
        result_simulation.tx_results[0]
    );

    // In normal execution mode, the transaction should fail with NonceTooHigh
    let result_normal = tester.execute_block(vec![tx]);
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
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let target_address = common_target_address();

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    let result_simulation = tester.simulate_block(vec![tx.clone()]);
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    let result_simulation = tester.simulate_block(vec![tx.clone()]);
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    let result_simulation = tester.simulate_block(vec![tx.clone()]);
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    let result_simulation = tester.simulate_block(vec![tx.clone()]);
    assert!(
        result_simulation.tx_results[0].is_ok(),
        "Transaction with no fee/value should pass validation in simulation mode"
    );
}

#[test]
fn test_simulation_4844_zero_blob_fee_allowed() {
    let mut tester = TestingFramework::new();
    let wallet = tester.prefunded_random_signer();
    let target_address = common_target_address();

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
    let tx_envelope = ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone());

    let block_context = BlockContext {
        blob_fee: U256::from(1),
        ..Default::default()
    };

    tester = tester.with_block_context(block_context);
    let result_simulation = tester.simulate_block(vec![tx_envelope]);
    assert!(
        result_simulation.tx_results[0].is_ok(),
        "EIP-4844 tx should pass simulation when blob_fee > 0 and max_fee_per_blob_gas = 0, got: {:?}",
        result_simulation.tx_results[0]
    );
}

/// Check that gas and native used is the same in simulation and actual execution
#[test]
fn test_simulation_gas_and_native_used() {
    let mut tester = TestingFramework::new();
    let wallet = tester.prefunded_random_signer();
    let target_address = common_target_address();

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

    let tx_for_simulation = ZKsyncTxEnvelope::from_eth_tx(tx.clone(), wallet.clone());

    // We use a very low native per gas ratio to force the transaction to require extra gas
    let block_context = BlockContext {
        eip1559_basefee: U256::from(1000),
        native_price: U256::from(500),
        ..Default::default()
    };

    tester = tester.with_block_context(block_context.clone());
    let result_simulation = tester.simulate_block(vec![tx_for_simulation]);
    let tx_result_simulation = result_simulation.tx_results[0]
        .clone()
        .expect("Simulation must succeed");

    let tx = TxEip1559 {
        gas_limit: tx_result_simulation.gas_used,
        ..tx
    };
    let encoded = ZKsyncTxEnvelope::from_eth_tx(tx, wallet);

    tester = tester.with_block_context(block_context);
    let result_normal = tester.execute_block(vec![encoded]);
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
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    let target_address = common_target_address();
    tester = tester.with_balance(wallet.address(), U256::MAX);

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };
    let block_context = BlockContext {
        eip1559_basefee: U256::from(91161500u64),
        native_price: U256::from(911615u64),
        pubdata_price: U256::from(10303657632u64 * 4),
        ..Default::default()
    };
    tester = tester.with_block_context(block_context.clone());
    let result_simulation = tester.simulate_block(vec![tx.clone()]);
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    tester = tester.with_block_context(block_context);
    let result_simulation = tester.simulate_block(vec![tx.clone()]);
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

    let mut tester = TestingFramework::new();

    // Manually ensure treasury is funded for this test
    tester.mint_tokens_to_treasury();

    // Create L1 transaction sender
    let l1_sender = address!("1234000000000000000000000000000000000000");
    let l1_recipient = address!("5678000000000000000000000000000000000000");
    let coinbase = address!("1000000000000000000000000000000000000000"); // operator
    let refund_recipient = address!("0000000000000000000000000000000000000000"); // refund recipient (zero address)

    // Record initial treasury balance
    let treasury_initial_balance = tester.get_balance(&BASE_TOKEN_HOLDER_ADDRESS.into_alloy());

    // Record initial operator balance
    let operator_initial_balance = tester.get_balance(&coinbase);

    // Record initial recipient balance
    let recipient_initial_balance = tester.get_balance(&l1_recipient);

    // Record initial refund recipient balance
    let refund_recipient_initial_balance = tester.get_balance(&refund_recipient);

    // Create L1→L2 transaction with value transfer and fees
    let gas_price = 1000u64;
    let gas_limit = 100_000u64;
    let value_to_transfer = U256::from(1_000_000u64);

    // Credit L1 sender with enough balance for the value transfer
    tester = tester.with_balance(l1_sender, value_to_transfer);

    let l1_tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(l1_sender)
        .to(l1_recipient)
        .gas_price(gas_price.into())
        .gas_limit(gas_limit.into())
        .value(value_to_transfer)
        .build()
        .into();

    let block_context = BlockContext {
        coinbase: B160::from_alloy(coinbase),
        ..Default::default()
    };
    tester = tester.with_block_context(block_context);
    let output = tester.execute_block(vec![l1_tx]);

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
    let treasury_final_balance = tester.get_balance(&BASE_TOKEN_HOLDER_ADDRESS.into_alloy());

    let operator_final_balance = tester.get_balance(&coinbase);

    let recipient_final_balance = tester.get_balance(&l1_recipient);

    let refund_recipient_final_balance = tester.get_balance(&refund_recipient);

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

    // Manually set very low treasury balance instead of using default
    let low_treasury_balance = U256::from(1000u64);
    let mut tester = TestingFramework::new()
        .with_balance(BASE_TOKEN_HOLDER_ADDRESS.into_alloy(), low_treasury_balance);

    // Create L1→L2 transaction that requires more tokens than treasury has
    let l1_sender = address!("1234000000000000000000000000000000000000");
    let l1_recipient = address!("5678000000000000000000000000000000000000");

    let gas_price = 1000u64;
    let gas_limit = 100_000u64;
    let value_to_transfer = U256::from(500_000u64); // More than treasury can cover

    let l1_tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(l1_sender)
        .to(l1_recipient)
        .gas_price(gas_price.into())
        .gas_limit(gas_limit.into())
        .value(value_to_transfer)
        .build()
        .into();

    let mut config: rig::chain::RunConfig = Default::default();
    config.skip_minting_tokens_to_treasury = true; // Ensure we rely on treasury balance, not minting

    // This should fail due to insufficient treasury balance
    tester = tester.with_run_config(config);
    let result = tester.execute_block_no_panic(vec![l1_tx]);

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

    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();

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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
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
    tester = tester
        .with_balance(
            wallet.address(),
            U256::from_str("100000000000000000000010000").unwrap(),
        )
        .with_evm_contract(to, &bytecode)
        .with_block_context(block_context);
    let result = tester.execute_block(vec![tx]);

    // Verify the specific error is OutOfNativeResources
    match &result.tx_results[0].as_ref().unwrap().execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(_) => panic!("Should fail"),
        rig::zksync_os_interface::types::ExecutionResult::Revert(_) => {}
    }
}
