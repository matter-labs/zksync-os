# Storage Model: Traits and Generics

This document explains the storage model trait hierarchy, generic type parameters,
and supporting abstractions in ZKsync OS. It is intended to help developers and
auditors navigate the heavily-parametrised codebase.

## Motivation

The IO subsystem is designed to work with multiple storage backends (different
Merkle tree structures, different account layouts, etc.) and multiple execution
environments (forward/sequencer mode, proving mode). Generics are used throughout
to achieve this flexibility with zero runtime cost. Understanding the role of each
type parameter is key to reading the code.

## Trait Hierarchy

```
SnapshottableIo
    └─ StorageModel          (persistent key-value storage + account model)
         └─ (implemented by) FlatTreeWithAccountsUnderHashesStorageModel
                              EthereumStorageModel

IOSubsystem                  (user-facing read/write interface)
IOSubsystemExt               (frame management, nonce/balance updates, deployment)
IOTeardown                   (block finalization, diff iteration, commitment update)
    └─ (implemented by) FullIO<A, R, P, SF, N, O, M, PROOF_ENV>
```

### `SnapshottableIo` ([source](../../../storage_models/src/common_structs/traits/snapshottable_io.rs))

Every storage layer must support transactional rollback via this trait:

```rust
pub trait SnapshottableIo {
    type StateSnapshot;

    fn begin_new_tx(&mut self);
    fn finish_tx(&mut self) -> Result<(), InternalError>;

    fn start_frame(&mut self) -> Self::StateSnapshot;
    fn finish_frame(&mut self, rollback_handle: Option<&Self::StateSnapshot>)
        -> Result<(), InternalError>;
}
```

- `start_frame` captures the current state as a snapshot.
- `finish_frame` either commits (when `rollback_handle` is `None`) or reverts all
  changes made since the snapshot was taken (when the handle is `Some`). This
  maps directly onto `CALL`/`CREATE` reversion semantics.
- `begin_new_tx` / `finish_tx` handle the transaction-level lifecycle, including
  clearing transient storage between transactions.

### `StorageModel` ([source](../../../storage_models/src/common_structs/traits/storage_model.rs))

The central abstraction for persistent state. It subsumes `SnapshottableIo` and
adds the full account+storage API used by the bootloader and EEs:

**Associated types:**

