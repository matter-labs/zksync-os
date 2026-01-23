# Bootloader

The bootloader is the component responsible for implementing the general blockchain protocol. Roughly, this means:

1. Initializing the system.
2. Reading the block context from oracle.
3. Reading and parsing the first transaction.
4. Validating the transaction.
5. Executing the transaction.
6. Saving the transaction result.
7. Repeat from 3 until there are no more transactions.
8. Finalizing the block.

This component is, as the name suggests, the entrypoint of the system. The function [`run_prepared`](../../basic_bootloader/src/bootloader/mod.rs) implements this top-level main loop.

## Configuration

The bootloader can be configured with the following parameters (found in the [`BasicBootloaderExecutionConfig`](../../basic_bootloader/src/bootloader/mod.rs) struct):

- `ONLY_SIMULATE`: skips the [validation](./transaction_processing.md#validation) step when processing a transaction. Used for call simulation in the node.

## Code execution

For transaction execution, the bootloader has to execute some contract code. This contract code corresponds to one of the supported VMs, as is executed through the [Execution Environment (EE)](../execution_environments/execution_environments.md) module.

A contract call is executed through an interplay between the bootloader and (potentially different) execution environments. Indeed, a contract executing in a given EE can call to contracts that run on a different EE or to a [System Hook](../system_hooks.md). This interplay is described in [Runner flow](./runner_flow.md).

## Block header

At the end of the execution, the bootloader outputs block header to the system.

For the block header, we use Ethereum block header format.
However, some of the fields will be set differently in the first version for simplification (most likely it will change before the mainnet launch).

The block header should determine the block fully, i.e. include all the inputs needed to execute the block.
Currently it misses `gas_per_pubdata` and `native_price`, but we already working on design and implementation to solve this issue.

| Ethereum field name | Ethereum value                                                                   | ZKsync OS value                                                    | Comments                                |
|---------------------|----------------------------------------------------------------------------------|--------------------------------------------------------------------|-----------------------------------------|
| parent_hash         | previous block hash                                                              | previous block hash                                                |                                         |
| owners_hash         | 0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347 (post merge)  | 0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347 | hash of empty rlp list                  |
| beneficiary         | block proposer                                                                   | Operator(fee) address                                              |                                         |
| state_root          | state commitment                                                                 | 0                                                                  |                                         |
| transactions_root   | transactions trie(patricia merkle tree) root                                     | transactions rolling hash                                          |                                         |
| receipts_root       | receipts  trie(patricia merkle tree) root                                        | 0                                                                  |                                         |
| logs_bloom          | 2048 bits bloom filter over logs addresses and topics                            | 0                                                                  |                                         |
| difficulty          | 0 (post merge)                                                                   | 0                                                                  |                                         |
| number              | block number                                                                     | block number                                                       |                                         |
| gas_limit           | block gas limit                                                                  | constant, not defined yet, 10-15m most likely                      |                                         |
| gas_used            | block gas used                                                                   | block gas used                                                     | TBD with or without pubdata             |
| timestamp           | block timestamp                                                                  | block timestamp                                                    |                                         |
| extra_data          | any extra data included by proposer                                              | TBD, possibly gas_per_pubdata and native price                     |                                         |
| mix_hash            | beacon chain provided random, prevrandao (post merge)                            | 0                                                                  | after consensus will be provided random |
| nonce               | 0 (post merge)                                                                   | 0                                                                  |                                         |
| base_fee_per_gas    | base_fee_per_gas                                                                 | base_fee_per_gas                                                   |                                         |

## Block and Batch Finalization

After processing all transactions in a block, the bootloader performs finalization through specialized post-transaction operations. The specific finalization behavior depends on the execution mode:

### Post-Transaction Operations

The bootloader uses different post-tx operations for different execution scenarios:

#### Sequencing Mode
- **Implementation**: [`PostTxOpSequencing`](../../basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_sequencing.rs)
- **Purpose**: Forward execution (sequencer mode)
- **Output**: Returns IO outputs (state diffs, events, messages) and pubdata to the caller via result keeper
- **Block Header**: Generates standard block header for the executed block

#### Proving Mode - Single Block Batch
- **Implementation**: [`PostTxOpProvingSingleblockBatch`](../../basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_proving_singleblock_batch.rs)
- **Purpose**: Proving execution for single-block batches
- **Process**:
  - Validates oracle-provided reads
  - Applies writes to state commitment
  - Calculates pubdata commitment using configured DA scheme
  - Generates public input hash for the batch
- **Output**: Public input hash and finalized oracle state

#### Proving Mode - Multiblock Batch
- **Implementation**: [`PostTxOpProvingMultiblockBatch`](../../basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_proving_multiblock_batch.rs)
- **Purpose**: Accumulates multiple blocks into a single batch
- **Process**: Applies block data to an external batch accumulator
- **Output**: Updated batch state for later finalization

### Data Availability Integration

All proving modes integrate with the [Data Availability commitment schemes](../da_commitment_schemes.md) to:
- Generate appropriate DA commitments based on the configured scheme
- Write versioned pubdata with proper format encoding
- Include pubdata in the final outputs or public inputs

This separation of finalization logic allows the system to support different deployment scenarios while maintaining consistency in the core transaction processing pipeline.
