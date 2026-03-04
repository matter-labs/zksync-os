# ZKsync OS Integration Testing Guide

**Primary audience:** humans and AI agents writing new tests.

After reading this document you should be able to write a correct integration test from scratch without looking at any other file.

---

## 1. Overview

The test rig (`tests/rig/`) contains an **in-memory chain** (`Chain`) that:

1. Accepts a list of encoded transactions.
2. Runs a **forward pass** through the bootloader (fast, no proving).
3. Optionally runs a **RISC-V proof simulation** using the `for_tests` binary.
4. Returns a `BlockOutput` containing per-transaction results, storage writes, emitted events, and published preimages.

Between blocks the rig persists:
- Storage tree updates from every `StorageWrite`.
- Preimage additions from `published_preimages`.
- The latest block hash (for `BLOCKHASH` opcode tests).

There is **no networking, no JSON-RPC**, and no external state. Everything runs in-process.

---

## 2. Quick Start

Minimal test — ETH transfer from a funded account to an EOA:

```rust
use rig::{Chain, builder::{ChainBuilder, TxBuilder}, constants::*, run_config};
use alloy::primitives::{address, Address};
use alloy::signers::local::PrivateKeySigner;
use ruint::aliases::{B160, U256};

#[test]
fn eth_transfer_succeeds() {
    // 1. Create a signer and derive its address as B160
    let signer = PrivateKeySigner::random();
    let sender = B160::from_be_bytes(signer.address().into_array());
    let recipient = address!("deadbeef00000000000000000000000000000001");

    // 2. Build initial chain state
    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    // 3. Build and sign a transaction
    let tx = TxBuilder::new()
        .from(signer)
        .to(recipient)
        .value(alloy::primitives::U256::from(1_000u64))
        .gas_limit(TRANSFER_GAS_LIMIT)
        .build();

    // 4. Run the block
    let output = chain.run_block(vec![tx], None, None, Some(run_config::full_proof()));

    // 5. Assert results
    assert_tx_success!(output, 0);
}
```

**That's it.** No wallet boilerplate, no raw `RunConfig { ... }`, no `.unwrap()` on results.

---

## 3. Setting Up Chain State — `ChainBuilder`

```rust
use rig::builder::ChainBuilder;
use rig::constants::*;
use ruint::aliases::{B160, U256, B256};

let mut chain = ChainBuilder::new()
    // Optional: override chain ID (default = TEST_CHAIN_ID = 37)
    .chain_id(TEST_CHAIN_ID)
    // Fund an address
    .with_balance(my_address, U256::from(DEFAULT_BALANCE))
    // Also accepts alloy Address:
    .with_balance_addr(alloy_address, U256::from(DEFAULT_BALANCE))
    // Deploy EVM bytecode
    .with_evm_bytecode(contract_address, bytecode_vec)
    .with_evm_bytecode_addr(alloy_address, bytecode_vec)
    // Pre-set a storage slot
    .with_storage_slot(address, slot_key_u256, value_b256)
    // Register a preimage (used for forced deployments)
    .with_preimage(hash_bytes32, preimage_vec)
    .build();  // returns Chain<false>
```

All builder methods consume and return `Self` (method chaining). `build()` returns a `Chain<false>`.

### Mutating state after build

The `Chain` itself exposes all the same setters if you need to add state between blocks:

```rust
chain.set_balance(address, amount);
chain.set_evm_bytecode(address, &bytecode);
chain.set_storage_slot(address, key, value);
chain.set_preimage(hash, &data);
```

### Loading contract bytecode

```rust
// Load a pre-compiled Solidity deployed bytecode
let bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");

// Load a WASM contract
let bytecode = rig::utils::load_wasm_bytecode("my_contract");
```

Paths are relative to workspace root: `tests/contracts_sol/{project}/out/{name}.dep.txt`.

### Generating a signer with the correct chain ID

