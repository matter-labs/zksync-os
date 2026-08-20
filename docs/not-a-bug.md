# Not a Bug

This page lists ZKsync OS behavior that is intentional, protocol-correct, or already documented, but is commonly misreported as a vulnerability.

Use this page as a triage aid, not as a substitute for checking the code. If an observed behavior differs from what is described here, investigate it normally.

## EVM / Osaka (Fusaka) Behavior

### Osaka (Fusaka) is the supported EVM hardfork

ZKsync OS 0.4.0 targets Osaka (Fusaka) EVM semantics, including changes introduced after Cancun. Reports about Pectra or Fusaka behavior must be evaluated against Osaka rather than dismissed as relying on an unsupported later hardfork. Only reports that assume behavior introduced after Osaka are outside the currently supported EVM hardfork.

See [EVM Execution Environment](./execution_environments/evm.md).

### EIP-4844 transactions are disabled in production

EIP-4844 type `0x03` transactions are implemented behind the `basic_bootloader/eip-4844` feature, but this feature is not enabled by the production feature sets. It is enabled for test and Ethereum test-runner configurations.

See [Transaction formats](./bootloader/transaction_format.md).

### `BLOBHASH` returns `0`

This is expected in production. Since EIP-4844 transactions are disabled, transactions do not have blob versioned hashes. Cancun/EIP-4844 specifies that `BLOBHASH(index)` returns a zeroed `bytes32` when `index` is outside `tx.blob_versioned_hashes`.

### `BLOBBASEFEE` returns `1`

This is expected in production. There are no EIP-4844 transactions in production history, so `excess_blob_gas` remains `0`. Cancun/EIP-4844 defines the minimum blob base fee as `1`, and EIP-7516 makes `BLOBBASEFEE` return the current block's blob base fee.

### Opcode `0x44` returns `1`

Opcode `0x44` is `PREVRANDAO` under Cancun semantics, historically named `DIFFICULTY`. ZKsync OS uses the block `mix_hash` value for this opcode. In production, `mix_hash` is mocked to `1`.

This does not contradict the block header `difficulty` field being `0` after the Merge.

See [Bootloader block header](./bootloader/bootloader.md#block-header).

### `SUB` operand order

ZKsync OS follows the standard EVM operand order for `SUB`. The top stack item is the first operand, and the item below it is the second operand. For example, `PUSH1 0x03 PUSH1 0x05 SUB` leaves `0x02` on the stack.

This is not a reversed subtraction bug.
