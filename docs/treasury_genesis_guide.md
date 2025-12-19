# Treasury Genesis Initialization Guide

## Overview

The treasury-based token system requires the L2 base token contract (0x800a) to be pre-funded with U256::MAX tokens at genesis. This guide explains how to initialize the treasury for different scenarios.

## For Testing

### Quick Start

Use the built-in helper method:

```rust
use rig::Chain;

let mut chain = Chain::empty(None);
chain.initialize_treasury();
```

This automatically sets the balance of address 0x800a to U256::MAX.

### What It Does

The `initialize_treasury()` method internally calls:

```rust
chain.set_account_properties(
    L2_BASE_TOKEN_ADDRESS,  // 0x800a
    Some(U256::MAX),        // balance
    None,                   // nonce stays 0
    None,                   // no bytecode needed
);
```

## For Production Deployment

### Method 1: Direct State Tree Initialization

Create a genesis state initialization script:

```rust
use forward_system::run::test_impl::{InMemoryTree, InMemoryPreimageSource};
use basic_system::system_implementation::flat_storage_model::{
    AccountProperties, derive_flat_storage_key, address_into_special_storage_key,
    ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use ruint::aliases::U256;
use system_hooks::addresses_constants::L2_BASE_TOKEN_ADDRESS;
use zksync_os_api::helpers::set_properties_balance;

pub fn create_genesis_with_treasury() -> (InMemoryTree, InMemoryPreimageSource) {
    let mut tree = InMemoryTree::empty();
    let mut preimage_source = InMemoryPreimageSource::new();

    // Create treasury account
    let mut account = AccountProperties::default();
    set_properties_balance(&mut account, U256::MAX);

    // Compute storage location
    let key = address_into_special_storage_key(&L2_BASE_TOKEN_ADDRESS);
    let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

    // Store account properties
    let encoding = account.encoding();
    let hash = account.compute_hash();
    preimage_source.inner.insert(hash, encoding.to_vec());
    tree.cold_storage.insert(flat_key, hash);
    tree.storage_tree.insert(&flat_key, &hash);

    (tree, preimage_source)
}
```

### Method 2: Modify Existing Genesis State

If you have existing genesis tooling, add treasury initialization:

```rust
// After creating your genesis state tree
pub fn add_treasury_to_genesis(
    tree: &mut InMemoryTree,
    preimage_source: &mut InMemoryPreimageSource,
) {
    use zksync_os_api::helpers::set_properties_balance;

    let mut account = AccountProperties::default();
    set_properties_balance(&mut account, U256::MAX);

    // Follow the pattern from Method 1 to insert into tree
    // ... (same insertion code as above)
}
```

### Method 3: JSON Genesis Configuration

If your system uses JSON configuration for genesis:

```json
{
  "genesis_accounts": {
    "0x000000000000000000000000000000000000800a": {
      "balance": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
      "nonce": "0",
      "code": null
    }
  }
}
```

Note: The balance value is U256::MAX (2^256 - 1).

## Verification

### Check Treasury Balance After Genesis

```rust
// In tests
let treasury_balance = chain.get_balance(L2_BASE_TOKEN_ADDRESS);
assert_eq!(treasury_balance, U256::MAX);
```

### Monitor Treasury During Operation

Add logging to track treasury usage:

```rust
use zk_ee::execution_environment_type::ExecutionEnvironmentType;

// After each block
let treasury_balance = system.io.get_nominal_token_balance(
    ExecutionEnvironmentType::NoEE,
    &mut infinite_resources,
    &L2_BASE_TOKEN_ADDRESS,
)?;

println!("Treasury balance: {}", treasury_balance);

// Alert if below threshold
if treasury_balance < U256::from(10u128.pow(30)) {
    eprintln!("⚠️  WARNING: Treasury balance low: {}", treasury_balance);
}
```

## Regenesis / Chain Reset

To reset the chain with a fresh treasury:

### For Tests

```rust
// Simply create a new chain
let mut chain = Chain::empty(None);
chain.initialize_treasury();
```

### For Production

1. **Export current state** (if needed):
```rust
let state_snapshot = (tree.clone(), preimage_source.clone());
```

2. **Create new genesis**:
```rust
let (new_tree, new_preimage) = create_genesis_with_treasury();
```

3. **Optionally migrate accounts** from old state to new:
```rust
// Copy important accounts (not the treasury)
for (address, account) in old_accounts {
    if address != L2_BASE_TOKEN_ADDRESS {
        // Copy to new_tree
    }
}
```

4. **Start chain from block 0** with new genesis state

## State Structure

The treasury is stored in the state tree as a regular account at address 0x800a:

```
State Tree:
├─ Account Properties Storage (0x8002)
│  └─ 0x800a properties:
│     ├─ balance: U256::MAX
│     ├─ nonce: 0
│     ├─ bytecode_hash: 0x0 (no code)
│     └─ versioning: default
```

## Integration with External Systems

### L1 Smart Contract Coordination

When deploying a new chain or doing regenesis:

1. **L1 State Root**: The genesis state root must be submitted to your L1 contract
2. **Commitment**: Ensure L1 contract expects treasury to be pre-funded
3. **Bridge Validation**: L1 bridge should not allow deposits exceeding U256::MAX

### Sequencer Configuration

Update your sequencer configuration to:

```yaml
genesis:
  treasury_address: "0x000000000000000000000000000000000000800a"
  treasury_initial_balance: "115792089237316195423570985008687907853269984665640564039457584007913129639935"
  validate_treasury_on_startup: true
```

## Common Issues

### Treasury Not Initialized

**Symptom**: Tests fail with `TreasuryTransferFailed` error

**Solution**: Ensure `chain.initialize_treasury()` is called after creating the chain

### Wrong Balance

**Symptom**: Treasury runs out unexpectedly

**Solution**: Verify you're using `U256::MAX`, not `U256::MAX - 1` or other values

### Genesis State Root Mismatch

**Symptom**: L1 contract rejects state root

**Solution**: Ensure your genesis state computation includes the treasury account

## Additional Resources

- Treasury implementation: `basic_bootloader/src/bootloader/run_single_interaction.rs:28`
- Account properties: `basic_system/src/system_implementation/flat_storage_model/account_cache_entry.rs`
- Test helper: `tests/rig/src/chain.rs:715`
- System addresses: `system_hooks/src/addresses_constants.rs:32`