`PrivateKeySigner::random()` creates a signer with no chain ID, which may cause wrong-chain-ID rejections if you forget to set it.
`chain.random_signer()` is a convenience wrapper that automatically sets the chain ID to match the chain:

```rust
// Preferred — chain ID is set automatically
let signer = chain.random_signer();
let sender = B160::from_be_bytes(signer.address().into_array());

// Also works, but requires manually setting chain ID for non-default chains
let signer = PrivateKeySigner::random();
```

---

## 4. Building Transactions — `TxBuilder`

### EIP-1559 (most common)

```rust
use rig::builder::TxBuilder;
use rig::constants::*;

let tx = TxBuilder::new()           // defaults to EIP-1559
    .from(signer)                   // PrivateKeySigner
    .to(recipient_address)          // alloy Address
    .calldata(my_bytes)             // Vec<u8>
    .value(alloy::primitives::U256::from(0u64))
    .gas_limit(CALL_GAS_LIMIT)      // optional, defaults to CALL_GAS_LIMIT
    .nonce(0)                       // optional, defaults to 0
    .max_fee(DEFAULT_MAX_FEE)       // optional
    .priority_fee(DEFAULT_PRIORITY_FEE) // optional
    .build();                       // returns EncodedTx
```

### Legacy (type 0)

```rust
let tx = TxBuilder::new()
    .legacy()
    .from(signer)
    .to(address)
    .build();
```

### EIP-2930 (type 1, with access list)

```rust
use alloy::eips::eip2930::{AccessList, AccessListItem};
use alloy::primitives::Address;

let al = AccessList(vec![AccessListItem {
    address: my_contract_address,
    storage_keys: vec![B256::ZERO],
}]);

let tx = TxBuilder::new()
    .eip2930()
    .from(signer)
    .to(address)
    .access_list(al)      // pre-warm the address/slot (EIP-2929)
    .build();
```

EIP-1559 transactions also accept `.access_list(al)`.

### Contract creation

```rust
let tx = TxBuilder::new()
    .eip1559()
    .from(signer)
    .create()                        // sets to = null
    .calldata(deployment_bytecode)
    .gas_limit(DEPLOY_GAS_LIMIT)
    .build();
```

### L1 → L2 transaction

```rust
let tx = TxBuilder::new()
    .l1()
    .from(signer)
    .to(address)
    .calldata(data)
    .build();
```

### Upgrade transaction

```rust
let tx = TxBuilder::new()
    .upgrade()
    .from(signer)
    .to(address)
    .build();
```

### Using low-level helpers directly

All low-level helpers are still available for cases not covered by `TxBuilder`:

```rust
use rig::utils::{sign_and_encode_alloy_tx, encode_l1_tx, encode_upgrade_tx,
                  sign_and_encode_ethers_legacy_tx};
```

---

## 5. Running Blocks

### Standard run

```rust
let output: BlockOutput = chain.run_block(
    transactions,           // Vec<EncodedTx>
    block_context,          // Option<BlockContext> — None = defaults
    da_commitment_scheme,   // Option<DACommitmentScheme> — None = BlobsAndPubdataKeccak256
    run_config,             // Option<RunConfig>
);
```

### `RunConfig` presets

| Preset | When to use |
|--------|-------------|
| `run_config::forward_only()` | Fast iteration — no RISC-V simulation. Can miss bugs in proving code. |
| `run_config::full_proof()` | Correctness tests — runs RISC-V sim, checks storage-diff hashes. Use in CI. |
| `run_config::with_profiler(path)` | Profiling — generates flamegraph at `path`. |
| `run_config::with_witness_dump(path)` | Save witness for replay / debugging. |
| `None` | Same as `forward_only()` — skips the RISC-V simulator entirely. |

Import with `use rig::run_config;`.

### Custom `BlockContext`

