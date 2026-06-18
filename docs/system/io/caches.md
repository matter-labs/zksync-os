# IO caches

As introduced in the [IO overview](./io.md), the system relies on three caches for IO. This section describes their implementation.

In general, all three caches have to provide the same functionality: materializing some data from the oracle for the first interaction, save it in a cache for further reads and updates, produce a diff to be applied at the end of the transaction and be able to handle take snapshots (via frame) to revert to in case of a invalid call.

## Trait hierarchy

Three traits in [`storage_models/src/common_structs/traits/`](../../../storage_models/src/common_structs/traits/) define the contract for cache behavior:

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
    account_data: CacheSnapshotId,    // account cache
    preimages: CacheSnapshotId,       // preimage cache
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

This is the core cache implementation, generic over key type `K`, value type `V`, allocator, and a `StorageAccessPolicy`. It is called "pubdata-aware" because it tracks three value states per storage slot — enabling precise computation of net state changes and their pubdata cost at the end of each transaction.

#### Oracle materialisation

On the first access to a key, `materialize_element()` queries the oracle via
`InitialStorageSlotQuery` to fetch the initial value and the `is_new_storage_slot`
flag. It validates that new slots (`is_new_storage_slot == true`) have a trivial
(zero) initial value — a malicious oracle returning non-zero for a new slot
triggers an assertion. The result is inserted into the `HistoryMap`-backed cache.

#### Cold/warm tracking

Each cache element carries a `StorageElementMetadata` recording the last
transaction ID that touched it. On access:

1. `materialize_element()` always charges a warm read first via `charge_warm_storage_read()`.
2. If the element's `last_touched_in_tx` does not match the current transaction ID
   (i.e. the access is "cold"), it additionally charges `charge_cold_storage_read_extra()`.
3. `last_touched_in_tx` is updated to the current transaction ID, making all
   subsequent accesses within the same transaction "warm".
4. At the transaction boundary, `finish_tx()` increments the transaction ID counter,
   resetting all elements to "cold" for the next transaction.

#### EVM gas refund accounting

The cache maintains a `NonEmptyHistoryCounter` of cumulative EVM refunds. Every
`apply_write_impl()` call passes the three-value state to `refund_for_storage_write()`,
which implements the EIP-3529 refund rules (e.g. +4800 gas when clearing a slot
from non-zero to zero). The counter participates in frame snapshots, so refunds
from reverted calls are correctly discarded.

#### Three-value state tracking

Each cached storage element tracks three values simultaneously:

- **`initial`** — the value at the start of the block, loaded from the oracle.
- **`committed`** (at transaction start) — the value at the start of the current
  transaction, frozen by `begin_new_tx()`. This is `initial` for the first
  transaction, or the value at the end of the previous transaction.
- **`current`** — the latest value after all writes in the current call stack.

This three-value model is essential for:

1. **Storage write gas costs** — EVM `SSTORE` pricing differs depending on whether
   the current write is "fresh" (`current == committed`) or "dirty" (`current !=
   committed`).
2. **EVM gas refunds** — refunds are computed from the transition between
   `committed`, `current`, and the new value.
3. **Pubdata cost computation** — only `committed → current` changes contribute
   to pubdata. Changes that reset to the committed value cancel out.

#### Pubdata computation

At the end of each transaction, `calculate_pubdata_used_by_tx()` iterates all
elements altered since the last commit via `iter_altered_since_commit()` and sums
up the net pubdata bytes:

1. **Deduplication** — multiple writes to the same key count once.
2. **Skip account properties** — slots under `ACCOUNT_PROPERTIES_STORAGE_ADDRESS`
   are published as preimages, not as storage diffs.
3. **Elimination** — if `current == initial`, the change nets to zero and is free.
4. **Compression** — for each net change, the cost is 32 bytes (key) plus the
   optimally compressed value diff. Compression strategies include add/subtract
   delta encoding and leading-zero removal, selecting whichever produces the
   fewest bytes.

The cache exposes two iterators for upper layers:
- `net_diffs_iter()` — yields only slots where `current_value != initial_value`
  (used for publishing state changes to the DA layer).
- `net_accesses_iter()` — yields all accessed slots regardless of change (used
  for Merkle proof validation, since all accessed slots need proofs).

#### `StorageAccessPolicy`

The [`StorageAccessPolicy`](../../../basic_system/src/system_implementation/caches/storage_access_policy.rs)
trait (parameterised by `P`) controls how gas/ergs are charged for storage
operations. It defines four methods:

- `charge_warm_storage_read` — base cost for every storage access (EVM: 100 gas).
- `charge_cold_storage_read_extra` — additional cost on the first access in a
  transaction (EVM: 2000 gas, totalling 2100 for a cold read).
- `charge_storage_write_extra` — write cost that varies based on the value
  transition (EVM: 0 for no-change, 20000 for fresh set, 5000 for fresh reset,
  plus 100 for cold writes).
- `refund_for_storage_write` — EIP-3529 refund calculation.

This abstraction allows the same `GenericPubdataAwarePlainStorage` to work under
different pricing models without branching at runtime.

#### Snapshotting

The cache creates a composite `StorageSnapshotId` that bundles a `HistoryMap`
snapshot and a refund counter snapshot. On `start_frame()` both are captured;
on `finish_frame(Some(snapshot))` both are rolled back atomically. This ensures
that a reverted `CALL` or `CREATE` discards both its storage changes and its
accumulated gas refunds.
