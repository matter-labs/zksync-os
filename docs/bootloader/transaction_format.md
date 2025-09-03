# Transaction format

ZKsyncOS expects transactions with the following fields:

| Field                     | Type         | Description                                                                                                                                                                                                                     |
|---------------------------|--------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `tx_type`                 | `u8`         | Type of the transaction. See the table below for supported values.                                                                                                                                                              |
| `from`                    | `B160`       | Caller.                                                                                                                                                                                                                         |
| `to`                      | `B160`       | Callee.                                                                                                                                                                                                                         |
| `gas_limit`               | `u64`        | Same meaning as Ethereum's `gasLimit`.                                                                                                                                                                                          |
| `gas_per_pubdata_limit`   | `u32`        | Maximum gas the user is willing to pay for a byte of [pubdata](https://docs.zksync.io/zksync-protocol/contracts/handling-pubdata).                                                                                               |
| `max_fee_per_gas`         | `u128`       | Maximum fee per gas the user is willing to pay. Akin to EIP-1559's `maxFeePerGas`.                                                                                                                                               |
| `max_priority_fee_per_gas`| `u128`       | Maximum priority fee per gas the user is willing to pay. Akin to EIP-1559's `maxPriorityFeePerGas`.                                                                                                                             |
| `paymaster`               | `B160`       | Transaction's paymaster. If none, it is equal to 0.                                                                                                                                                                             |
| `nonce`                   | `U256`       | Nonce of the transaction.                                                                                                                                                                                                       |
| `value`                   | `U256`       | Value to pass with the transaction.                                                                                                                                                                                             |
| `reserved`                | `[U256; 4]`  | Extra data for future use. See the table below for details on reserved fields.                                                                                                                                                   |
| `data`                    | `bytes`      | The calldata.                                                                                                                                                                                                                   |
| `signature`               | `bytes`      | Signature of the transaction.                                                                                                                                                                                                   |
| `factory_deps`            | `bytes`      | Only for EraVM. Properly formatted hashes of bytecodes to be published on L1 with this transaction. Previously published bytecodes won't incur additional fees.                                                                  |
| `paymaster_input`         | `bytes`      | Input for the paymaster.                                                                                                                                                                                                        |
| `reserved_dynamic`        | `bytes`      | Field used for extra functionality.  Currently, it's used for access and authorization lists. The field is encoded as a list, to be able to extend it in the future. The field is encoded a the ABI encoding of a bytestring containing the ABI encoding of the list itself. Currently the list contains 2 elements. First, the access list: encoded as `tuple(address, bytes32[])[]`, i.e. a list of (address, keys) pairs. Second, the authorization list: encoded as `tuple(chain_id, address, nonce, y_parity, r, s)[]`.                                                  |

### Transaction Types

| Value   | Description                                                                                       |
|---------|---------------------------------------------------------------------------------------------------|
| `0x0`   | Legacy transaction.                                                                              |
| `0x1`   | EIP-2930 transaction.                                                                            |
| `0x2`   | EIP-1559 transaction.                                                                            |
| `0x4`   | EIP-7702 transaction.                                                                            |
| `0x71`  | EIP-712 transaction following the [Era format](https://docs.zksync.io/zksync-protocol/rollup/transaction-lifecycle#eip-712-0x71). |
| `0x7E`  | Upgrade transaction.                                                                             |
| `0x7F`  | L1 -> L2 transaction.                                                                            |

### Reserved Fields

| Index   | L2 Transactions Description                                                                 | L1 Transactions Description                                                                 |
|---------|---------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| `0`     | Distinguishes EIP-155 (chain id) legacy transactions.                                       | Holds the total deposit.                                                                    |
| `1`     | EVM deployment transaction flag.                                                            | Refund recipient.                                                                           |
| `2`     | Reserved for future use.                                                                    | Reserved for future use.                                                                    |
| `3`     | Reserved for future use.                                                                    | Reserved for future use.                                                                    |

Transactions are encoded using the tightly packed ABI encoding for this list of fields. All numeric types are encoded as big-endian `U256`. Encoding and hashing of transactions is implemented in this [module](../../basic_bootloader/src/bootloader/transaction/mod.rs).