```rust
use rig::chain::BlockContext;

let ctx = BlockContext {
    timestamp: 1_000_000,
    eip1559_basefee: ruint::aliases::U256::from(500u64),
    ..BlockContext::default()
};
let output = chain.run_block(txs, Some(ctx), None, Some(run_config::full_proof()));
```

### Simulation (no validation)

For gas estimation or exploratory calls:

```rust
let output = chain.simulate_block(transactions, None);
```

Simulation skips signature validation and does not update chain state.

### Multi-block sequences

```rust
let out1 = chain.run_block(block1_txs, None, None, Some(run_config::full_proof()));
assert_all_success!(out1);

// State is automatically updated between blocks
let out2 = chain.run_block(block2_txs, None, None, Some(run_config::full_proof()));
assert_tx_success!(out2, 0);
```

### When the whole block panics — `run_block_no_panic`

`run_block()` panics if the bootloader itself encounters an internal error (distinct from a per-transaction failure). In rare cases (e.g., malformed block-level inputs) you may want to test that the bootloader returns an error without crashing the test process. Use:

```rust
let result = chain.run_block_no_panic(transactions, None, None, None);
match result {
    Ok(output) => { /* block executed */ }
    Err(e) => { /* bootloader-level panic captured */ }
}
```

Per-transaction failures (bad nonce, OOG, EVM revert) are **not** bootloader panics — they appear as `Err(InvalidTransaction)` inside `output.tx_results`. Use `assert_tx_failed!` / `assert_tx_reverted!` for those.

### Simulation (no state mutation, no signature verification)

`simulate_block` runs the block in a read-only mode: signature validation is skipped and chain state is **not** updated. Useful for gas estimation or exploratory calls where you do not have a valid signer:

```rust
let output = chain.simulate_block(transactions, None);
// output.tx_results contains results but chain state is unchanged
// chain.run_block(next_txs, ...) sees the same state as before simulate_block
```

---

## 6. Asserting Results

All assertion macros are defined with `#[macro_export]` in the `rig` crate. In Rust 2021 edition, you must import them explicitly:

```rust
use rig::{assert_tx_success, assert_tx_reverted, assert_tx_failed,
          assert_all_success, assert_gas_used_lt, assert_gas_used_gt,
          assert_gas_used_between, assert_storage_written,
          assert_event_emitted, assert_event_not_emitted,
          assert_block_events_count, assert_account_balance, assert_nonce};
```

Import only what you need — unused imports are compile warnings.

### Per-transaction assertions

```rust
assert_tx_success!(output, 0);    // tx[0] accepted by bootloader AND EVM succeeded
assert_tx_reverted!(output, 0);   // tx[0] accepted by bootloader BUT EVM reverted
assert_tx_failed!(output, 0);     // tx[0] rejected at bootloader level (bad nonce, sig, etc.)
```

**Choosing the right macro:**

| Scenario | Expected macro |
|---------|----------------|
| Nonce too low | `assert_tx_failed!` |
| Insufficient balance for gas | `assert_tx_failed!` |
| Wrong chain ID | `assert_tx_failed!` |
| `max_fee_per_gas` below basefee | `assert_tx_failed!` |
| EVM `REVERT` opcode | `assert_tx_reverted!` |
| Out-of-gas mid-execution | `assert_tx_reverted!` |
| Stack underflow / `INVALID` opcode | `assert_tx_reverted!` |
| Successful transfer / call / deploy | `assert_tx_success!` |

### Block-level assertions

```rust
assert_all_success!(output);      // every tx in the block succeeded
```

### Gas assertions

```rust
assert_gas_used_lt!(output, 0, 50_000);          // tx[0] used < 50 000 computational_native
assert_gas_used_gt!(output, 0, 1_000);            // tx[0] used > 1 000 (not optimized away)
assert_gas_used_between!(output, 0, 1_000, 50_000); // tx[0] is in [1000, 50000)
```

### Storage assertions

