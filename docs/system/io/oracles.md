# Oracles

Oracles are an abstraction boundary on how the system gets data from the outer world.
The oracle is just providing the system with some data, the caller is responsible of verifying it.

The oracle interface can be found in the [`IOOracle`](../../../zk_ee/src/system_io_oracle/mod.rs) trait of the system interface.

The system uses oracles for:

- Reading the next transaction size and data.
- Reading block metadata (this is verified by having it as part of the public inputs).
- Retrieving preimages for bytecode and account hashes. Bytecode hashes are verified (recomputed) before actually using the bytecode, while account hashes are verified while materializing the account properties for the first time (both are only done if running in proving environment).
- Initial read into a storage slot. These are cached and verified only one at the end of the batch, as explained in the [`tree` section](tree.md).
- Helpers for the tree, such as getting indices, as explained also in the tree section.
- Reading the state commitment.

## Implementations

The main reason why we have this trait as abstraction is because we want to have different implementations for forward running (in sequencer), and proving running.

For forward running it can be implemented pretty straight forward, the easiest way is just to define a structure that has access to all the needed data, probably connected to a DB. For example we have the following [forward running implementation](../../../forward_system/src/run/oracle.rs).

For proving it’s a bit harder, as we are running it as a separate program on the RISC-V machine, so somehow we need to get data into the RISC-V machine.

The way it's done is that we have a special behavior for system register (CSR) reading/writing in our risc-v implementation. It can execute some special logic on write and return any data for read.

The methods to use it from the rust program are located [here](../../../zksync_os/src/csr_io.rs), which are then used to implement the [`CsrBasedIOOracle`](../../../proof_running_system/src/io_oracle/mod.rs).
