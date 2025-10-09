# Oracles

Oracles are an abstraction boundary on how the system gets data from the outer world.
The oracle is just providing the system with some data, the caller is responsible of verifying it.

The oracle interface can be found in the [`IOOracle`](../../../zk_ee/src/oracle/mod.rs) trait of the system interface.

The system uses oracles for:

- Reading the next transaction size and data.
- Reading block metadata (this is verified by having it as part of the public inputs).
- Retrieving preimages for bytecode and account hashes. Bytecode hashes are verified (recomputed) before actually using the bytecode, while account hashes are verified while materializing the account properties for the first time (both are only done if running in proving environment).
- Initial read into a storage slot. These are cached and verified only one at the end of the batch, as explained in the [`tree` section](tree.md).
- Helpers for the tree, such as getting indices, as explained also in the tree section.
- Reading the state commitment.

## Implementations

The main reason why we have this trait as abstraction is because we want to have different implementations for forward running (in sequencer), and proving running.

For forward running it can be implemented pretty straight forward, the easiest way is just to define a structure that has access to all the needed data, probably connected to a DB. For example we have the following set of [oracle query processors](../../../forward_system/src/run/query_processors).

For proving it’s a bit harder, as we are running it as a separate program on the RISC-V machine, so somehow we need to get data into the RISC-V machine.

The way it's done is that we have a special behavior for system register (CSR) reading/writing in our risc-v implementation. It can execute some special logic on write and return any data for read.

The methods to use it from the rust program are located [here](../../../zksync_os/src/csr_io.rs), which are then used to implement the [`CsrBasedIOOracle`](../../../proof_running_system/src/io_oracle/mod.rs).

## Oracle Query System

ZKsync OS uses a structured query system to request different types of data from oracles. Each query type is identified by a unique 32-bit query ID that follows a hierarchical bitmask structure for namespace isolation.

### Query ID Organization

Query IDs are organized using bitmasks to separate different categories:

- **Top-level masks**: Define major categories (reserved, basic functionality)
- **Subspace masks**: Define subcategories within each major category
- **Individual query IDs**: Assigned within each subcategory

The core query ID definitions can be found in [`query_ids.rs`](../../../zk_ee/src/oracle/query_ids.rs).

### Core Oracle Queries

#### Special Queries
- **UART_QUERY_ID** (`0xffffffff`): Special debugging query for UART output (logs in simulator)

#### System Queries (0x40000000)
- **DISCONNECT_ORACLE_QUERY_ID** (`0x40000000`): Signals the oracle to disconnect and switch to autonomous execution mode

#### Transaction Queries (0x40060000)
- **NEXT_TX_SIZE_QUERY_ID** (`0x40060000`): Gets the size (in bytes) of the next transaction to be processed
- **TX_DATA_WORDS_QUERY_ID** (`0x40060001`): Retrieves transaction data words for the current transaction

#### Block/Batch Queries (0x40070000)
- **BLOCK_METADATA_QUERY_ID** (`0x40070000`): Retrieves block metadata (timestamp, number, etc.)
- **ZK_PROOF_DATA_INIT_QUERY_ID** (`0x40070001`): Gets data required for state correctness proving

#### Storage and State Queries (0x40030000, 0x40040000)
- **INITIAL_STORAGE_SLOT_VALUE_QUERY_ID** (`0x40030000`): Gets the initial value of a storage slot before modifications
- **INITIAL_STATE_COMMITMENT_QUERY_ID** (`0x40040000`): Gets the initial state commitment (root hash) before block execution

#### Preimage Queries (0x40020000)
- **GENERIC_PREIMAGE_QUERY_ID** (`0x40020000`): Retrieves preimage data for a given hash

#### Computational Advice Queries (0x40050000)
- **MODEXP_ADVICE_QUERY_ID** (`0x40050010`): Provides computational advice for modular exponentiation operations

### STF-specific Oracle Queries

#### Flat Storage Model Queries (0x4004f000, 0x4002f000)
The flat storage model implements a fixed-height Merkle tree with linked lists in leaves, sorted by storage keys:

- **PreviousIndexQuery** (`0x4004f000`): Gets the previous index for a storage key in the flat storage tree
- **ExactIndexQuery** (`0x4004f001`): Gets the exact index for a storage key if it exists in the flat storage tree
- **PROOF_FOR_INDEX_QUERY_ID** (`0x4004f002`): Gets Merkle proof for a value at a specific index in flat storage
- **FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID** (`0x4002f000`): Gets preimage data specific to flat storage implementation

### Security Considerations

**Important**: Oracle responses are non-deterministic and MUST be treated as untrusted input. All responses should be:

- Treated as opaque byte arrays, or
- Validated against additional constraints during deserialization or usage

Query ID uniqueness is not enforced on the caller side, so proper validation is critical for system security.