```rust
// Check that a specific slot was written
assert_storage_written!(
    output,
    addr.to_be_bytes::<32>(),        // [u8; 32] — left-pad a B160 address
    key.to_be_bytes::<32>(),         // [u8; 32] — big-endian U256 key
    value.to_be_bytes::<32>(),       // [u8; 32] — big-endian B256 value
);
```

### Event / log assertions

```rust
use alloy::primitives::{Address, B256};

// Check that a log was emitted from a given address with a given topic0
assert_event_emitted!(output, address, topic0);     // Address, B256
assert_event_not_emitted!(output, address, topic0); // ensures no such event

// Check total event count across all transactions in the block
assert_block_events_count!(output, 3);
```

Events are stored per-transaction at `output.tx_results[n].as_ref().unwrap().logs`. You can inspect them directly:

```rust
let tx_out = output.tx_results[0].as_ref().unwrap();
for log in &tx_out.logs {
    // log.address — emitting contract
    // log.topics() — slice of B256 topic values
    // log.data.data — raw ABI-encoded data bytes
}
```

### Account state assertions

```rust
use ruint::aliases::{B160, U256};

// After a block run, check the on-chain balance of an address
assert_account_balance!(chain, sender_b160, U256::from(expected_wei));

// Check the on-chain nonce of an address
assert_nonce!(chain, sender_b160, 1u64);
```

These call `chain.get_account_properties(&addr)` and compare the `.balance` / `.nonce` fields.

### Inspecting results manually

```rust
// Check a specific tx result
let tx_out = output.tx_results[0].as_ref().unwrap();
assert!(tx_out.is_success());
let gas_used = tx_out.computational_native_used;
let gas_refunded = tx_out.gas_refunded;

// Full BlockOutput fields
// output.header          — sealed block header (block hash, timestamp, …)
// output.tx_results      — Vec<Result<TxOutput, InvalidTransaction>>
// output.storage_writes  — all storage writes in the block (across all txs)
// output.account_diffs   — per-account nonce/balance/bytecode diffs
// output.published_preimages — bytecodes published to the chain
// output.pubdata         — raw pubdata bytes
// output.computaional_native_used — total computational gas used (note: typo in field name)

// Iterate storage writes
for write in &output.storage_writes {
    // write.account — Address (20-byte alloy type)
    // write.account_key — B256 slot key
    // write.value — B256 value
    // write.key — B256 flat storage key (hash of account+slot)
}

// Inspect account diffs
for diff in &output.account_diffs {
    // diff.address — Address
    // diff.nonce   — u64
    // diff.balance — U256
    // diff.bytecode_hash — B256
}
```

---

## 7. System Contract Addresses

All addresses are available in `rig::constants`:

```rust
use rig::constants::*;

// CONTRACT_DEPLOYER  = 0x0000...8006
// L2_BASE_TOKEN      = 0x0000...800a
// L1_MESSENGER       = 0x0000...8008
// MSG_VALUE_SIMULATOR = 0x0000...8009
// NONCE_HOLDER       = 0x0000...8003
// ACCOUNT_CODE_STORAGE = 0x0000...8002
```

---

## 8. Common Patterns

### ERC-20 deploy and call

```rust
let bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");
let contract = address!("0000000000000000000000000000000000010002");

let mut chain = ChainBuilder::new()
    .with_evm_bytecode_addr(contract, bytecode)
    .with_balance(sender, U256::from(DEFAULT_BALANCE))
    .build();

let mint_calldata = hex::decode(rig::utils::ERC_20_MINT_CALLDATA).unwrap();
let mint_tx = TxBuilder::new()
    .from(signer.clone())
    .to(contract)
    .calldata(mint_calldata)
    .gas_limit(80_000)
    .build();

let output = chain.run_block(vec![mint_tx], None, None, Some(run_config::full_proof()));
assert_tx_success!(output, 0);
```

### L1→L2 message with value

