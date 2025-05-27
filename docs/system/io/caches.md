# IO caches

As introduced in the [IO overview](./io.md), the system relies on three caches for IO. This section describes their implementation.

In general, all three caches have to provide the same functionality: materializing some data from the oracle for the first interaction, save it in a cache for further reads and updates, produce a diff to be applied at the end of the transaction and be able to handle take snapshots (via frame) to revert to in case of a invalid call.

## Preimage cache

The preimage cache is used for account properties preimages and bytecodes. It's implemented by [`BytecodeAndAccountDataPreimagesStorage`](../../../basic_system/src/system_implementation/io/preimage_cache.rs) and it contains two parts: an actual mapping between hashes and preimages (`storage`) and a `publication_storage` that deals with the rollbacking logic.

This latter keeps a map of hashes to be published (whose preimage is saved in `storage`) to some publication metadata (number of uses and size). The `publication_storage` also keeps a stack of hashes with a pointer to the start of the current frame (and a stack of pointers for previous frames). For rolling back the current frame, the cache goes through all the hashes pushed to the stack in this frame and decreases the use counter. Only preimages with non-zero use counter are published.

## Account cache

The [account cache](../../../basic_system/src/system_implementation/io/account_cache.rs) is used to temporarily store the account properties that will later be hashed and stored into the corresponding account properties hash slot.

For snapshotting, it uses a [`history_map`](../../../zk_ee/src/common_structs/history_map.rs) together with a stack of snapshot identifiers. A history map is a key-value map that stores a history of snapshots for every value. With this, it allows to revert to any snapshot from the stack.

Additionally, it keeps a stack of addresses that have been deconstructed using "SELFDESTRUCT". Due to EIP-6780, these account are only destructed at the end of the processing of the transaction. These addresses are stored in a simple stack with rollback functionality.

## Storage cache

The [storage cache](../../../basic_system/src/system_implementation/io/storage_cache.rs) is just the general cache for the slots stored in the tree. It uses the same history map implementation as the previous one for snapshotting.
