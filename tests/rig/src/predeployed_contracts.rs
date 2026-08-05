use alloy::hex;
use crypto::MiniDigest;
use ruint::aliases::{B256, U256};
use system_hooks::addresses_constants::{L2_ASSET_TRACKER_ADDRESS, SYSTEM_CONTEXT_ADDRESS};

use crate::Chain;

pub const L2_ASSET_TRACKER_L1_CHAIN_ID_SLOT: u64 = 154;
pub const L2_ASSET_TRACKER_BASE_TOKEN_ASSET_ID_SLOT: u64 = 155;
pub const L2_ASSET_TRACKER_ASSET_MIGRATION_NUMBER_SLOT: u64 = 152;
pub const L2_ASSET_TRACKER_IS_ASSET_REGISTERED_SLOT: u64 = 153;
pub const DEFAULT_L1_CHAIN_ID: u64 = 1;
pub const DEFAULT_BASE_TOKEN_ASSET_ID: B256 = B256::from_limbs([1, 0, 0, 0]);
pub const SYSTEM_CONTEXT_SETTLEMENT_LAYER_CHAIN_ID_SLOT: u64 = 0;

// Runtime bytecode for the L2AssetTracker predeploy.
// Source contract:
// https://github.com/matter-labs/era-contracts/blob/2f024c5764e7a873ce1dda5fb990331559996441/l1-contracts/contracts/bridge/asset-tracker/L2AssetTracker.sol
pub const L2_ASSET_TRACKER_BYTECODE: &str = include_str!("bytecodes/l2_asset_tracker.hex");

pub const SYSTEM_CONTEXT_BYTECODE: &str = include_str!("bytecodes/system_context.hex");

fn mapping_slot_bytes32(key: B256, slot: u64) -> U256 {
    let mut hasher = crypto::sha3::Keccak256::new();
    hasher.update(key.to_be_bytes::<32>());
    hasher.update(U256::from(slot).to_be_bytes::<32>());
    U256::from_be_bytes(hasher.finalize())
}

fn nested_mapping_slot_u64_bytes32(key1: u64, key2: B256, slot: u64) -> U256 {
    let mut hasher = crypto::sha3::Keccak256::new();
    hasher.update(U256::from(key1).to_be_bytes::<32>());
    hasher.update(U256::from(slot).to_be_bytes::<32>());
    let outer_slot = hasher.finalize();

    let mut hasher = crypto::sha3::Keccak256::new();
    hasher.update(key2.to_be_bytes::<32>());
    hasher.update(outer_slot);
    U256::from_be_bytes(hasher.finalize())
}

/// Installs the default system-contract predeploys required by rig-based tests.
///
/// This deploys `L2AssetTracker` and `SystemContext` at their canonical addresses and
/// seeds the minimal storage they need for the L1 finalization and settlement-layer
/// chain-id flows used across the test suite. The initialized state is intentionally
/// deterministic so every fresh `TestingFramework` instance starts from the same
/// protocol-level assumptions.
pub fn install_default_predeployed_contracts<const RANDOMIZED_TREE: bool>(
    chain: &mut Chain<RANDOMIZED_TREE>,
) {
    let l2_asset_tracker_bytecode =
        hex::decode(L2_ASSET_TRACKER_BYTECODE.trim()).expect("valid L2AssetTracker bytecode");
    chain.set_evm_bytecode(L2_ASSET_TRACKER_ADDRESS, &l2_asset_tracker_bytecode);
    chain.set_storage_slot(
        L2_ASSET_TRACKER_ADDRESS,
        U256::from(L2_ASSET_TRACKER_L1_CHAIN_ID_SLOT),
        B256::from(U256::from(DEFAULT_L1_CHAIN_ID)),
    );
    chain.set_storage_slot(
        L2_ASSET_TRACKER_ADDRESS,
        U256::from(L2_ASSET_TRACKER_BASE_TOKEN_ASSET_ID_SLOT),
        DEFAULT_BASE_TOKEN_ASSET_ID,
    );
    chain.set_storage_slot(
        L2_ASSET_TRACKER_ADDRESS,
        mapping_slot_bytes32(
            DEFAULT_BASE_TOKEN_ASSET_ID,
            L2_ASSET_TRACKER_IS_ASSET_REGISTERED_SLOT,
        ),
        B256::from(U256::ONE),
    );
    chain.set_storage_slot(
        L2_ASSET_TRACKER_ADDRESS,
        nested_mapping_slot_u64_bytes32(
            chain.chain_id(),
            DEFAULT_BASE_TOKEN_ASSET_ID,
            L2_ASSET_TRACKER_ASSET_MIGRATION_NUMBER_SLOT,
        ),
        B256::from(U256::ONE),
    );

    let system_context_bytecode =
        hex::decode(SYSTEM_CONTEXT_BYTECODE.trim()).expect("valid system context bytecode");
    chain.set_evm_bytecode(SYSTEM_CONTEXT_ADDRESS, &system_context_bytecode);
    chain.set_storage_slot(
        SYSTEM_CONTEXT_ADDRESS,
        U256::from(SYSTEM_CONTEXT_SETTLEMENT_LAYER_CHAIN_ID_SLOT),
        B256::from(U256::from(DEFAULT_L1_CHAIN_ID)),
    );
}