| Associated type        | Meaning |
|------------------------|---------|
| `IOTypes`              | Concrete type-level configuration (`SystemIOTypesConfig`). Fixes `Address`, `StorageKey`, `StorageValue`, etc. |
| `Resources`            | Resource tracking type (see [Resources](#resources-and-resource-charging)). |
| `StorageCommitment`    | The state root type (e.g. a Merkle root). Must be serialisable for oracle communication. |
| `Allocator`            | The allocator used for internal heap allocations. In proving mode this must be a custom bump allocator. |
| `InitData`             | Initialisation parameters (typically the `StorageAccessPolicy`). Passed once at construction. |
| `StorageKey<'a>`       | Opaque key type used in diff iteration. |
| `StorageDiff<'a>`      | Opaque diff entry type used in diff iteration. |

**Core methods:**
- `storage_read` / `storage_write` / `storage_touch` — slot access; all charge
  resources via the `EE`-specific policy.
- `read_account_properties` — reads a subset of account fields; see
  [AccountDataRequest](#accountdatarequest-and-the-maybe-type).
- `increment_nonce`, `update_nominal_token_value`, `transfer_nominal_token_value`
  — account mutation helpers.
- `deploy_code`, `set_bytecode_details`, `set_delegation` — bytecode lifecycle.
- `mark_for_deconstruction` — SELFDESTRUCT semantics.
- `persist_caches`, `report_new_preimages`, `update_commitment` — block
  finalisation hooks.
- `storage_diffs_iterator` — iterates all state changes accumulated during the
  block; used to produce pubdata.

## `FullIO` and Its Generic Parameters

`FullIO` ([source](../../../basic_system/src/system_implementation/system/io_subsystem.rs))
is the concrete IO subsystem that implements `IOSubsystem`, `IOSubsystemExt`, and
`IOTeardown`. Its full signature is:

```rust
pub struct FullIO<
    A: Allocator + Clone + Default,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SF: StackFactory<N>,
    const N: usize,
    O: IOOracle,
    M: StorageModel<IOTypes = EthereumIOTypesConfig, Resources = R, InitData = P, Allocator = A>,
    const PROOF_ENV: bool,
>
```

| Parameter | Constraint | What it represents |
|-----------|------------|--------------------|
| `A` | `Allocator + Clone + Default` | Memory allocator. In forward mode this is the global allocator (`Global`). In proving mode it is a custom bump allocator that avoids the standard `malloc`. |
| `R` | `Resources` | Dual resource counter (ergs + native). Tracks gas charges (`Ergs`) and prover-complexity charges (`Native`) in a single structure. |
| `P` | `StorageAccessPolicy<R, Bytes32>` | EE-specific cost policy for storage access. Provides warm/cold read and write costs. The EVM policy lives in `basic_system`. |
| `SF` | `StackFactory<N>` | Factory for fixed-capacity stack structures used by transient storage, event storage, log storage, etc. |
| `N` | `const usize` | Stack depth bound (number of nested frames). |
| `O` | `IOOracle` | Non-determinism source. In forward mode this wraps a database connection. In proving mode it uses CSR reads on the RISC-V target. |
| `M` | `StorageModel<IOTypes = EthereumIOTypesConfig, Resources = R, InitData = P, Allocator = A>` | The pluggable persistent storage backend (e.g. `FlatTreeWithAccountsUnderHashesStorageModel`). |
| `PROOF_ENV` | `const bool` | Compile-time flag. When `true`, the system is running inside the ZK proof (RISC-V). Enables additional proof-correctness checks such as re-hashing bytecode against the stored hash and verifying account preimage hashes. |

`FullIO` owns several sub-storages in addition to the main `M`:
- **`storage`** (`M`) — the main persistent key-value storage.
- **`transient_storage`** — EIP-1153 transient slots, cleared after each transaction.
- **`logs_storage`** — L2→L1 message logs.
- **`events_storage`** — EVM event log.
- **`interop_root_storage`** — Interop root storage for cross-chain messages.
- **`new_settlement_layer_chain_id_storage`** — Tracks chain ID updates.

## `SystemIOTypesConfig` ([source](../../../zk_ee/src/types_config/mod.rs))

This trait is the type-level configuration for all IO primitive types. It decouples
the system logic from the specific widths of addresses, keys, and values:

```rust
pub trait SystemIOTypesConfig: Sized + 'static + Send + Sync {
    type Address;           // e.g. B160 (20-byte Ethereum address)
    type StorageKey;        // e.g. Bytes32
    type StorageValue;      // e.g. Bytes32
    type NominalTokenValue; // e.g. U256
    type BytecodeHashValue; // e.g. Bytes32
    type EventKey;          // Topic type for event emission
    type SignalingKey;      // Key type for L2→L1 signals
}
```

All associated types must implement `UsizeSerializable` + `UsizeDeserializable` so
they can be exchanged with the oracle over the `usize`-based serialisation
interface (see [Oracle Serialisation](#oracle-serialisation)).

The only concrete instantiation currently used is `EthereumIOTypesConfig`:

| Type | Concrete type | Size |
|------|---------------|------|
| `Address` | `B160` | 20 bytes |
| `StorageKey` | `Bytes32` | 32 bytes |
| `StorageValue` | `Bytes32` | 32 bytes |
| `NominalTokenValue` | `U256` | 32 bytes |
| `BytecodeHashValue` | `Bytes32` | 32 bytes |
| `EventKey` | `Bytes32` | 32 bytes |
| `SignalingKey` | `Bytes32` | 32 bytes |

## Resources and Resource Charging

### The `Resources` Trait ([source](../../../zk_ee/src/system/resources.rs))

Resources are a dual counter pairing two independent budgets:

- **`Ergs`** — execution gas, charged proportionally to EVM gas consumed. Directly
  corresponds to the user-visible gas limit.
- **`Native`** — prover complexity budget. Tracks work that has no direct EVM gas
  analogue but must be bounded for proof generation (e.g. hash operations). This
  prevents DoS via cheap-gas but prover-heavy operations.

The `Resources` trait wraps both:

```rust
pub trait Resources: Sized + Clone + core::fmt::Debug {
    const FORMAL_INFINITE: Self;  // Sentinel for unconstrained execution (system paths)
    fn empty() -> Self;
    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError>;
    fn has_enough(&self, to_spend: &Self) -> bool;
    fn reclaim(&mut self, to_reclaim: Self);
    // ...
}
```

### `StorageAccessPolicy` ([source](../../../basic_system/src/system_implementation/caches/storage_access_policy.rs))

This trait encodes EE-specific storage costs. `FullIO` is parameterised over it so
that different execution environments can use different cost schedules without
branching at runtime:

```rust
pub trait StorageAccessPolicy<R: Resources, V> {
    fn charge_warm_storage_read(&self, ee_type, resources) -> Result<(), SystemError>;
    fn charge_cold_storage_read_extra(&self, ee_type, resources, is_new_slot) -> Result<(), SystemError>;
    fn charge_storage_write_extra(&self, ee_type, initial, current, new, resources, is_warm, is_new) -> Result<(), SystemError>;
    fn refund_for_storage_write(&self, ee_type, ..., refund_counter) -> Result<(), SystemError>;
}
```

For EVM: warm reads cost 100 gas, cold reads add 2100 gas, new slot writes add
20000 gas. Gas refunds from `SSTORE` (`EIP-3529`) are tracked in the
`refund_counter` field of `StorageModel`.

## `AccountDataRequest` and the `Maybe` Type

Reading account properties involves a phantom-type trick that encodes at compile
time exactly which fields are requested, avoiding over-fetching:

```rust
// In zk_ee/src/system/io.rs
pub struct AccountDataRequest<T>(PhantomData<T>);

// The Maybe marker types:
pub struct Just<T>(PhantomData<T>);  // "request this field"
pub struct Nothing;                   // "don't request this field"
```

`AccountData` is parameterised over one `Maybe<T>` per field:

```rust
pub struct AccountData<
    EEVersion,             // Maybe<u8>
    ObservableBytecodeHash,
    ObservableBytecodeLen,
    Nonce,                 // Maybe<u64>
    BytecodeHash,
    BytecodeLen,           // Maybe<u32>
    ArtifactsLen,
    NominalTokenBalance,   // Maybe<U256>
    Bytecode,              // Maybe<&'static [u8]>
    CodeVersion,
    IsDelegated,           // Maybe<bool>
> { /* fields present only when their type is Just<T> */ }
```

A request is built with a builder pattern:

```rust
let request = AccountDataRequest::empty()
    .with_nonce()
    .with_nominal_token_balance();
// The return type encodes exactly these two fields as Just<_>, rest as Nothing.
```

This ensures:
1. Only the requested data is deserialised from the oracle response.
2. Callers cannot accidentally access fields they did not request (compile error).
3. The storage implementation fetches only what is needed (e.g. avoids loading
   bytecode preimage when only the nonce is required).

## `PROOF_ENV` Const Generic

The `const PROOF_ENV: bool` parameter threads through `FullIO`, `StorageModel`
implementations, and several helper methods. When `true`:

- Bytecode hashes are **re-verified** after loading from the preimage oracle
  (guards against a malicious oracle providing wrong bytecode).
- Account property preimage hashes are **re-verified** against the hash stored
  in the tree.
- Certain internal assertions become active that are too expensive for forward
  execution.

When `false` (forward/sequencer mode):
- The system trusts the oracle responses (the sequencer controls its own oracle).
- Verification steps are skipped for performance.

This ensures the ZK proof is sound without imposing proof-environment overhead on
the hot path.

## Oracle Serialisation

All types exchanged with the oracle implement
`UsizeSerializable` / `UsizeDeserializable`
([source](../../../zk_ee/src/oracle/usize_serialization/mod.rs)).
These traits serialise values as slices of `usize` words, which allows the same
Rust code to run on both 64-bit (sequencer) and 32-bit RISC-V (prover) with
correct packing:

- On 64-bit: 1 `usize` = 8 bytes → a `U256` uses 4 words.
- On 32-bit: 1 `usize` = 4 bytes → a `U256` uses 8 words.

The trait is implemented for `u8`, `u32`, `u64`, `U256`, `B160`, `Bytes32`,
tuples, and fixed-size arrays. Little-endian encoding is used throughout.

> **Note**: Deserialization panics on values that cannot be constructed from the
> provided word count. Callers in the oracle layer must ensure the oracle response
> length matches the expected `USIZE_LEN` constant before invoking deserialization.

## Data Flow: Storage Read

```
EE calls IOSubsystem::storage_read(ee_type, resources, address, key)
  │
  └─> FullIO dispatches to:
        ├─ transient_storage  (if TRANSIENT=true)
        └─ storage (M = StorageModel)
              │
              └─> StorageModel::storage_read(ee_type, resources, address, key, oracle)
                    │
                    ├─ StorageAccessPolicy: charge warm read cost
                    │
                    └─> StorageCacheModel::read(address, key)
                          ├─ Cache hit:  return cached value
                          └─ Cache miss:
                                │
                                ├─ StorageAccessPolicy: charge cold read extra cost
                                │
                                └─> oracle.query(INITIAL_STORAGE_SLOT_VALUE_QUERY_ID,
                                                 (address, key))
                                      │
                                      └─> Deserialise → insert into cache → return
```

## Data Flow: Account Property Read

```
EE calls IOSubsystemExt::read_account_properties(request, address)
  │
  └─> StorageModel::read_account_properties(...)
        │
        ├─ Check account cache (HistoryMap keyed by address)
        │    ├─ Cache hit:  return requested fields from AccountCacheEntry
        │    └─ Cache miss:
        │          │
        │          └─> Read hash at slot (ACCOUNT_PROPERTIES_STORAGE_ADDRESS, address)
        │                │
        │                └─> oracle.query(GENERIC_PREIMAGE_QUERY_ID, hash)
        │                      │
        │                      ├─ [PROOF_ENV=true] re-verify hash of received preimage
        │                      └─> Deserialise AccountCacheEntry → insert into cache
        │
        └─ Return requested fields (type-checked at compile time by AccountDataRequest)
```

## Data Flow: Block Finalisation

```
Bootloader calls IOTeardown::finish_block()
  │
  ├─ 1. persist_caches:       flush storage diffs to result keeper
  ├─ 2. report_new_preimages: emit bytecodes and account structures for pubdata
  ├─ 3. storage_diffs_iterator: iterate all (address, key, old_value, new_value)
  ├─ 4. update_commitment:    apply diffs to Merkle tree, compute new state root
  └─ 5. result_keeper collects:
          - storage diffs  (→ pubdata encoding)
          - account diffs  (nonce, balance, bytecode hash changes)
          - preimages      (bytecodes, account structures)
          - events / L2→L1 logs
```

## Concrete Storage Model: `FlatTreeWithAccountsUnderHashesStorageModel`

The default storage model
([source](../../../basic_system/src/system_implementation/flat_storage_model/mod.rs))
is composed of:

| Field | Type | Purpose |
|-------|------|---------|
| `storage_cache` | `NewStorageWithAccountPropertiesUnderHash` | Caches raw storage slots; backed by the Merkle tree described in [tree.md](tree.md). |
| `preimages_cache` | `BytecodeAndAccountDataPreimagesStorage` | Caches bytecode and serialised account property blobs, keyed by their hash. |
| `account_data_cache` | `NewModelAccountCache` | Caches deserialized account structs for efficient per-field access. |

Account properties are **not** stored directly in the tree. Instead, a
`keccak256` hash of the serialised account struct is stored at tree slot
`(ACCOUNT_PROPERTIES_STORAGE_ADDRESS, address)` (address `0x8003`). The preimage
is loaded on demand through the oracle. This keeps tree leaves small and allows
the account encoding to change without changing the tree structure.

The `HistoryMap` ([source](../../../zk_ee/src/common_structs/history_map/mod.rs))
powers all three caches. It stores a per-key chain of historical values and
supports O(1) snapshot and rollback via a global event log plus per-entry
version pointers.

## Ethereum Storage Model: `EthereumStorageModel`

An alternative storage model
([source](../../../basic_system/src/system_implementation/ethereum_storage_model/mod.rs))
that uses a Merkle Patricia Trie (MPT) with RLP encoding, as in mainnet Ethereum.
It is used for Ethereum compatibility testing (EVM spec tests). It is not used in
production: the MPT requires a large number of preimage oracle queries (one per
trie node on each path), making it unsuitable for efficient proof generation.

## Security Notes

1. **Oracle responses are untrusted.** All data from the oracle must be validated
   before use. In `PROOF_ENV=true` mode, hashes are re-derived from the received
   preimages and compared against the tree. In forward mode the sequencer trusts
   its own oracle, but the ZK proof provides the ultimate correctness guarantee.

2. **Rollback safety.** `HistoryMap`-based caches guarantee that any changes made
   inside a call frame are atomically reverted when `finish_frame(Some(snapshot))`
   is called. This invariant is critical for correct EVM `CALL` reversion.

3. **No panics from external input.** Deserialization of oracle responses and
   resource charging both return `Result`. Callers must propagate errors rather
   than `.unwrap()` them.

4. **`PROOF_ENV` cannot be changed at runtime.** It is a const generic, so it
   is burned in at compile time. The sequencer binary and the prover binary are
   separate build artifacts.
