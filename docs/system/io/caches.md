# IO caches

As introduced in the [IO overview](./io.md), the system relies on three caches for IO. This section describes their implementation.

In general, all three caches have to provide the same functionality: materializing some data from the oracle for the first interaction, save it in a cache for further reads and updates, produce a diff to be applied at the end of the transaction and be able to handle take snapshots (via frame) to revert to in case of a invalid call.

## Trait hierarchy

Two traits in [`storage_models/src/common_structs/traits/`](../../../storage_models/src/common_structs/traits/) define the contract for cache behaviour:

```
SnapshottableIo                         (transactional rollback: begin_new_tx, finish_tx, start_frame, finish_frame)
    ├─ StorageCacheModel                (key-value slot reads/writes/touches + special account property access)
    └─ PreimageCacheModel               (hash→preimage lookup and recording)
```

### `SnapshottableIo` ([source](../../../storage_models/src/common_structs/traits/snapshottable_io.rs))

Base trait for any cache that participates in transaction and call-frame lifecycle:

- `begin_new_tx` / `finish_tx` — transaction boundary (e.g. clears transient state).
- `start_frame` → `StateSnapshot` — captures a snapshot before entering a call frame.
- `finish_frame(rollback_handle)` — commits when `None`, reverts when `Some(snapshot)`.

### `StorageCacheModel` ([source](../../../storage_models/src/common_structs/traits/storage_cache_model.rs))

Extends `SnapshottableIo`. Provides the read/write/touch interface for storage slots,
plus `read_special_account_property` / `write_special_account_property` for
account-level data stored in special slots (e.g. account aggregate data hash).

Implemented by: [`NewStorageWithAccountPropertiesUnderHash`](../../../basic_system/src/system_implementation/flat_storage_model/storage_cache.rs).

### `PreimageCacheModel` ([source](../../../storage_models/src/common_structs/traits/preimage_cache_model.rs))

Extends `SnapshottableIo`. Provides `get_preimage` (fetch by hash, hitting the oracle
on a miss) and `record_preimage` (store a new preimage and mark it for publication).

Implemented by: [`BytecodeAndAccountDataPreimagesStorage`](../../../basic_system/src/system_implementation/flat_storage_model/preimage_cache.rs).

### Account cache

[`NewModelAccountCache`](../../../basic_system/src/system_implementation/flat_storage_model/account_cache.rs)
does not implement a formal trait. It provides `start_frame`, `begin_new_tx`, and
rollback methods directly on the struct, backed by a `HistoryMap` internally. It
is used only by `FlatTreeWithAccountsUnderHashesStorageModel`, which coordinates
snapshot/rollback across all three caches.

### Coordination

[`FlatTreeWithAccountsUnderHashesStorageModel`](../../../basic_system/src/system_implementation/flat_storage_model/mod.rs) implements
both `StorageModel` and `SnapshottableIo`. Its snapshot type bundles one snapshot
ID from each cache:

```rust
pub struct FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot {
    storage: StorageSnapshotId,       // storage cache
    preimages: CacheSnapshotId,       // preimage cache
    account_data: CacheSnapshotId,    // account cache
}
```

On `start_frame` each cache is snapshotted independently; on `finish_frame` each
is rolled back (or committed) using its own handle.

## Preimage cache

The preimage cache is used for account properties preimages and bytecodes. It's implemented by [`BytecodeAndAccountDataPreimagesStorage`](../../../basic_system/src/system_implementation/flat_storage_model/preimage_cache.rs) and it contains two parts: an actual mapping between hashes and preimages (`storage`) and a `publication_storage` that deals with the rollbacking logic.

This latter keeps a map of hashes to be published (whose preimage is saved in `storage`) to some publication metadata (number of uses and size). The `publication_storage` also keeps a stack of hashes with a pointer to the start of the current frame (and a stack of pointers for previous frames). For rolling back the current frame, the cache goes through all the hashes pushed to the stack in this frame and decreases the use counter. Only preimages with non-zero use counter are published.

## Account cache

The [account cache](../../../basic_system/src/system_implementation/flat_storage_model/account_cache.rs) is used to temporarily store the account properties that will later be hashed and stored into the corresponding account properties hash slot.

For snapshotting, it uses a [`history_map`](../../../zk_ee/src/common_structs/history_map/mod.rs) together with a stack of snapshot identifiers. A history map is a key-value map that stores a history of snapshots for every value. With this, it allows to revert to any snapshot from the stack.

## Storage cache

The [storage cache](../../../basic_system/src/system_implementation/flat_storage_model/storage_cache.rs) is the general cache for the slots stored in the tree. It is implemented as a thin wrapper ([`NewStorageWithAccountPropertiesUnderHash`](../../../basic_system/src/system_implementation/flat_storage_model/storage_cache.rs)) around the generic pubdata-aware cache described below.

### `GenericPubdataAwarePlainStorage`

[Source](../../../basic_system/src/system_implementation/caches/generic_pubdata_aware_plain_storage.rs)

This is the core cache implementation, generic over key type `K`, value type `V`, allocator, and a `StorageAccessPolicy`. It handles:

- **Oracle materialisation**: on the first access to a key, queries the oracle via `InitialStorageSlotQuery` to fetch the initial value. Validates that new slots (`is_new_storage_slot == true`) have a trivial (zero) initial value — a malicious oracle returning non-zero for a new slot triggers an assertion.
- **Cold/warm tracking**: each cache element carries a `StorageElementMetadata` with the last transaction ID that touched it. The first read in a transaction is "cold" (charged extra via `StorageAccessPolicy`); subsequent reads are "warm".
- **EVM gas refund accounting**: maintains a `NonEmptyHistoryCounter` of EVM refunds. The counter participates in frame snapshots so that refunds from reverted calls are correctly discarded.
- **Pubdata awareness**: the storage model computes pubdata costs from the diff between initial and current values. The cache exposes `net_diffs_iter()` (changed slots) and `net_accesses_iter()` (all accessed slots) so the upper layer can derive pubdata obligations.
- **Snapshotting**: delegates to a `HistoryMap` for cache entries and a `NonEmptyHistoryCounter` for refunds. Both are snapshotted together via `StorageSnapshotId`.

The `StorageAccessPolicy` trait (parameterised by `P`) controls how gas/ergs are charged for cold and warm reads. This allows the same cache implementation to work under different pricing models.
