//!
//! Tests for the L2AssetTracker.handleFinalizeBaseTokenBridgingOnL2 calls
//! that the bootloader makes during L1 transaction processing.
//!
//! When an L1 transaction deposits base tokens (total_deposited > 0), the
//! bootloader calls handleFinalizeBaseTokenBridgingOnL2(uint256, uint256)
//! on the real L2AssetTracker contract up to three times — once for the
//! value mint, once for the operator fee, and once for the refund. If any
//! of these amounts is zero the corresponding call is skipped.
//!
//! When the source chain matches `L1_CHAIN_ID` and the current settlement
//! layer also matches `L1_CHAIN_ID`, the contract records the aggregate
//! bridged amount in `interopInfo[BASE_TOKEN_ASSET_ID].totalSuccessfulDepositsFromL1`.

use alloy_sol_types::sol;
use alloy_sol_types::SolCall;
use rig::alloy::consensus::TxLegacy;
use rig::alloy::primitives::{address, TxKind};
use rig::crypto::MiniDigest;
use rig::evm_bytecode::BytecodeBuilder;
use rig::forward_system::run::convert_alloy::IntoAlloy;
use rig::predeployed_contracts::{
    DEFAULT_BASE_TOKEN_ASSET_ID, L2_ASSET_TRACKER_L1_CHAIN_ID_SLOT,
    SYSTEM_CONTEXT_SETTLEMENT_LAYER_CHAIN_ID_SLOT,
};
use rig::ruint::aliases::{B256, U256};
use rig::system_hooks::addresses_constants::{L2_ASSET_TRACKER_ADDRESS, SYSTEM_CONTEXT_ADDRESS};
use rig::testing_signer;
use rig::utils::L1TxBuilder;
use rig::zksync_os_interface::types::{ExecutionOutput, ExecutionResult};
use rig::TestingFramework;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

sol! {
    function L1_CHAIN_ID() external view returns (uint256 chainId);
}

const L2_ASSET_TRACKER_INTEROP_INFO_SLOT: u64 = 156;
const INTEROP_INFO_TOTAL_SUCCESSFUL_DEPOSITS_OFFSET: u64 = 1;

fn b160_to_address(b: rig::ruint::aliases::B160) -> rig::alloy::primitives::Address {
    b.into_alloy()
}

fn asset_tracker_address() -> rig::alloy::primitives::Address {
    b160_to_address(L2_ASSET_TRACKER_ADDRESS)
}

fn interop_info_mapping_slot(asset_id: B256) -> U256 {
    let mut hasher = rig::crypto::sha3::Keccak256::new();
    hasher.update(asset_id.to_be_bytes::<32>());
    hasher.update(U256::from(L2_ASSET_TRACKER_INTEROP_INFO_SLOT).to_be_bytes::<32>());
    U256::from_be_bytes(hasher.finalize())
}

fn read_total_successful_deposits_from_l1(tester: &mut TestingFramework) -> U256 {
    let slot = interop_info_mapping_slot(DEFAULT_BASE_TOKEN_ASSET_ID)
        + U256::from(INTEROP_INFO_TOTAL_SUCCESSFUL_DEPOSITS_OFFSET);
    tester
        .get_storage_slot(&asset_tracker_address(), slot)
        .map(|value| value.into_u256_be())
        .unwrap_or(U256::ZERO)
}

