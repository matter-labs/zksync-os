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
use rig::zk_ee::system::tracer::NopTracer;
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_interface::traits::EncodedTx;
use rig::{alloy, zksync_web3_rs, BlockContext, Chain};
use std::fs;
use std::str::FromStr;
use zksync_web3_rs::signers::{LocalWallet, Signer};

/// Test constants for L2 interop root storage contract address
pub const L2_INTEROP_ROOT_STORAGE_ADDRESS_LOW: u32 = 0x10008;
pub const L2_INTEROP_ROOT_STORAGE_ADDRESS: B160 =
    B160::from_limbs([L2_INTEROP_ROOT_STORAGE_ADDRESS_LOW as u64, 0, 0]);

#[test]
fn run_processes_one_interop_root() {
    let (mut chain, encoded_mint_tx) = prepare_chain();

    let mut interops_roots = Vec::new();
    // Create some dummy interop root
    interops_roots.push(InteropRoot {
        root: Bytes32::from_u256_be(&U256::ONE),
        block_or_batch_number: 42,
        chain_id: 1,
    });

    run_tx_with_interop_roots(&mut chain, encoded_mint_tx, interops_roots);
}

#[test]
#[should_panic(expected = "FailedToSetInteropRoots")]
fn run_fails_if_interop_root_is_incorrect() {
    let (mut chain, encoded_mint_tx) = prepare_chain();

    let mut interops_roots = Vec::new();
    // Create some dummy interop root
    interops_roots.push(InteropRoot {
        root: Bytes32::zero(), // Root can't be zero
        block_or_batch_number: 42,
        chain_id: 1,
    });

    run_tx_with_interop_roots(&mut chain, encoded_mint_tx, interops_roots);
}

#[test]
fn run_processes_several_interop_roots() {
    let (mut chain, encoded_mint_tx) = prepare_chain();

    let mut interop_roots = Vec::new();
    // Create several interop roots to test batch processing and resource costs
    for i in 1..=20 {
        interop_roots.push(InteropRoot {
            root: Bytes32::from_u256_be(&U256::from(0x1000 + i)),
            block_or_batch_number: 100 + i,
            chain_id: i, // Use different chain IDs
        });
    }

    run_tx_with_interop_roots(&mut chain, encoded_mint_tx, interop_roots);
}

#[test]
fn run_processes_empty_interop_roots() {
    let (mut chain, encoded_mint_tx) = prepare_chain();

    run_tx_with_interop_roots(&mut chain, encoded_mint_tx, vec![]);
}

#[test]
fn run_processes_interop_roots_max_amount() {
    let (mut chain, encoded_mint_tx) = prepare_chain();

    let mut interop_roots = Vec::new();

    // Edge case: Maximum values
    interop_roots.push(InteropRoot {
        root: Bytes32::from_u256_be(&U256::MAX),
        block_or_batch_number: u64::MAX,
        chain_id: u64::MAX,
    });

    // Edge case: Minimum valid values (chain_id = 1, block = 0)
    interop_roots.push(InteropRoot {
        root: Bytes32::from_u256_be(&U256::from(1)),
        block_or_batch_number: 0,
        chain_id: 1,
    });

    // Edge case: Large root hash with small numbers
    interop_roots.push(InteropRoot {
        root: Bytes32::from_u256_be(&(U256::MAX - U256::from(1))),
        block_or_batch_number: 1,
        chain_id: 1,
    });

    run_tx_with_interop_roots(&mut chain, encoded_mint_tx, interop_roots);
}

/// Sets up test chain with ERC-20 contract and interop root storage contract
fn prepare_chain() -> (Chain, EncodedTx) {
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
        fs::read_to_string("dummy_interop_root_storage_v30.txt").expect("Should read bytecode");
    let bytecode = hex::decode(dummy_l2_interop_roots_storage_bytecode).unwrap();
    chain.set_evm_bytecode(L2_INTEROP_ROOT_STORAGE_ADDRESS, &bytecode);

    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 100_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    (chain, encoded_mint_tx)
}

/// Executes a transaction with specified interop roots and verifies success
fn run_tx_with_interop_roots(
    chain: &mut Chain,
    encoded_tx: EncodedTx,
    interop_roots: Vec<InteropRoot>,
) {
    let mut block_context = BlockContext::default();
    block_context.interop_roots = interop_roots;

    let mut tracer = NopTracer::default();

    let (output, _, _) = chain
        .run_block_with_extra_stats(
            vec![encoded_tx],
            Some(block_context),
            None,
            None,
            &mut tracer,
        )
        .expect("Block should run successfully");

    // Verify the transaction succeeded
    assert_eq!(output.tx_results.len(), 1);
    assert!(
        output.tx_results[0].is_ok(),
        "Transaction should succeed with edge case interop root values"
    );
}
