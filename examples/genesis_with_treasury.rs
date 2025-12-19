/// Example: How to create genesis state with treasury pre-funded
///
/// This demonstrates the production approach for initializing a chain
/// with the treasury account (0x800a) funded with U256::MAX tokens.

use basic_system::system_implementation::flat_storage_model::{
    address_into_special_storage_key, derive_flat_storage_key, AccountProperties,
    ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use ruint::aliases::{B160, U256};
use system_hooks::addresses_constants::L2_BASE_TOKEN_ADDRESS;
use zksync_os_api::helpers::{set_properties_balance, set_properties_code, set_properties_nonce};

/// Creates a genesis state with the treasury pre-funded
pub fn create_genesis_state() -> (InMemoryTree, InMemoryPreimageSource) {
    let mut tree = InMemoryTree::empty();
    let mut preimage_source = InMemoryPreimageSource::new();

    // 1. Initialize the treasury (REQUIRED)
    add_treasury(&mut tree, &mut preimage_source);

    // 2. Optionally add other genesis accounts
    // add_initial_validators(&mut tree, &mut preimage_source);
    // add_system_contracts(&mut tree, &mut preimage_source);

    (tree, preimage_source)
}

/// Add the treasury account with U256::MAX balance
fn add_treasury(tree: &mut InMemoryTree, preimage_source: &mut InMemoryPreimageSource) {
    let address = L2_BASE_TOKEN_ADDRESS; // 0x800a
    let balance = U256::MAX; // 2^256 - 1 (effectively unlimited)
    let nonce = 0;
    let bytecode = None; // Treasury doesn't need bytecode

    add_account(tree, preimage_source, address, balance, nonce, bytecode);

    println!("✓ Treasury initialized at 0x{:x}", address);
    println!("  Balance: {}", balance);
}

/// Generic function to add any account to genesis state
fn add_account(
    tree: &mut InMemoryTree,
    preimage_source: &mut InMemoryPreimageSource,
    address: B160,
    balance: U256,
    nonce: u64,
    bytecode: Option<Vec<u8>>,
) {
    // Create account properties
    let mut account = AccountProperties::default();
    set_properties_balance(&mut account, balance);
    set_properties_nonce(&mut account, nonce);

    // If bytecode is provided, set it
    if let Some(code) = bytecode {
        let full_bytecode = set_properties_code(&mut account, &code);
        preimage_source
            .inner
            .insert(account.bytecode_hash, full_bytecode);
    }

    // Compute storage location for this account
    let key = address_into_special_storage_key(&address);
    let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

    // Encode and hash account properties
    let encoding = account.encoding();
    let properties_hash = account.compute_hash();

    // Store in preimage source (for oracle queries)
    preimage_source.inner.insert(properties_hash, encoding.to_vec());

    // Store in state tree (both cold storage and merkle tree)
    tree.cold_storage.insert(flat_key, properties_hash);
    tree.storage_tree.insert(&flat_key, &properties_hash);
}

/// Example: Add an initial validator/operator with some balance
#[allow(dead_code)]
fn add_initial_validators(tree: &mut InMemoryTree, preimage_source: &mut InMemoryPreimageSource) {
    // Example validator address
    let validator_address = B160::from([0x42; 20]);
    let validator_balance = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
    let nonce = 0;

    add_account(
        tree,
        preimage_source,
        validator_address,
        validator_balance,
        nonce,
        None,
    );

    println!("✓ Validator added at 0x{:x}", validator_address);
}

/// Example: Deploy a contract at genesis
#[allow(dead_code)]
fn deploy_contract_at_genesis(
    tree: &mut InMemoryTree,
    preimage_source: &mut InMemoryPreimageSource,
    contract_address: B160,
    bytecode: Vec<u8>,
) {
    add_account(
        tree,
        preimage_source,
        contract_address,
        U256::ZERO, // Contracts start with 0 balance
        0,          // nonce 0
        Some(bytecode),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use forward_system::run::{run_block, BlockContext};
    use zksync_os_interface::traits::TxListSource;

    #[test]
    fn test_genesis_creation() {
        // Create genesis state
        let (tree, preimage_source) = create_genesis_state();

        // Verify treasury is in the state
        let key = address_into_special_storage_key(&L2_BASE_TOKEN_ADDRESS);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

        let treasury_hash = tree.cold_storage.get(&flat_key);
        assert!(
            treasury_hash.is_some(),
            "Treasury should be in genesis state"
        );

        println!("Genesis state created successfully");
        println!("Treasury hash: {:?}", treasury_hash);
    }

    #[test]
    #[ignore] // Remove this to run actual block execution
    fn test_run_block_with_genesis() {
        // Create genesis state with treasury
        let (tree, preimage_source) = create_genesis_state();

        // Create block 0 context
        let block_context = BlockContext {
            number: 0,
            timestamp: 1704067200, // 2024-01-01 00:00:00 UTC
            coinbase: B160::from([0x01; 20]),
            ..Default::default()
        };

        // Empty transaction list for genesis block
        let tx_source = TxListSource::default();

        // Run block 0
        let output = run_block(
            block_context,
            tree,
            preimage_source,
            tx_source,
            None, // no tracer
            None, // no tx result callback
        );

        println!("Genesis block executed successfully");
        println!("State root: {:?}", output.state_root);
    }
}

/// Main function showing how to use this in production
fn main() {
    println!("=== ZKsync OS Genesis State Creation ===\n");

    // Create genesis state
    let (tree, preimage_source) = create_genesis_state();

    println!("\n✓ Genesis state created");
    println!("  Total accounts: {}", tree.cold_storage.len());
    println!(
        "  Total preimages: {}",
        preimage_source.inner.len()
    );

    // In production, you would:
    // 1. Serialize this state
    // 2. Compute the state root
    // 3. Submit state root to L1 contract
    // 4. Start the sequencer with this genesis state

    // Example: Compute state root (simplified)
    let state_root = tree.storage_tree.root();
    println!("\n✓ Genesis state root: {:?}", state_root);
}