fn read_l1_chain_id_tx(nonce: u64) -> ZKsyncTxEnvelope {
    let wallet = testing_signer(0);
    let tx = TxLegacy {
        chain_id: 37u64.into(),
        nonce,
        gas_price: 1000,
        gas_limit: 50_000,
        to: TxKind::Call(asset_tracker_address()),
        value: Default::default(),
        input: L1_CHAIN_IDCall {}.abi_encode().into(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
}

fn assert_reverted_deposit_asset_tracker(
    tester: &mut TestingFramework,
    output: &rig::zksync_os_interface::types::BlockOutput,
    expected_total_deposited: rig::alloy::primitives::U256,
) {
    assert_eq!(output.tx_results.len(), 1);
    let tx_result = output.tx_results[0].as_ref().expect("tx should not error");
    assert!(
        !tx_result.is_success(),
        "L1 tx should revert during execution, not be rejected"
    );

    let accumulated = read_total_successful_deposits_from_l1(tester);
    assert_eq!(
        accumulated,
        U256::from_be_slice(&expected_total_deposited.to_be_bytes::<32>()),
        "successful L1 deposits recorded by the real asset tracker should equal total_deposited"
    );
}

/// Verify that when an L1 tx has a deposit (total_deposited > 0), the bootloader
/// notifies the real `L2AssetTracker` and it records the deposited amount.
#[test]
fn test_asset_tracker_called_on_deposit() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = address!("abcd000000000000000000000000000000000000");
    let gas_price: u128 = 10_000;
    let gas_limit: u128 = 50_000;
    let value = rig::alloy::primitives::U256::from(500);
    let to_mint = rig::alloy::primitives::U256::from(gas_limit * gas_price)
        + rig::alloy::primitives::U256::from(1_000_000u64);

    let mut tester = TestingFramework::new().with_balance(from, U256::from(u64::MAX));

    let tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(gas_price)
        .gas_limit(gas_limit)
        .value(value)
        .to_mint(to_mint)
        .build();

    let output = tester.execute_block(vec![tx]);

    assert_eq!(output.tx_results.len(), 1);
    let tx_result = output.tx_results[0].as_ref().expect("tx should not error");
    assert!(tx_result.is_success(), "L1 tx should succeed");

    let accumulated = read_total_successful_deposits_from_l1(&mut tester);
    assert_eq!(
        accumulated,
        U256::from_be_slice(&to_mint.to_be_bytes::<32>()),
        "recorded deposits from L1 should equal to_mint"
    );

    // computational_native_used reflects the main tx body computation
    // plus intrinsic native. Post-execution operations (asset tracker
    // notifications, coinbase transfer, refund) run on FORMAL_INFINITE
    // and their cost is covered by L1_TX_INTRINSIC_NATIVE_COST, not
    // measured at runtime.
    assert!(
        tx_result.computational_native_used > 0,
        "computational_native_used should be nonzero"
    );
}

#[test]
fn test_asset_tracker_predeploy_is_usable_in_l1_flow() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = address!("abcd000000000000000000000000000000000000");
    let gas_price: u128 = 10_000;
    let gas_limit: u128 = 50_000;
    let value = rig::alloy::primitives::U256::from(500);
    let to_mint = rig::alloy::primitives::U256::from(gas_limit * gas_price)
        + rig::alloy::primitives::U256::from(1_000_000u64);
    let wallet = testing_signer(0);

    let mut tester = TestingFramework::new()
        .with_balance(from, U256::from(u64::MAX))
        .with_prefunded_account(wallet.address());

    let l1_tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(gas_price)
        .gas_limit(gas_limit)
        .value(value)
        .to_mint(to_mint)
        .build();

    let l1_output = tester.execute_block(vec![l1_tx]);
    assert_eq!(l1_output.tx_results.len(), 1);
    let l1_result = l1_output.tx_results[0]
        .as_ref()
        .expect("L1 tx should not error");
    assert!(l1_result.is_success(), "L1 tx should succeed");

    let getter_output = tester.execute_block(vec![read_l1_chain_id_tx(0)]);
    assert_eq!(getter_output.tx_results.len(), 1);
    let getter_result = getter_output.tx_results[0]
        .as_ref()
        .expect("getter tx should not error");
    assert!(getter_result.is_success(), "getter tx should succeed");

    match &getter_result.execution_result {
        ExecutionResult::Success(ExecutionOutput::Call(output)) => {
            let decoded =
                L1_CHAIN_IDCall::abi_decode_returns(output.as_slice()).expect("valid ABI");
            assert_eq!(
                decoded,
                U256::ONE,
                "L2AssetTracker L1_CHAIN_ID() should return 1"
            );
        }
        other => panic!("execution result must be a successful call, got {other:?}"),
    }
}

/// Verify that no deposit is recorded when total_deposited == 0.
#[test]
fn test_asset_tracker_not_called_without_deposit() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = address!("abcd000000000000000000000000000000000000");
    let gas_price: u128 = 0;
    let gas_limit: u128 = 50_000;
    let to_mint = rig::alloy::primitives::U256::ZERO;

    let mut tester = TestingFramework::new().with_balance(from, U256::from(u64::MAX));

    let tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(gas_price)
        .gas_limit(gas_limit)
        .value(rig::alloy::primitives::U256::ZERO)
        .to_mint(to_mint)
        .build();

    let output = tester.execute_block(vec![tx]);

    assert_eq!(output.tx_results.len(), 1);
    let tx_result = output.tx_results[0].as_ref().expect("tx should not error");
    assert!(tx_result.is_success(), "L1 tx should succeed");

    assert_eq!(
        read_total_successful_deposits_from_l1(&mut tester),
        U256::ZERO,
        "asset tracker should not record deposits when total_deposited == 0"
    );
}