```rust
let l1_tx = TxBuilder::new()
    .l1()
    .from(system_signer)
    .to(target_address)
    .calldata(calldata)
    .value(alloy::primitives::U256::from(1_000_000u64))
    .gas_limit(DEFAULT_GAS_LIMIT)
    .build();

let output = chain.run_block(vec![l1_tx], None, None, Some(run_config::full_proof()));
assert_tx_success!(output, 0);
```

### Multi-tx block with state dependencies

```rust
// tx0 writes slot, tx1 reads it
let output = chain.run_block(
    vec![tx_write, tx_read],
    None, None,
    Some(run_config::full_proof()),
);
assert_all_success!(output);
```

### Gas measurement

```rust
let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));
assert_tx_success!(output, 0);
let gas = output.tx_results[0].as_ref().unwrap().computational_native_used;
println!("gas used: {gas}");
assert_gas_used_lt!(output, 0, 100_000);
```

### Testing revert paths

```rust
// Transaction with insufficient gas
let tx = TxBuilder::new()
    .from(signer)
    .to(contract)
    .calldata(expensive_calldata)
    .gas_limit(100)   // not enough
    .build();

let output = chain.run_block(vec![tx], None, None, Some(run_config::forward_only()));
// Either bootloader-rejected or EVM-reverted depending on where OOG fires
// assert_tx_failed! or assert_tx_reverted! as appropriate
```

---

## 9. Coverage Map

| Subsystem | Test crate | Status |
|-----------|-----------|--------|
| EIP-1559 / Legacy / EIP-2930 tx types | `transactions` | ✅ |
| Contract deployment | `transactions` | ✅ |
| ERC-20 Solidity contract | `erc20` | ✅ |
| Precompiles | `precompiles` | ✅ |
| System hooks | `system_hooks` | ✅ |
| Multi-block batches | `multiblock_batch` | ✅ |
| Block header | `header` | ✅ |
| EVM conformance | `evm` | ✅ |
| Forge-compatible tests | `forge_tests` | ✅ |
| Out-of-gas / invalid txs | `errors` | ✅ |
| Edge cases (zero-value, self-transfer, …) | `edge_cases` | ✅ |

---

## 10. How to Add a New Test

### Step 1 — Choose a crate

Add to an existing instance crate if the test fits an existing category. Create a new crate under `tests/instances/` otherwise.

### Step 2 — New crate template

`tests/instances/my_feature/Cargo.toml`:

```toml
[package]
name = "my_feature"
version.workspace = true
edition.workspace = true
authors.workspace = true
homepage.workspace = true
repository.workspace = true
license.workspace = true
keywords.workspace = true
categories.workspace = true

[dependencies]
rig = { path = "../../rig", features = ["for_tests"] }
hex = "*"

[features]
e2e_proving = ["rig/e2e_proving"]
```

Add `"tests/instances/my_feature"` to the `members` list in the root `Cargo.toml`.

### Step 3 — Write the test

`tests/instances/my_feature/src/lib.rs`:

```rust
#![cfg(test)]
use rig::{
    builder::{ChainBuilder, TxBuilder},
    constants::*,
    run_config,
    Chain,
};
use alloy::signers::local::PrivateKeySigner;
use ruint::aliases::{B160, U256};

#[test]
fn my_new_test() {
    let signer = PrivateKeySigner::random();
    let sender = B160::from_be_bytes(signer.address().into_array());

    let mut chain = ChainBuilder::new()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .build();

    let tx = TxBuilder::new()
        .from(signer)
        .to(/* target address */)
        .gas_limit(CALL_GAS_LIMIT)
        .build();

    let output = chain.run_block(vec![tx], None, None, Some(run_config::full_proof()));

    assert_tx_success!(output, 0);
}
```

### Step 4 — Run

```bash
cargo test -p my_feature --release
```

For all tests:

```bash
cargo test --workspace --exclude zksync_os --release -j 4
```
