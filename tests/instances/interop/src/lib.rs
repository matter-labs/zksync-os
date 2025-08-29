//!
//! These tests are focused on interop support in ZKsync OS
//!
#![cfg(test)]

use alloy::consensus::TxLegacy;
use alloy::primitives::TxKind;
use alloy::signers::local::PrivateKeySigner;
use rig::alloy::primitives::address;
use rig::ruint::aliases::{B160, U256};
use rig::utils::{ERC_20_BYTECODE, ERC_20_MINT_CALLDATA};
use rig::zk_ee::common_structs::interop_root::InteropRoot;
use rig::zk_ee::utils::Bytes32;
use rig::{alloy, zksync_web3_rs, BlockContext, Chain};
use std::fs;
use std::str::FromStr;
use zksync_web3_rs::signers::{LocalWallet, Signer};

pub const L2_INTEROP_ROOT_STORAGE_ADDRESS_LOW: u32 = 0x10008;
pub const L2_INTEROP_ROOT_STORAGE_ADDRESS: B160 =
    B160::from_limbs([L2_INTEROP_ROOT_STORAGE_ADDRESS_LOW as u64, 0, 0]);

#[test]
fn run_processes_one_interop_root() {
    let mut chain = Chain::empty(None);
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let dummy_l2_interop_roots_storage_bytecode =
        fs::read_to_string("dummy_interop_root_storage.txt").expect("Should read bytecode");
    let bytecode = hex::decode(dummy_l2_interop_roots_storage_bytecode).unwrap();
    chain.set_evm_bytecode(L2_INTEROP_ROOT_STORAGE_ADDRESS, &bytecode);

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
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let mut block_context = BlockContext::default();

    let mut interops_roots = Vec::new();

    // Create some dummy interop root
    interops_roots.push(InteropRoot {
        root: Bytes32::from_u256_be(&U256::ONE),
        block_or_batch_number: 42,
        chain_id: 1,
    });

    block_context.interop_roots = interops_roots;

    chain.run_block_with_extra_stats(
        vec![encoded_mint_tx],
        Some(block_context),
        None,
        None,
        Some("server_app_logging_enabled".to_owned()),
    );
}

#[test]
#[should_panic(expected = "Forward run failed with: FailedToSetInteropRoots")]
fn run_fails_if_interop_root_is_incorrect() {
    let mut chain = Chain::empty(None);
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let dummy_l2_interop_roots_storage_bytecode =
        fs::read_to_string("dummy_interop_root_storage.txt").expect("Should read bytecode");
    let bytecode = hex::decode(dummy_l2_interop_roots_storage_bytecode).unwrap();
    chain.set_evm_bytecode(L2_INTEROP_ROOT_STORAGE_ADDRESS, &bytecode);

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
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let mut block_context = BlockContext::default();

    let mut interops_roots = Vec::new();

    // Create some dummy interop root
    interops_roots.push(InteropRoot {
        root: Bytes32::zero(), // Root can't be zero
        block_or_batch_number: 42,
        chain_id: 1,
    });

    block_context.interop_roots = interops_roots;

    chain.run_block_with_extra_stats(vec![encoded_mint_tx], Some(block_context), None, None, None);
}