/// Verify the call works with a different settlement layer chain ID
/// and that the real asset tracker records the deposit under that configuration.
#[test]
fn test_asset_tracker_uses_correct_chain_id() {
    let sl_chain_id: u64 = 270;
    let from = address!("1234000000000000000000000000000000000000");
    let to = address!("abcd000000000000000000000000000000000000");
    let gas_price: u128 = 10_000;
    let gas_limit: u128 = 50_000;
    let to_mint = rig::alloy::primitives::U256::from(gas_limit * gas_price)
        + rig::alloy::primitives::U256::from(2_000_000u64);

    let mut tester = TestingFramework::new()
        .with_storage_slot(
            asset_tracker_address(),
            U256::from(L2_ASSET_TRACKER_L1_CHAIN_ID_SLOT),
            B256::from(U256::from(sl_chain_id)),
        )
        .with_storage_slot(
            b160_to_address(SYSTEM_CONTEXT_ADDRESS),
            U256::from(SYSTEM_CONTEXT_SETTLEMENT_LAYER_CHAIN_ID_SLOT),
            B256::from(U256::from(sl_chain_id)),
        )
        .with_balance(from, U256::from(u64::MAX));

    let tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(gas_price)
        .gas_limit(gas_limit)
        .value(rig::alloy::primitives::U256::from(100))
        .to_mint(to_mint)
        .build();

    let output = tester.execute_block(vec![tx]);

    assert_eq!(output.tx_results.len(), 1);
    let tx_result = output.tx_results[0].as_ref().expect("tx should not error");
    assert!(tx_result.is_success(), "L1 tx should succeed");

    let accumulated = read_total_successful_deposits_from_l1(&mut tester);
    assert_eq!(
        accumulated,
        U256::from_be_slice(&to_mint.to_be_bytes::<32>()),
        "recorded deposit amount should match to_mint for the configured chain id"
    );
}

#[test]
fn test_asset_tracker_reverted_body_skips_value_mint_notification() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = address!("abcd000000000000000000000000000000000000");
    let gas_price: u128 = 1000;
    let gas_limit: u128 = 50_000;
    let to_mint = rig::alloy::primitives::U256::from(gas_limit * gas_price)
        + rig::alloy::primitives::U256::from(1_000_000u64);

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &rig::evm_bytecode::revert())
        .with_balance(from, U256::from(u64::MAX));

    let tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(gas_price)
        .gas_limit(gas_limit)
        .value(rig::alloy::primitives::U256::from(500))
        .to_mint(to_mint)
        .build();

    let output = tester.execute_block(vec![tx]);
    assert_reverted_deposit_asset_tracker(&mut tester, &output, to_mint);
}

#[test]
fn test_asset_tracker_oog_body_still_notifies_fee_and_refund() {
    let from = address!("1234000000000000000000000000000000000000");
    let to = address!("abcd000000000000000000000000000000000000");
    let gas_price: u128 = 1000;
    let gas_limit: u128 = 50_000;
    let to_mint = rig::alloy::primitives::U256::from(gas_limit * gas_price)
        + rig::alloy::primitives::U256::from(1_000_000u64);

    let expensive_bytecode = BytecodeBuilder::new()
        .push_u8(1)
        .push0()
        .sstore()
        .push_u8(2)
        .push_u8(1)
        .sstore()
        .push_u8(3)
        .push_u8(2)
        .sstore()
        .return_empty()
        .finish();

    let mut tester = TestingFramework::new()
        .with_evm_contract(to, &expensive_bytecode)
        .with_balance(from, U256::from(u64::MAX));

    let tx: ZKsyncTxEnvelope = L1TxBuilder::new()
        .from(from)
        .to(to)
        .gas_price(gas_price)
        .gas_limit(gas_limit)
        .value(rig::alloy::primitives::U256::from(500))
        .to_mint(to_mint)
        .build();

    let output = tester.execute_block(vec![tx]);
    assert_reverted_deposit_asset_tracker(&mut tester, &output, to_mint);
}
