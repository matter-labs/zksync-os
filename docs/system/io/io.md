
# IO subsystem

The IO subsystem is designed to abstract over the underlying storage implementation. This decision allows ZKsyncOS to be instantiated with different storage implementations depending on the chain needs.
Each implementation is responsible for charging resources, and the charging can be different for each Execution Environment type.

## Interfaces

The only requirements that the IO subsystem interface imposes are:

- The storage is a key-value store of slots, where each slot is indexed by a pair (address, key).
- The storage implements an account model for addresses, where addresses have:
  - A token balance.
  - A nonce.
  - Optionally some bytecode, which has two hashes. In practice, this is used to use a prover-friendly hash function for the "usable" bytecode hash while keeping an "observable" hash for compatibility with, for example, the Keccak hash expected in EVM.
- Storage of events and signals emitted during execution.

All the types for the account model are specified in the `SystemIOTypesConfig`.

### User-facing interface

The user-facing interface, [`IOSubsystem`](../../../zk_ee/src/system/io.rs) provides methods for:

- Reading and writing into storage slots,
- Querying some of the account properties,
- Emitting events and signals.

Note that this methods require an EE type, to potentially charge resources following an EE-specific policy.

### Extended interface

The extended interface, [`IOSubsystemExt`](../../../zk_ee/src/system/io.rs) extends the previous one with methods for:

- Updating an account's nonce and balance,
- Deploy and destruct an account's code,
- Starting and finishing an IO frame, reverting storage changes if necessary.

## Basic IO implementation

The basic [IO subsystem implementation](../../../basic_system/src/system_implementation/system/io_subsystem.rs) is composed by 4 different storages:

- The main persistent storage slot-based storage, described later in this section.
- The [transient storage](../../../storage_models/src/common_structs/generic_transient_storage.rs), a key-value map that is discarded after processing each transaction.
- [Logs](../../../zk_ee/src/common_structs/logs_storage.rs) and [events](../../../zk_ee/src/common_structs/events_storage.rs) storages: two simple rollbackable storages. They are initialized empty for each block and dumped in full at block finalization into the block result.

The basic implementation consist mostly on handling those four storages. We'll focus on the main storage, as the rest are quite straightforward.

The main storage, implemented by [`FlatTreeWithAccountsUnderHashesStorageModel`](../../../basic_system/src/system_implementation/flat_storage_model/mod.rs), is composed of a Merkle tree and 3 caches for its data. The Merkle tree uses 32-byte keys (hash of (address,key)) and 32-byte values, and is described in details in [its own page](./tree.md). Initial reads into this tree are provided by an oracle, and verified as a batch at the end of the system run. The three caches are for storage (general storage slots), account properties and preimages. The use of the last two will become clear after the next section.

### Storage model for accounts

Each account has the following properties:

- Versioning data (EE, code version, deployment status, aux bitmasks),
- Nonce,
- Base token balance,
- Bytecode hash,
- Bytecode length,
- Observable bytecode hash,
- Observable bytecode length,
- Artifacts length (unused for now).

The precise serialization layout for this information can be found in the [implementation](../../../basic_system/src/system_implementation/flat_storage_model/account_cache_entry.rs).
For a given address, its **properties aren't stored directly into the tree**. Instead, a hash of the properties is stored under at slot (`ACCOUNT_PROPERTIES_STORAGE_ADDRESS`, address).
The preimage of this hash (i.e. the encoded properties) is initially read from the `preimages_source`, which is part of the oracle. `ACCOUNT_PROPERTIES_STORAGE_ADDRESS` is the address `0x8003`, and is just used to store these special properties hashes.

During the execution of a block, accounts that are decommitted (i.e. read from the oracle verifying their hash) are cached in the account cache. At block finalization, the accounts in the account cache are hashed and inserted into the preimage cache. In turn, the preimages in the preimage cache (which include serialized accounts and bytecodes) are reported to the block's result and are included in the pubdata.

The underlying implementation of the caches is described in the [Caches section](caches.md).

### Finish method

The finish method is the main method executed during block finalization.
It has different implementations depending on whether it's a forward or proof run.

For the forward run, we are just returning IO outputs(state diffs, events, messages) and pubdata to the caller(using result keeper).

For the proof run we should validate reads, apply writes to the state commitment, and calculate pubdata commitment.
Calculate and return public input using these and some other values.
